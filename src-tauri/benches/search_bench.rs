use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

use dynamic_markdown_dock_lib::indexer::db::IndexDb;
use dynamic_markdown_dock_lib::indexer::search::search_documents;
use dynamic_markdown_dock_lib::watcher::scanner::scan_root;
use tempfile::TempDir;

const CORPUS_SIZE: usize = 5000;

const DIRS: &[&str] = &[
    "journal/2026",
    "engineering/rust",
    "engineering/frontend",
    "notes/security",
    "specs/sync",
    "meetings/weekly",
];

const TAGS: &[&str] = &[
    "rust",
    "tauri",
    "keamanan",
    "catatan",
    "arsitektur",
    "sqlite",
    "fts5",
    "enkripsi",
    "performa",
    "google-drive",
    "rfc",
    "roadmap",
];

const QUERIES: &[&str] = &[
    "enkripsi",
    "sqlite",
    "tag:rust",
    "tag:keamanan",
    "path:engineering/",
    "argon2id zero-knowledge",
    "dokumen",
    "webview2 memory",
    "tag:fts5 performa",
    "",
];

fn generate_corpus(root: &Path, count: usize) {
    for dir in DIRS {
        fs::create_dir_all(root.join(dir)).unwrap();
    }

    let mut state: u64 = 0xFEED_C0DE_5000_2026;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    for i in 0..count {
        let dir = DIRS[(next() as usize) % DIRS.len()];
        let tag_a = TAGS[(next() as usize) % TAGS.len()];
        let tag_b = TAGS[(next() as usize) % TAGS.len()];
        let path = root.join(dir).join(format!("note_{i:04}_{tag_a}.md"));
        let mut f = fs::File::create(&path).unwrap();
        writeln!(
            f,
            "---\ntitle: \"Dokumen #{i:04} - {tag_a}\"\ntags: [{tag_a}, {tag_b}]\n---\n\n# Note {i:04} ({tag_a})\n\nRancangan zero-knowledge vault dengan Argon2id, zstd, serta SQLite FTS5 untuk performa pencarian sub-5ms dan konsumsi memori WebView2 rendah.\n\n```rust\nfn note_{i:04}() -> u64 {{ {i} }}\n```\n\nCatatan tambahan mengenai {tag_b} dan sinkronisasi Google Drive delta.\n"
        )
        .unwrap();
    }
}

fn percentile(sorted_us: &[u128], pct: usize) -> f64 {
    if sorted_us.is_empty() {
        return 0.0;
    }
    let idx = ((sorted_us.len() - 1) * pct) / 100;
    sorted_us[idx] as f64 / 1000.0
}

fn main() {
    let strict = env::args().any(|a| a == "--strict")
        || env::var("DMD_BENCH_STRICT").is_ok_and(|v| v == "1");

    let tmp = TempDir::new().unwrap();
    let corpus_dir = tmp.path().join("corpus");
    let db_path = tmp.path().join("bench.db");

    generate_corpus(&corpus_dir, CORPUS_SIZE);

    let db = IndexDb::open(&db_path).unwrap();
    let root = db
        .add_root(
            corpus_dir.to_string_lossy().to_string(),
            "BenchCorpus".to_string(),
            vec![],
        )
        .unwrap();

    let t_index_start = Instant::now();
    let indexed = scan_root(&db, &root, None).unwrap();
    let index_elapsed = t_index_start.elapsed();
    let index_ms = index_elapsed.as_secs_f64() * 1000.0;
    assert_eq!(
        indexed as usize, CORPUS_SIZE,
        "Expected {CORPUS_SIZE} indexed docs, got {indexed}"
    );

    // Warm up SQLite page cache cleanly with 10 queries, then measure 200 mixed queries.
    for q in QUERIES {
        let _ = search_documents(&db, q, None, 50).unwrap();
    }

    let mut latencies_us = Vec::with_capacity(200);
    for iter in 0..200 {
        let q = QUERIES[iter % QUERIES.len()];
        let t0 = Instant::now();
        let hits = search_documents(&db, q, None, 50).unwrap();
        let us = t0.elapsed().as_micros();
        assert!(hits.len() <= 50);
        latencies_us.push(us);
    }
    latencies_us.sort_unstable();

    let min_ms = latencies_us.first().copied().unwrap_or(0) as f64 / 1000.0;
    let p50_ms = percentile(&latencies_us, 50);
    let p95_ms = percentile(&latencies_us, 95);
    let p99_ms = percentile(&latencies_us, 99);
    let max_ms = latencies_us.last().copied().unwrap_or(0) as f64 / 1000.0;
    let docs_per_sec = CORPUS_SIZE as f64 / index_elapsed.as_secs_f64();

    println!(
        "DMD Benchmark ({CORPUS_SIZE} Markdown files):\n  Cold index : {index_ms:.1} ms ({docs_per_sec:.0} docs/s)\n  Search min : {min_ms:.3} ms\n  Search p50 : {p50_ms:.3} ms\n  Search p95 : {p95_ms:.3} ms\n  Search p99 : {p99_ms:.3} ms\n  Search max : {max_ms:.3} ms"
    );

    db.shutdown();

    if strict {
        assert!(
            index_ms < 5000.0,
            "Cold index exceeded 5000 ms budget: {index_ms:.1} ms"
        );
        assert!(
            p95_ms < 5.0,
            "Search p95 exceeded 5.0 ms budget: {p95_ms:.3} ms"
        );
    }
}
