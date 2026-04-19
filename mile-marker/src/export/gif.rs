//! Animated-GIF exporter.
//!
//! Each session frame becomes one GIF frame, sized identically (the
//! largest frame's dimensions, with shorter ones padded). The default
//! per-frame delay is generous (3s) because the slideshow is meant for
//! reading captions, not animating motion.
//!
//! GIF colour quantisation is by `gif`'s own neuquant; for our typical
//! input (UI screenshots) this gives passable quality without taking a
//! Rust dependency on a heavier encoder.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use anyhow::{Context, Result};
use gif::{Encoder, Frame as GifFrame, Repeat};
use image::{Rgba, RgbaImage};

use crate::composer::{compose_frame, Style};
use crate::session::{downscale_to_width, Session};
use crate::EMBEDDED_FONT;

/// Tunable knobs for [`write_gif`].
#[derive(Debug, Clone)]
pub struct GifOptions {
    /// Maximum width of each output frame in pixels. Larger captures get
    /// downscaled (preserving aspect ratio) before encoding so the GIF
    /// stays under control. 0 = no limit.
    pub max_width_px: u32,
    /// How long each frame is shown, in 1/100ths of a second (the GIF
    /// format's native unit).
    pub delay_centisecs: u16,
    /// Whether to loop the slideshow indefinitely. False = play once.
    pub loop_forever: bool,
    /// Caption banner styling.
    pub style: Style,
}

impl Default for GifOptions {
    fn default() -> Self {
        Self {
            max_width_px: 1280,
            delay_centisecs: 300, // 3 seconds
            loop_forever: true,
            style: Style::default(),
        }
    }
}

/// Encode `session` as an animated GIF at `path`.
///
/// Empty sessions write a no-op file (a valid 1x1 transparent GIF) rather
/// than erroring; callers can decide if that's actionable.
pub fn write_gif(session: &Session, path: &Path, opts: &GifOptions) -> Result<()> {
    let composed: Vec<RgbaImage> = session
        .frames
        .iter()
        .map(|f| {
            let scaled = if opts.max_width_px == 0 {
                f.image.clone()
            } else {
                downscale_to_width(&f.image, opts.max_width_px)
            };
            compose_frame(&scaled, &f.caption, &opts.style, EMBEDDED_FONT)
        })
        .collect();

    let (canvas_w, canvas_h) = composed.iter().fold((1u32, 1u32), |(mw, mh), img| {
        (mw.max(img.width()), mh.max(img.height()))
    });

    let file = File::create(path).with_context(|| format!("creating GIF at {}", path.display()))?;
    let writer = BufWriter::new(file);
    let mut encoder = Encoder::new(writer, canvas_w as u16, canvas_h as u16, &[])
        .context("starting GIF encoder")?;
    encoder
        .set_repeat(if opts.loop_forever {
            Repeat::Infinite
        } else {
            Repeat::Finite(0)
        })
        .context("setting GIF loop mode")?;

    for img in &composed {
        // Pad each frame to the canvas size so the GIF doesn't reflow.
        let padded = pad_to(img, canvas_w, canvas_h, opts.style.background);
        let mut bytes = padded.into_raw();
        let mut gframe =
            GifFrame::from_rgba_speed(canvas_w as u16, canvas_h as u16, &mut bytes, 10);
        gframe.delay = opts.delay_centisecs;
        encoder.write_frame(&gframe).context("writing GIF frame")?;
    }

    Ok(())
}

/// Pad `img` up to `target_w x target_h`, filling with `bg` colour. If
/// the image already matches, returns a clone.
fn pad_to(img: &RgbaImage, target_w: u32, target_h: u32, bg: [u8; 4]) -> RgbaImage {
    if img.width() == target_w && img.height() == target_h {
        return img.clone();
    }
    let mut canvas = RgbaImage::from_pixel(target_w, target_h, Rgba(bg));
    image::imageops::overlay(&mut canvas, img, 0, 0);
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use image::Rgba;

    fn solid(w: u32, h: u32, color: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba(color))
    }

    #[test]
    fn writes_a_non_empty_gif_for_a_real_session() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.gif");

        let mut s = Session::new("demo");
        s.push(solid(64, 32, [10, 20, 30, 255]));
        s.push(solid(64, 32, [200, 100, 50, 255]));
        s.frames[0].caption = "first".into();
        s.frames[1].caption = "second".into();

        write_gif(&s, &path, &GifOptions::default()).expect("write_gif");
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.len() > 100, "GIF too small: {} bytes", bytes.len());
        // GIF magic at the start of the file.
        assert!(
            bytes.starts_with(b"GIF89a") || bytes.starts_with(b"GIF87a"),
            "missing GIF magic"
        );
    }

    #[test]
    fn empty_session_still_writes_a_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.gif");
        let s = Session::new("empty");
        write_gif(&s, &path, &GifOptions::default()).expect("write_gif");
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"GIF"), "missing GIF magic in empty file");
    }
}
