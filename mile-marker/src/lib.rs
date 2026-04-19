//! mile-marker: record-and-narrate-as-you-go screen capture.
//!
//! The library is split so the GUI is a thin layer over headless,
//! testable building blocks:
//!
//! * [`session`]: in-memory model of a capture session (frames, captions,
//!   metadata) plus JSON persistence.
//! * [`capture`]: cross-platform screen-capture wrapper around `xcap`.
//! * [`composer`]: burn caption banners onto captured frames using a
//!   bundled font.
//! * [`export`]: render a finished session as either an animated GIF or
//!   a Markdown walkthrough with PNG frames.
//!
//! `main.rs` only wires these together inside an `eframe` window.

pub mod app;
pub mod capture;
pub mod composer;
pub mod export;
pub mod session;

/// The bundled UI font used for caption overlays burned into output frames.
///
/// We embed the bytes at compile time so the binary is self-contained and
/// works on machines without DejaVu Sans installed (notably fresh Windows
/// installs). The font is permissively licensed; see
/// `assets/font.LICENSE`.
pub const EMBEDDED_FONT: &[u8] = include_bytes!("../assets/font.ttf");
