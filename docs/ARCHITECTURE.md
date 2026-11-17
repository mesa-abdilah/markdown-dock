# Architecture — Dynamic Markdown Dock

## 1. High-Level Overview
Dynamic Markdown Dock pairs a Rust/Tauri v2 native backend with a lightweight React 19 / TypeScript / Tailwind v4 / Motion UI.

```text
+-------------------------------------------------------------------+
|  React UI ("dock" WebviewWindow)                                  |
|  Capsule (40x148) <-> PeekCard (196x148) <-> Expanded (400x640)   |
|                        |                                          |
|                        +--> Wide + PreviewPane (760x720)          |
+----------------------------+--------------------------------------+
                             | tauri-specta typed IPC + Events
+----------------------------v--------------------------------------+
|  Rust Core (dynamic_markdown_dock_lib)                            |
|  - window_manager : monitor selection, DPI math, platform dock    |
|  - watcher        : notify-debouncer-full (200ms) + gitignore     |
|  - indexer        : SQLite WAL + FTS5 unicode61 + trigram + cache |
|  - crypto         : Argon2id -> HKDF -> zstd + Padme + AEAD       |
|  - gdrive_sync    : OAuth2 PKCE + Drive v3 blobs + conflict copy  |
+-------------------------------------------------------------------+
```

## 2. Window State Machine & Anti-Clip Bounds
The Tauri window starts `Collapsed` (`40x148` visual capsule inside `56x164` transparent window with `8dp` shadow pad) anchored to the right work-area edge:
1. **Collapsed (`40x148`):** Non-activating (`WS_EX_NOACTIVATE` on Win32, `set_focusable(false)` on X11/macOS). After 30s idle in `Collapsed`, Windows WebView2 transitions to `COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW`.
2. **Peek (`196x148`, hover dwell 90ms):** Expands horizontally leftwards to show indexed count, vault state, and sync status without showing the file list or stealing OS focus.
3. **Expanded (`400x640`, click or `Alt+Space`):** Enables window activation (`focus`), auto-focuses the search input, and renders virtualized FTS5 search hits via `@tanstack/react-virtual`.
4. **Wide (`760x720`, file selected):** Splits horizontally with a lazy-loaded `PreviewPane` supporting sanitized HTML preview and raw Markdown copy.
5. **DropTarget:** Dragging `.md` files previews immediately with a *Watch Folder* button; dragging folders registers them directly as watched roots.

See [`src-tauri/src/window_manager/geometry.rs`](../src-tauri/src/window_manager/geometry.rs) for pure work-area clamping and physical pixel rounding, and [`docs/VAULT_FORMAT.md`](./VAULT_FORMAT.md) for the binary `.vault` wire specification.

## 3. OS Support Priority
1. **Windows 11 (Primary):** Win32 `SetWindowPos` atomic resize, `WS_EX_TOOLWINDOW` + `WS_EX_NOACTIVATE`, `WM_DISPLAYCHANGE` / `SPI_SETWORKAREA` auto-redock, WebView2 low-memory idle trim.
2. **Linux X11 (Secondary):** GTK/X11 dock setup (`alwaysOnTop`, `skipTaskbar`, `visibleOnAllWorkspaces`, focusable toggle, Tauri cursor probe). Wayland is out of scope in v1.
3. **macOS (Tertiary):** `ActivationPolicy::Accessory` (no Dock icon), multi-space floating window, Tauri cursor probe.

## 4. Storage Layout
- **Local Index Database:** SQLite WAL (`rusqlite`) with `documents`, `tags`, `documents_fts` (FTS5 prefix=2 3), `documents_trgm` (trigram fallback), `sync_queue`, and `sync_conflicts`.
- **Google Drive (`drive.file`):** App folder holding `manifest.vault` (or chunked snapshots), `keyfile.vault`, and opaque `<hmac_hex>.vault` file blobs. Auto-backup runs one-way (`local -> Drive`), paired with explicit **Sync / Pull** and **Point-in-time Restore**.
