use std::path::Path;

use globset::{Glob, GlobSet, GlobSetBuilder};

const DEFAULT_EXCLUDED_DIRS: &[&str] = &[".git", "node_modules", ".trash", "target", ".svn", ".hg"];

#[derive(Clone)]
pub struct ExcludeMatcher {
    glob_set: GlobSet,
}

impl ExcludeMatcher {
    pub fn new(user_globs: &[String]) -> Self {
        let mut builder = GlobSetBuilder::new();
        for pat in user_globs {
            let trimmed = pat.trim();
            if !trimmed.is_empty()
                && let Ok(g) = Glob::new(trimmed)
            {
                builder.add(g);
            }
        }
        let glob_set = builder.build().unwrap_or_else(|_| GlobSet::empty());
        Self { glob_set }
    }

    pub fn is_excluded_rel(&self, rel_path: &Path) -> bool {
        for comp in rel_path.components() {
            if let std::path::Component::Normal(name) = comp {
                let s = name.to_string_lossy();
                if DEFAULT_EXCLUDED_DIRS
                    .iter()
                    .any(|d| d.eq_ignore_ascii_case(&s))
                {
                    return true;
                }
            }
        }
        let normalized = rel_path.to_string_lossy().replace('\\', "/");
        if normalized.starts_with(".obsidian/workspace") {
            return true;
        }
        self.glob_set.is_match(&normalized)
    }
}

pub fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_markdown_and_excludes() {
        assert!(is_markdown_file(Path::new("notes/Hello.MD")));
        assert!(is_markdown_file(Path::new("notes/Spec.markdown")));
        assert!(!is_markdown_file(Path::new("notes/image.png")));

        let matcher = ExcludeMatcher::new(&["**/drafts/**".into(), "*.tmp.md".into()]);
        assert!(matcher.is_excluded_rel(Path::new(".git/HEAD")));
        assert!(matcher.is_excluded_rel(Path::new("sub/node_modules/pkg/README.md")));
        assert!(matcher.is_excluded_rel(Path::new("sub/drafts/idea.md")));
        assert!(matcher.is_excluded_rel(Path::new("scratch.tmp.md")));
        assert!(!matcher.is_excluded_rel(Path::new("notes/architecture.md")));
    }
}
