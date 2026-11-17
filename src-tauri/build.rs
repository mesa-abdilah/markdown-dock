use std::fs;
use std::path::Path;

const ALLOWED_KEYS: &[&str] = &["DMD_GOOGLE_CLIENT_ID", "DMD_GOOGLE_CLIENT_SECRET"];

fn strip_quotes(val: &str) -> &str {
    let trimmed = val.trim();
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    }
}

fn load_dotenv(path: &Path) {
    let Ok(contents) = fs::read_to_string(path) else {
        return;
    };
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !ALLOWED_KEYS.contains(&key) {
            continue;
        }
        if std::env::var_os(key).is_some() {
            continue;
        }
        let cleaned = strip_quotes(val);
        if !cleaned.is_empty() && !cleaned.contains(['\r', '\n']) {
            println!("cargo:rustc-env={key}={cleaned}");
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=../.env");
    println!("cargo:rerun-if-changed=.env");
    for key in ALLOWED_KEYS {
        println!("cargo:rerun-if-env-changed={key}");
    }

    // Prefer root .env, then fallback to src-tauri/.env if keys are still unset.
    load_dotenv(Path::new("../.env"));
    load_dotenv(Path::new(".env"));

    tauri_build::build()
}
