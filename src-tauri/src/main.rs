#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    {
        // Force X11 (XWayland) backend on Linux when an X11 display is available,
        // even if the parent terminal/IDE exported `GDK_BACKEND=wayland`.
        // Wayland compositors (e.g. Ubuntu GNOME Mutter) block global window positioning
        // (`set_position`), always-on-top (`set_keep_above`), global cursor probing, and
        // X11 global shortcuts, which causes the dock to appear in the center or vanish.
        if std::env::var_os("DISPLAY").is_some() {
            // SAFETY: Called on the main thread at process startup before any threads exist.
            unsafe {
                std::env::set_var("GDK_BACKEND", "x11");
            }
            gtk::gdk::set_allowed_backends("x11");
        }

        // Prevent transparent WebKit2GTK windows from failing to render / becoming invisible
        // due to DMABUF compositor issues on Ubuntu/Linux GPU drivers.
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            // SAFETY: Called on the main thread at process startup before any threads exist.
            unsafe {
                std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
            }
        }
    }

    dynamic_markdown_dock_lib::run()
}
