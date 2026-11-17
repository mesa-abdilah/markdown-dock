use std::collections::BTreeSet;
use std::path::Path;

use unicode_normalization::UnicodeNormalization;
use yaml_rust2::{Yaml, YamlLoader};

#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub title: String,
    pub tags: Vec<String>,
    pub tags_text: String,
    pub frontmatter_json: Option<String>,
    pub body: String,
    pub word_count: u32,
}

pub fn normalize_rel_path(rel: &Path) -> String {
    let raw = rel
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    raw.nfc().collect()
}

pub fn parse_markdown(raw_bytes: &[u8], file_name: &str) -> ParsedDocument {
    let text = String::from_utf8_lossy(raw_bytes);
    let clean = text.strip_prefix('\u{FEFF}').unwrap_or(&text);

    let (frontmatter_yaml, body_slice) = split_frontmatter(clean);
    let mut title_from_fm: Option<String> = None;
    let mut tags_set = BTreeSet::<String>::new();
    let mut frontmatter_json: Option<String> = None;

    if let Some(yaml_str) = frontmatter_yaml
        && let Ok(docs) = YamlLoader::load_from_str(yaml_str)
        && let Some(doc) = docs.first()
    {
        if let Some(json_val) = yaml_to_json(doc)
            && let Ok(serialized) = serde_json::to_string(&json_val)
        {
            frontmatter_json = Some(serialized);
        }

        if let Yaml::Hash(map) = doc {
            for (k, v) in map {
                let Some(key) = k.as_str() else { continue };
                match key.to_ascii_lowercase().as_str() {
                    "title" => {
                        if let Some(s) = v.as_str() {
                            let trimmed = s.trim();
                            if !trimmed.is_empty() {
                                title_from_fm = Some(trimmed.to_string());
                            }
                        }
                    }
                    "tags" | "tag" | "keywords" => {
                        collect_yaml_tags(v, &mut tags_set);
                    }
                    _ => {}
                }
            }
        }
    }

    extract_inline_tags(body_slice, &mut tags_set);

    let title = title_from_fm
        .or_else(|| extract_first_h1(body_slice))
        .unwrap_or_else(|| strip_md_ext(file_name).to_string());

    let tags: Vec<String> = tags_set.into_iter().collect();
    let tags_text = tags.join(" ");
    let word_count = body_slice.split_whitespace().count().min(u32::MAX as usize) as u32;

    ParsedDocument {
        title,
        tags,
        tags_text,
        frontmatter_json,
        body: body_slice.to_string(),
        word_count,
    }
}

fn strip_md_ext(file_name: &str) -> &str {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".markdown") {
        &file_name[..file_name.len() - 9]
    } else if lower.ends_with(".md") {
        &file_name[..file_name.len() - 3]
    } else {
        file_name
    }
}

fn split_frontmatter(input: &str) -> (Option<&str>, &str) {
    let normalized = input.strip_prefix("\r\n").unwrap_or(input);
    let (delim, after_open) = if let Some(rest) = normalized.strip_prefix("---\r\n") {
        ("---", rest)
    } else if let Some(rest) = normalized.strip_prefix("---\n") {
        ("---", rest)
    } else {
        return (None, input);
    };

    let mut offset = 0usize;
    for line in after_open.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == delim || trimmed == "..." {
            let yaml_part = &after_open[..offset];
            let body_part = &after_open[offset + line.len()..];
            return (Some(yaml_part), body_part);
        }
        offset += line.len();
    }

    (None, input)
}

fn collect_yaml_tags(val: &Yaml, out: &mut BTreeSet<String>) {
    match val {
        Yaml::Array(items) => {
            for item in items {
                if let Some(s) = item.as_str() {
                    for part in s.split(',') {
                        push_tag(part, out);
                    }
                }
            }
        }
        Yaml::String(s) => {
            for part in s.split([',', ' ']) {
                push_tag(part, out);
            }
        }
        _ => {}
    }
}

fn push_tag(raw: &str, out: &mut BTreeSet<String>) {
    let clean = raw.trim().trim_start_matches('#').trim();
    if !clean.is_empty() && clean.len() <= 64 {
        out.insert(clean.to_lowercase());
    }
}

fn extract_first_h1(body: &str) -> Option<String> {
    let mut in_fence = false;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            let heading = rest.trim().trim_end_matches('#').trim();
            if !heading.is_empty() {
                return Some(heading.to_string());
            }
        }
    }
    None
}

fn extract_inline_tags(body: &str, out: &mut BTreeSet<String>) {
    let mut in_fence = false;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }

        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'#' {
                let prev_ok = i == 0 || bytes[i - 1].is_ascii_whitespace();
                if prev_ok && i + 1 < bytes.len() {
                    let rest = &line[i + 1..];
                    let tag_len = rest
                        .bytes()
                        .take_while(|b| {
                            b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-' || *b == b'/'
                        })
                        .count();
                    if tag_len > 0 {
                        let candidate = &rest[..tag_len];
                        // Ignore pure numeric anchors like #123
                        if candidate.bytes().any(|b| b.is_ascii_alphabetic()) {
                            push_tag(candidate, out);
                        }
                        i += 1 + tag_len;
                        continue;
                    }
                }
            }
            i += 1;
        }
    }
}

fn yaml_to_json(yaml: &Yaml) -> Option<serde_json::Value> {
    match yaml {
        Yaml::Real(s) => s
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        Yaml::Integer(i) => Some(serde_json::Value::Number((*i).into())),
        Yaml::String(s) => Some(serde_json::Value::String(s.clone())),
        Yaml::Boolean(b) => Some(serde_json::Value::Bool(*b)),
        Yaml::Array(arr) => Some(serde_json::Value::Array(
            arr.iter().filter_map(yaml_to_json).collect(),
        )),
        Yaml::Hash(map) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in map {
                if let Some(ks) = k.as_str()
                    && let Some(jv) = yaml_to_json(v)
                {
                    obj.insert(ks.to_string(), jv);
                }
            }
            Some(serde_json::Value::Object(obj))
        }
        Yaml::Null => Some(serde_json::Value::Null),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_title_and_tags() {
        let md = "\u{FEFF}---\ntitle: \"Arch Notes\"\ntags: [rust, Tauri, \"#sqlite\"]\n---\n\n# Ignored H1\n\nHello #fts5 world.\n```md\n#not-a-tag\n```\n";
        let parsed = parse_markdown(md.as_bytes(), "arch.md");
        assert_eq!(parsed.title, "Arch Notes");
        assert_eq!(parsed.tags, vec!["fts5", "rust", "sqlite", "tauri"]);
        assert!(parsed.frontmatter_json.is_some());
        assert_eq!(parsed.word_count, 9);
    }

    #[test]
    fn falls_back_gracefully_on_malformed_frontmatter() {
        let md = "---\ntitle: [unclosed\n---\n# Real Heading\nBody text #ok";
        let parsed = parse_markdown(md.as_bytes(), "fallback.md");
        assert_eq!(parsed.title, "Real Heading");
        assert_eq!(parsed.tags, vec!["ok"]);
    }

    #[test]
    fn falls_back_to_filename_without_extension() {
        let md = "Just plain markdown text without heading.";
        let parsed = parse_markdown(md.as_bytes(), "Meeting Notes.markdown");
        assert_eq!(parsed.title, "Meeting Notes");
    }
}
