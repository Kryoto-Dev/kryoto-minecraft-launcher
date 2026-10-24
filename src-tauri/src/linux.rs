//! Linux: how WebKitGTK draws the window, set before it starts.
//!
//! What Kryoto Desktop learned (its display_env.rs): WebKitGTK's DMA-BUF
//! renderer breaks on NVIDIA's driver, on machines without a GPU render node,
//! and inside AppImages whose WebKit disagrees with the system's drivers, and
//! makes pages stutter elsewhere, so it is off. A frameless window moves and
//! resizes itself, which Wayland does not allow a client to do, so the
//! window runs through XWayland. Both are only defaults: a variable the
//! player set themselves always wins. `KRYOTO_GPU=compatible` also turns GPU
//! compositing off, for a machine where the window stays blank.

pub fn apply() {
    let set = |key: &str, value: &str| {
        if std::env::var_os(key).is_none() {
            std::env::set_var(key, value);
        }
    };
    set("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var("XDG_SESSION_TYPE").is_ok_and(|t| t == "wayland");
    if wayland && std::env::var_os("DISPLAY").is_some() {
        set("GDK_BACKEND", "x11");
    }
    if std::env::var("KRYOTO_GPU").is_ok_and(|v| v == "compatible") {
        set("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }
}
