# Dynamic Markdown Dock

A lightweight, right-edge Dynamic Island desktop dock for searching, editing, previewing, organizing, and backing up local Markdown (`.md`) notes with zero-knowledge Google Drive encryption. Built with **Tauri v2**, **Rust**, **React 19**, **Tailwind CSS v4**, and **SQLite FTS5**.

## Key Features

- **Right-Edge Dock Ergonomics:**
  - **Collapsed Tab (`58×98`):** Stays flush against the right screen edge with a folder-and-documents icon, live Vault lock status badge, indexed file counter, and a vertical drag grip to reposition along the edge.
  - **Hover Peek Card (`308×184`):** Expanding hover card showing Vault status (`Unlocked` / `Locked` / `Local`), indexed file & folder counts, sync readiness, and shortcut hint.
  - **Expanded (`460×680`), Wide Split (`1060×760`), & Fullscreen (`F11`):** Smooth spring-animated transitions across folder list, split Markdown workspace, and full work-area editing/reading modes.
- **In-App Folder Organization & File-Only `.md` Drag-Drop:**
  - Group and manage notes in collapsible in-app folders (create, rename, delete, move notes between folders, or toggle visibility).
  - Drag and drop `.md` files directly onto the right-edge dock to preview and upload into a target folder.
- **Editable Raw Mode, Undo/Redo, Explicit Save & Find-in-Doc:**
  - Notes open in **Raw** edit mode by default with explicit save (`Ctrl+S` / `Save` button — no surprise auto-saves) plus **Undo** (`Ctrl+Z`) and **Redo** (`Ctrl+Y` / `Ctrl+Shift+Z`).
  - Toggle cleanly between **Raw** and sanitized **Rendered** Markdown views (`Tab`), copy Markdown/HTML, reveal on disk, or open in an external editor.
  - Built-in **Find in Document** (`Ctrl+F`) with live match counts and highlighted navigation (`Enter` / `Shift+Enter`).
- **Instant Local Search:** Real-time `notify` folder watcher (`200ms` debouncing, `.gitignore`-aware) feeding SQLite FTS5 (`unicode61` prefix + trigram fallback) with virtualized scrolling (`@tanstack/react-virtual`).
- **Zero-Knowledge Encrypted Backup:** Plaintext `.md` notes are compressed (`zstd` level 9 + Padmé size padding) and sealed locally with `Argon2id` + HKDF + `AES-256-GCM` / `ChaCha20-Poly1305` before uploading `.vault` blobs to Google Drive (`drive.file` scope). Includes mandatory 52-character Base32 Recovery Key verification, Vault lock/unlock controls with hover guidance, and non-destructive point-in-time restore.

## Prerequisites

- **Node.js** 22+ & npm
- **Rust** 1.90+ (`rustfmt`, `clippy`)
- **Platform system dependencies:**
  - **Windows 11:** WebView2 Runtime + MSVC Build Tools
  - **Linux X11:** `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev libxdo-dev libssl-dev pkg-config`
  - **macOS:** Xcode Command Line Tools

## Development

```bash
npm install
npm run tauri dev
```

## Google OAuth2 Setup (Loopback PKCE)

1. Create a **Desktop app** OAuth 2.0 Client ID in Google Cloud Console with the restricted `https://www.googleapis.com/auth/drive.file` scope enabled.
2. Enter your OAuth Client ID directly in the Dock **Settings -> Google Drive Backup** panel (PKCE S256 requires no client secret).

## Keyboard Shortcuts

- **Toggle Dock / Focus:** Global hotkey (configurable in Settings)
- `ArrowUp` / `ArrowDown`: Navigate search results
- `Enter`: Open selected note in Wide split editor/preview
- `Tab`: Toggle between **Raw** editor and **Rendered** preview (when preview is open and editor is not focused)
- `Ctrl + F` (or `Cmd + F`): Focus search bar, or open Find-in-Document bar when viewing/editing a note (`Enter` / `Shift + Enter` for next/previous match)
- `Ctrl + S` (or `Cmd + S`): Save changes in Raw editor
- `Ctrl + Z` / `Ctrl + Y` (or `Ctrl + Shift + Z`): Undo / Redo edits in Raw editor
- `Ctrl + C`: Copy selected note Markdown to clipboard (when no text is selected)
- `Ctrl + O`: Open selected note in default external editor
- `F11`: Toggle **Fullscreen** Markdown mode (press `F11` or `Esc` to exit)
- `Esc`: Exit Fullscreen -> Wide, close preview, or collapse dock (unless pinned)

## Verification & Benchmarks

- **Frontend checks & build:**
  ```bash
  npm run typecheck && npm run lint && npm test && npm run build
  ```
- **Formatting check:**
  ```bash
  npm run format:check
  ```
- **Rust checks & tests:**
  ```bash
  cargo fmt --manifest-path src-tauri/Cargo.toml --check
  cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
  cargo test --manifest-path src-tauri/Cargo.toml
  ```
- **5,000-file Corpus Generator:**
  ```bash
  cargo run --manifest-path tools/gen-corpus/Cargo.toml -- --out ./corpus --count 5000
  ```
- **Cold Index & Sub-5ms Search Benchmark:**
  ```bash
  cargo bench --manifest-path src-tauri/Cargo.toml --bench search_bench -- --strict
  ```
- **Offline Vault CLI (`tools/dmd-vault`):**
  ```bash
  cargo run --manifest-path tools/dmd-vault/Cargo.toml -- inspect path/to/blob.vault
  ```
- **Fuzzing Vault Header Parser (requires nightly + `cargo-fuzz`):**
  ```bash
  cd src-tauri && cargo +nightly fuzz run parse_vault_header
  ```
