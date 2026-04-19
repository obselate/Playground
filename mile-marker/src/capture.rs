//! Cross-platform screen capture via the [`xcap`] crate.
//!
//! `xcap` enumerates monitors and grabs frames using the appropriate OS
//! API (X11/Wayland on Linux, AVFoundation on macOS, DXGI on Windows).
//! All we expose is a tiny façade that returns an `image::RgbaImage`,
//! plus an enum that makes it easy to ask for a specific monitor.

use anyhow::{anyhow, Context, Result};
use image::RgbaImage;
use xcap::Monitor;

/// Which monitor to capture from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The primary monitor as reported by the OS.
    Primary,
    /// A specific monitor by 0-based index in the order `xcap` returns them.
    Index(usize),
}

/// List the monitors the OS exposes, in the order the rest of this module
/// uses for indexing. Returns the user-facing display name for each.
pub fn list_monitors() -> Result<Vec<String>> {
    let monitors = Monitor::all().context("enumerating monitors")?;
    Ok(monitors.iter().map(|m| m.name().to_string()).collect())
}

/// Take a screenshot from the requested target.
///
/// Returns an `RgbaImage` in the monitor's native pixel dimensions. Errors
/// bubble up from `xcap`, with extra context about which target we tried.
pub fn capture(target: Target) -> Result<RgbaImage> {
    let monitors = Monitor::all().context("enumerating monitors")?;
    if monitors.is_empty() {
        return Err(anyhow!("no monitors detected"));
    }

    let monitor = match target {
        Target::Primary => monitors
            .into_iter()
            .find(|m| m.is_primary())
            .ok_or_else(|| anyhow!("no primary monitor reported"))?,
        Target::Index(i) => monitors
            .into_iter()
            .nth(i)
            .ok_or_else(|| anyhow!("monitor index {i} out of range"))?,
    };

    // `xcap`'s `capture_image` returns an `image::RgbaImage` directly, so
    // we don't need to copy or repack pixels.
    let img = monitor
        .capture_image()
        .with_context(|| format!("capturing from monitor {:?}", target))?;
    Ok(img)
}
