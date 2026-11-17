use std::fs;

use tempfile::TempDir;

use dynamic_markdown_dock_lib::indexer::db::IndexDb;
use dynamic_markdown_dock_lib::indexer::search::{get_snippets, search_documents};
use dynamic_markdown_dock_lib::watcher::scanner::scan_root;

#[test]
fn scans_indexes_searches_and_reconciles_markdown_corpus() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let vault = dir.path().join("notes");
    fs::create_dir_all(vault.join("rust")).unwrap();
    fs::create_dir_all(vault.join(".git")).unwrap();

    fs::write(
        vault.join("rust/tauri-guide.md"),
        "---\ntitle: Tauri Window Guide\ntags: [rust, desktop]\n---\n\n# Ignored\n\nBuilding zero-knowledge apps with SQLite FTS5.",
    )
    .unwrap();
    fs::write(
        vault.join("catatan.md"),
        "# Catatan Rapat\n\nArsitektur enkripsi lokal memakai #argon2id dan zstd.",
    )
    .unwrap();
    // Should be ignored because of .git directory
    fs::write(vault.join(".git/COMMIT_EDITMSG.md"), "ignore me").unwrap();

    let db = IndexDb::open(&db_path).unwrap();
    let root = db
        .add_root(
            fs::canonicalize(&vault)
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            "Notes".into(),
            Vec::new(),
        )
        .unwrap();

    let scanned = scan_root(&db, &root, None).unwrap();
    assert_eq!(scanned, 2);
    assert_eq!(db.count_enabled_documents().unwrap(), 2);

    // Quick-check rescan skips unchanged files
    let rescan = scan_root(&db, &root, None).unwrap();
    assert_eq!(rescan, 0);

    // FTS5 search with special punctuation (must never syntax-error)
    let hits = search_documents(&db, "SQLite (FTS5)!", None, 20).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "Tauri Window Guide");

    // Tag filter search
    let tag_hits = search_documents(&db, "tag:argon2id", None, 20).unwrap();
    assert_eq!(tag_hits.len(), 1);
    assert_eq!(tag_hits[0].file_name, "catatan.md");

    // Fuzzy typo search
    let fuzzy_hits = search_documents(&db, "Turi Windw", None, 20).unwrap();
    assert!(!fuzzy_hits.is_empty());
    assert_eq!(fuzzy_hits[0].title, "Tauri Window Guide");

    // Lazy snippets
    let snips = get_snippets(&db, &[hits[0].doc_id], "SQLite").unwrap();
    assert_eq!(snips.len(), 1);
    assert!(snips[0].html.contains("<mark>"));

    // Verified path resolution
    let (_, resolved) = db
        .resolve_verified_doc_path(i64::from(hits[0].doc_id))
        .unwrap();
    assert!(resolved.ends_with("tauri-guide.md"));

    // Delete file and reconcile
    fs::remove_file(vault.join("catatan.md")).unwrap();
    scan_root(&db, &root, None).unwrap();
    assert_eq!(db.count_enabled_documents().unwrap(), 1);

    // Lookup by rel_path + rename grouping folder updates fuzzy cache/results cleanly
    let found_id = db
        .find_doc_id_by_rel_path(i64::from(root.id), "rust/tauri-guide.md")
        .unwrap();
    assert_eq!(found_id, Some(hits[0].doc_id));
    db.rename_root(i64::from(root.id), "Engineering".into())
        .unwrap();
    let renamed_hits = search_documents(&db, "Tauri", Some(i64::from(root.id)), 10).unwrap();
    assert_eq!(renamed_hits.len(), 1);
    assert_eq!(renamed_hits[0].root_label, "Engineering");

    db.shutdown();
}
