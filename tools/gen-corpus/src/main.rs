use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process;

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

const TOPICS_ID: &[&str] = &[
    "Rancangan sinkronisasi delta zero-knowledge memakai manifest terenkripsi AES-256-GCM.",
    "Catatan optimasi memori WebView2 agar konsumsi RAM idle tetap di bawah empat puluh megabita.",
    "Evaluasi kueri pencarian FTS5 dengan awalan kata serta pencocokan fuzzy nama berkas.",
    "Panduan rotasi kunci master Argon2id dan verifikasi kunci pemulihan sebelum aktivasi vault.",
    "Ringkasan rapat mingguan mengenai perilaku jendela Dynamic Island di tepi kanan layar.",
];

const TOPICS_EN: &[&str] = &[
    "Zero-knowledge client compression with zstd level 9 plus Padme bucket padding prior to AEAD sealing.",
    "Non-blocking watcher debouncing coalesces burst saves over 200ms while respecting gitignore rules.",
    "Dynamic dock bounds resize cleanly between Collapsed capsule, Peek card, Expanded search, and Wide preview.",
    "OAuth2 loopback PKCE flow requests restricted drive.file scope so Drive UI never exposes plaintext notes.",
    "Incremental conflict detection compares normalized content SHA-256 hashes and keeps non-destructive conflict copies.",
];

struct XorShift64(u64);

impl XorShift64 {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn pick<'a, T>(&mut self, slice: &'a [T]) -> &'a T {
        let idx = (self.next_u64() as usize) % slice.len();
        &slice[idx]
    }
}

fn parse_args() -> (PathBuf, usize) {
    let mut out = PathBuf::from("./corpus");
    let mut count = 5000usize;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => {
                if let Some(v) = args.next() {
                    out = PathBuf::from(v);
                } else {
                    eprintln!("--out requires a directory argument");
                    process::exit(2);
                }
            }
            "--count" => {
                if let Some(v) = args.next() {
                    count = v.parse().unwrap_or_else(|_| {
                        eprintln!("Invalid --count value: {v}");
                        process::exit(2);
                    });
                } else {
                    eprintln!("--count requires a number");
                    process::exit(2);
                }
            }
            "-h" | "--help" => {
                println!("Usage: gen-corpus [--out DIR] [--count N]");
                process::exit(0);
            }
            other => {
                eprintln!("Unknown option: {other}");
                process::exit(2);
            }
        }
    }
    (out, count)
}

fn main() {
    let (out_dir, count) = parse_args();
    if let Err(err) = fs::create_dir_all(&out_dir) {
        eprintln!("Failed to create output dir {}: {err}", out_dir.display());
        process::exit(1);
    }

    let mut rng = XorShift64(0xDEAD_BEEF_2026_0500u64);
    for dir in DIRS {
        if let Err(err) = fs::create_dir_all(out_dir.join(dir)) {
            eprintln!("Failed to create subdir {dir}: {err}");
            process::exit(1);
        }
    }

    for i in 0..count {
        let subdir = *rng.pick(DIRS);
        let tag_a = *rng.pick(TAGS);
        let tag_b = *rng.pick(TAGS);
        let topic_id = *rng.pick(TOPICS_ID);
        let topic_en = *rng.pick(TOPICS_EN);
        let extra_paragraphs = (rng.next_u64() % 4) + 2;

        let rel_path = format!("{subdir}/doc_{i:04}_{tag_a}.md");
        let full_path = out_dir.join(&rel_path);
        let mut file = fs::File::create(&full_path).unwrap_or_else(|e| {
            eprintln!("Failed creating {}: {e}", full_path.display());
            process::exit(1);
        });

        writeln!(
            file,
            "---\ntitle: \"Dokumen #{i:04} - {tag_a} & {tag_b}\"\ntags: [{tag_a}, {tag_b}]\nstatus: active\n---\n"
        )
        .unwrap();
        writeln!(file, "# Note {i:04}: {tag_a} / {tag_b}\n").unwrap();
        writeln!(file, "{topic_id}\n\n{topic_en}\n").unwrap();
        writeln!(
            file,
            "## Implementation Detail\n\n```rust\npub fn compute_doc_{i:04}(input: &[u8]) -> usize {{\n    input.len()\n}}\n```\n"
        )
        .unwrap();

        for p in 0..extra_paragraphs {
            let line_id = *rng.pick(TOPICS_ID);
            let line_en = *rng.pick(TOPICS_EN);
            writeln!(file, "### Section {p}\n- {line_id}\n- {line_en}\n").unwrap();
        }
    }

    println!(
        "Generated {count} deterministic Markdown files in {}",
        out_dir.display()
    );
}
