use std::sync::RwLock;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

#[derive(Debug, Clone)]
pub struct DocMeta {
    pub id: i64,
    pub root_id: i64,
    pub file_name: String,
    pub title: String,
    pub tags_text: String,
    pub rel_path: String,
    pub mtime_ns: i64,
}

#[derive(Default)]
pub struct FuzzyCache {
    items: RwLock<Vec<DocMeta>>,
}

impl FuzzyCache {
    pub fn replace_all(&self, docs: Vec<DocMeta>) {
        if let Ok(mut guard) = self.items.write() {
            *guard = docs;
        }
    }

    pub fn search(&self, query: &str, root_filter: Option<i64>, limit: usize) -> Vec<(i64, u32)> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let guard = match self.items.read() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };

        let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
        let pattern = Pattern::parse(trimmed, CaseMatching::Ignore, Normalization::Smart);
        let mut buf = Vec::new();
        let mut scored: Vec<(i64, u32)> = Vec::new();

        for doc in guard.iter() {
            if let Some(rf) = root_filter
                && doc.root_id != rf
            {
                continue;
            }
            let title_score = pattern
                .score(Utf32Str::new(&doc.title, &mut buf), &mut matcher)
                .map(|s| s.saturating_mul(2));
            let name_score = pattern.score(Utf32Str::new(&doc.file_name, &mut buf), &mut matcher);
            let path_score = pattern
                .score(Utf32Str::new(&doc.rel_path, &mut buf), &mut matcher)
                .map(|s| s / 2);

            let best = [title_score, name_score, path_score]
                .into_iter()
                .flatten()
                .max();
            if let Some(score) = best {
                scored.push((doc.id, score));
            }
        }

        scored.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        scored.truncate(limit);
        scored
    }
}
