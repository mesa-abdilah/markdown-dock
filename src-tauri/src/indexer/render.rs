use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use comrak::{Options, markdown_to_html};

const MAX_CACHE_ENTRIES: usize = 256;

struct CachedHtml {
    sha256: [u8; 32],
    html: String,
    last_used: u64,
}

#[derive(Default)]
pub struct RenderCache {
    entries: Mutex<HashMap<i64, CachedHtml>>,
    clock: Mutex<u64>,
}

impl RenderCache {
    pub fn render_or_cached(&self, doc_id: i64, sha256: &[u8; 32], raw_markdown: &str) -> String {
        {
            let mut map = self.entries.lock().unwrap();
            if let Some(entry) = map.get_mut(&doc_id)
                && &entry.sha256 == sha256
            {
                let mut clk = self.clock.lock().unwrap();
                *clk += 1;
                entry.last_used = *clk;
                return entry.html.clone();
            }
        }

        let html = render_safe_html(raw_markdown);

        {
            let mut map = self.entries.lock().unwrap();
            if map.len() >= MAX_CACHE_ENTRIES
                && let Some(&evict_id) = map
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_used)
                    .map(|(k, _)| k)
            {
                map.remove(&evict_id);
            }
            let mut clk = self.clock.lock().unwrap();
            *clk += 1;
            map.insert(
                doc_id,
                CachedHtml {
                    sha256: *sha256,
                    html: html.clone(),
                    last_used: *clk,
                },
            );
        }

        html
    }

    pub fn invalidate(&self, doc_id: i64) {
        self.entries.lock().unwrap().remove(&doc_id);
    }
}

pub fn render_safe_html(markdown: &str) -> String {
    let mut opts = Options::default();
    opts.extension.strikethrough = true;
    opts.extension.table = true;
    opts.extension.autolink = true;
    opts.extension.tasklist = true;
    opts.extension.footnotes = true;
    opts.render.r#unsafe = false;

    let raw_html = markdown_to_html(markdown, &opts);

    let tags: HashSet<&'static str> = [
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "p",
        "br",
        "hr",
        "ul",
        "ol",
        "li",
        "blockquote",
        "pre",
        "code",
        "table",
        "thead",
        "tbody",
        "tr",
        "th",
        "td",
        "a",
        "strong",
        "em",
        "del",
        "sup",
        "sub",
        "input",
        "span",
        "div",
        "section",
    ]
    .into_iter()
    .collect();

    let mut cleaner = ammonia::Builder::new();
    cleaner
        .tags(tags)
        .add_tag_attributes("input", &["type", "checked", "disabled"])
        .add_tag_attributes("a", &["href", "title"])
        .add_tag_attributes("code", &["class"])
        .add_tag_attributes("pre", &["class"])
        .add_tag_attributes("th", &["align"])
        .add_tag_attributes("td", &["align"])
        .url_schemes(["http", "https", "mailto"].into_iter().collect())
        .link_rel(Some("noopener noreferrer"));

    cleaner.clean(&raw_html).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_scripts_and_remote_images() {
        let md = "# Hello\n\n<script>alert(1)</script>\n\n![img](https://evil.example/track.png)\n\n[link](https://example.com)";
        let html = render_safe_html(md);
        assert!(!html.contains("<script"));
        assert!(!html.contains("alert(1)"));
        assert!(!html.contains("<img"));
        assert!(html.contains("rel=\"noopener noreferrer\""));
    }
}
