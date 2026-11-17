use std::collections::HashMap;

use rusqlite::params;
use serde::Serialize;
use specta::Type;

use crate::error::AppError;
use crate::indexer::db::IndexDb;

#[derive(Debug, Clone, Serialize, Type)]
pub struct SearchHit {
    pub doc_id: i32,
    pub root_id: i32,
    pub root_label: String,
    pub rel_path: String,
    pub file_name: String,
    pub title: String,
    pub tags: Vec<String>,
    pub word_count: u32,
    pub mtime_ms: f64,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct DocSnippet {
    pub doc_id: i32,
    pub html: String,
}

#[derive(Debug, Default)]
struct ParsedQuery {
    terms: Vec<String>,
    phrases: Vec<String>,
    tag_filters: Vec<String>,
    path_filters: Vec<String>,
}

impl ParsedQuery {
    fn free_text(&self) -> String {
        let mut parts = Vec::with_capacity(self.phrases.len() + self.terms.len());
        parts.extend(self.phrases.iter().cloned());
        parts.extend(self.terms.iter().cloned());
        parts.join(" ")
    }

    fn has_text(&self) -> bool {
        !self.terms.is_empty() || !self.phrases.is_empty()
    }
}

fn parse_query(raw: &str) -> ParsedQuery {
    let mut parsed = ParsedQuery::default();
    let mut chars = raw.trim().chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }
        if ch == '"' {
            chars.next();
            let mut phrase = String::new();
            for c in chars.by_ref() {
                if c == '"' {
                    break;
                }
                phrase.push(c);
            }
            let trimmed = phrase.trim();
            if !trimmed.is_empty() {
                parsed.phrases.push(trimmed.to_string());
            }
            continue;
        }

        let mut token = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                break;
            }
            token.push(c);
            chars.next();
        }

        if let Some(tag) = token
            .strip_prefix("tag:")
            .or_else(|| token.strip_prefix('#'))
        {
            let clean = tag.trim().to_lowercase();
            if !clean.is_empty() {
                parsed.tag_filters.push(clean);
            }
        } else if let Some(path_q) = token.strip_prefix("path:") {
            let clean = path_q.trim().to_lowercase();
            if !clean.is_empty() {
                parsed.path_filters.push(clean);
            }
        } else if !token.is_empty() {
            parsed.terms.push(token);
        }
    }

    parsed
}

/// Builds an injection-safe FTS5 query string where each token is quoted and prefix-matched.
pub fn build_fts_match_expr(raw: &str) -> Option<String> {
    let pq = parse_query(raw);
    build_fts_from_parsed(&pq)
}

fn build_fts_from_parsed(pq: &ParsedQuery) -> Option<String> {
    let mut clauses = Vec::new();
    for phrase in &pq.phrases {
        let escaped = phrase.replace('"', "\"\"");
        if !escaped.trim().is_empty() {
            clauses.push(format!("\"{escaped}\""));
        }
    }
    for term in &pq.terms {
        // Strip FTS operators and wrap in quotes + prefix wildcard
        let sanitized: String = term
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
            .collect();
        if !sanitized.is_empty() {
            let escaped = sanitized.replace('"', "\"\"");
            clauses.push(format!("\"{escaped}\"*"));
        }
    }
    if clauses.is_empty() {
        None
    } else {
        Some(clauses.join(" "))
    }
}

fn build_trigram_expr(pq: &ParsedQuery) -> Option<String> {
    let text = pq.free_text();
    let trimmed = text.trim();
    if trimmed.chars().count() < 3 {
        return None;
    }
    let escaped = trimmed.replace('"', "\"\"");
    Some(format!("\"{escaped}\""))
}

pub fn search_documents(
    db: &IndexDb,
    raw_query: &str,
    root_filter: Option<i64>,
    limit: usize,
) -> Result<Vec<SearchHit>, AppError> {
    let limit = limit.clamp(1, 500);
    let pq = parse_query(raw_query);
    let conn = db.read_conn()?;

    // Empty free-text query (optionally filtered by tag:/path:/root_id) -> recency order
    if !pq.has_text() {
        return fetch_recent_or_filtered(&conn, &pq, root_filter, limit);
    }

    let mut rrf_scores: HashMap<i64, f64> = HashMap::new();
    const RRF_K: f64 = 60.0;

    // 1. FTS5 word + prefix search (BM25 weights: file_name=10, title=8, tags=5, body=1)
    if let Some(fts_expr) = build_fts_from_parsed(&pq) {
        let mut stmt = conn
            .prepare_cached(
                "SELECT d.id
                 FROM documents_fts f
                 JOIN documents d ON d.id = f.rowid
                 JOIN roots r ON r.id = d.root_id
                 WHERE documents_fts MATCH ?1 AND r.enabled = 1
                   AND (?2 IS NULL OR d.root_id = ?2)
                 ORDER BY bm25(documents_fts, 10.0, 8.0, 5.0, 1.0)
                 LIMIT 200",
            )
            .map_err(|e| AppError::Db(e.to_string()))?;

        let ids: Vec<i64> = stmt
            .query_map(params![fts_expr, root_filter], |r| r.get(0))
            .map_err(|e| AppError::Db(e.to_string()))?
            .filter_map(Result::ok)
            .collect();

        for (rank, id) in ids.into_iter().enumerate() {
            *rrf_scores.entry(id).or_default() += 1.4 / (RRF_K + rank as f64 + 1.0);
        }
    }

    // 2. FTS5 trigram substring match on metadata (queries >= 3 chars)
    if let Some(trgm_expr) = build_trigram_expr(&pq) {
        let mut stmt = conn
            .prepare_cached(
                "SELECT d.id
                 FROM documents_trgm t
                 JOIN documents d ON d.id = t.rowid
                 JOIN roots r ON r.id = d.root_id
                 WHERE documents_trgm MATCH ?1 AND r.enabled = 1
                   AND (?2 IS NULL OR d.root_id = ?2)
                 LIMIT 200",
            )
            .map_err(|e| AppError::Db(e.to_string()))?;

        if let Ok(rows) = stmt.query_map(params![trgm_expr, root_filter], |r| r.get::<_, i64>(0)) {
            for (rank, id) in rows.filter_map(Result::ok).enumerate() {
                *rrf_scores.entry(id).or_default() += 1.0 / (RRF_K + rank as f64 + 1.0);
            }
        }
    }

    // 3. In-memory nucleo fuzzy match for typo tolerance
    let free = pq.free_text();
    let fuzzy_hits = db.fuzzy_cache.search(&free, root_filter, 200);
    for (rank, (id, _fuzzy_score)) in fuzzy_hits.into_iter().enumerate() {
        *rrf_scores.entry(id).or_default() += 0.9 / (RRF_K + rank as f64 + 1.0);
    }

    if rrf_scores.is_empty() {
        return Ok(Vec::new());
    }

    // Fetch metadata for candidate IDs (bounded <= 300) and apply tag:/path: filters + recency boost.
    let mut candidates: Vec<(i64, f64)> = rrf_scores.into_iter().collect();
    candidates.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
    candidates.truncate(300);

    let mut hits = Vec::new();
    let mut stmt = conn
        .prepare_cached(
            "SELECT d.id, d.root_id, r.label, d.rel_path, d.file_name, d.title, d.tags_text,
                    d.word_count, d.mtime_ns
             FROM documents d
             JOIN roots r ON r.id = d.root_id
             WHERE d.id = ?1 AND r.enabled = 1",
        )
        .map_err(|e| AppError::Db(e.to_string()))?;

    for (doc_id, base_score) in candidates {
        if let Ok(hit) = stmt.query_row([doc_id], |row| {
            let tags_text: String = row.get(6)?;
            let tags: Vec<String> = tags_text
                .split_whitespace()
                .map(std::string::ToString::to_string)
                .collect();
            let mtime_ns: i64 = row.get(8)?;
            Ok(SearchHit {
                doc_id: row.get::<_, i64>(0)? as i32,
                root_id: row.get::<_, i64>(1)? as i32,
                root_label: row.get(2)?,
                rel_path: row.get(3)?,
                file_name: row.get(4)?,
                title: row.get(5)?,
                tags,
                word_count: row.get::<_, i64>(7)?.max(0) as u32,
                mtime_ms: (mtime_ns as f64) / 1_000_000.0,
                score: base_score,
            })
        }) {
            if !matches_filters(&hit, &pq) {
                continue;
            }
            hits.push(hit);
        }
    }

    hits.sort_unstable_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| b.mtime_ms.total_cmp(&a.mtime_ms))
    });
    hits.truncate(limit);
    Ok(hits)
}

fn fetch_recent_or_filtered(
    conn: &rusqlite::Connection,
    pq: &ParsedQuery,
    root_filter: Option<i64>,
    limit: usize,
) -> Result<Vec<SearchHit>, AppError> {
    let sql_limit = if pq.tag_filters.is_empty() && pq.path_filters.is_empty() {
        limit as i64
    } else {
        (limit * 5).min(1000) as i64
    };

    let mut stmt = conn
        .prepare_cached(
            "SELECT d.id, d.root_id, r.label, d.rel_path, d.file_name, d.title, d.tags_text,
                    d.word_count, d.mtime_ns
             FROM documents d
             JOIN roots r ON r.id = d.root_id
             WHERE r.enabled = 1 AND (?1 IS NULL OR d.root_id = ?1)
             ORDER BY d.mtime_ns DESC
             LIMIT ?2",
        )
        .map_err(|e| AppError::Db(e.to_string()))?;

    let rows = stmt
        .query_map(params![root_filter, sql_limit], |row| {
            let tags_text: String = row.get(6)?;
            let tags: Vec<String> = tags_text
                .split_whitespace()
                .map(std::string::ToString::to_string)
                .collect();
            let mtime_ns: i64 = row.get(8)?;
            Ok(SearchHit {
                doc_id: row.get::<_, i64>(0)? as i32,
                root_id: row.get::<_, i64>(1)? as i32,
                root_label: row.get(2)?,
                rel_path: row.get(3)?,
                file_name: row.get(4)?,
                title: row.get(5)?,
                tags,
                word_count: row.get::<_, i64>(7)?.max(0) as u32,
                mtime_ms: (mtime_ns as f64) / 1_000_000.0,
                score: 0.0,
            })
        })
        .map_err(|e| AppError::Db(e.to_string()))?;

    let mut out = Vec::new();
    for item in rows {
        let hit = item.map_err(|e| AppError::Db(e.to_string()))?;
        if matches_filters(&hit, pq) {
            out.push(hit);
            if out.len() >= limit {
                break;
            }
        }
    }
    Ok(out)
}

fn matches_filters(hit: &SearchHit, pq: &ParsedQuery) -> bool {
    for tf in &pq.tag_filters {
        if !hit.tags.iter().any(|t| t.contains(tf)) {
            return false;
        }
    }
    for pf in &pq.path_filters {
        if !hit.rel_path.to_lowercase().contains(pf) {
            return false;
        }
    }
    true
}

pub fn get_snippets(
    db: &IndexDb,
    doc_ids: &[i32],
    raw_query: &str,
) -> Result<Vec<DocSnippet>, AppError> {
    if doc_ids.is_empty() {
        return Ok(Vec::new());
    }
    let conn = db.read_conn()?;
    let fts_expr = build_fts_match_expr(raw_query);

    let mut out = Vec::with_capacity(doc_ids.len());
    let mut fts_stmt = conn
        .prepare_cached(
            "SELECT snippet(documents_fts, 3, '<mark>', '</mark>', '…', 18)
             FROM documents_fts
             WHERE documents_fts MATCH ?1 AND rowid = ?2
             LIMIT 1",
        )
        .map_err(|e| AppError::Db(e.to_string()))?;
    let mut fallback_stmt = conn
        .prepare_cached("SELECT substr(body, 1, 180) FROM documents WHERE id = ?1")
        .map_err(|e| AppError::Db(e.to_string()))?;

    for &doc_id in doc_ids {
        let id64 = i64::from(doc_id);
        let snippet_raw = if let Some(ref expr) = fts_expr {
            fts_stmt
                .query_row(params![expr, id64], |r| r.get::<_, String>(0))
                .ok()
        } else {
            None
        };

        let text = match snippet_raw {
            Some(s) if !s.trim().is_empty() => sanitize_snippet(&s),
            _ => {
                let raw_body: String = fallback_stmt
                    .query_row([id64], |r| r.get(0))
                    .unwrap_or_default();
                let collapsed = raw_body.split_whitespace().collect::<Vec<_>>().join(" ");
                html_escape(&collapsed)
            }
        };

        out.push(DocSnippet { doc_id, html: text });
    }

    Ok(out)
}

fn sanitize_snippet(raw: &str) -> String {
    // Only preserve <mark> and </mark> generated by FTS5 snippet(), escape everything else.
    const OPEN_SENTINEL: &str = "\x01MARK_OPEN\x02";
    const CLOSE_SENTINEL: &str = "\x01MARK_CLOSE\x02";
    let marked = raw
        .replace("<mark>", OPEN_SENTINEL)
        .replace("</mark>", CLOSE_SENTINEL);
    html_escape(&marked)
        .replace(OPEN_SENTINEL, "<mark>")
        .replace(CLOSE_SENTINEL, "</mark>")
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
