//! In-memory model of a capture session.
//!
//! A [`Session`] is a list of [`Frame`]s plus some metadata. Frames are
//! held in memory as raw RGBA buffers so the UI can show thumbnails and
//! the exporters can re-encode them without re-reading from disk. Sessions
//! serialise to JSON with frames written out as adjacent PNG files; we
//! intentionally don't base64-embed image bytes in JSON so the on-disk
//! form stays diff-friendly.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use image::RgbaImage;
use serde::{Deserialize, Serialize};

/// A single captured frame plus its caption.
#[derive(Debug, Clone)]
pub struct Frame {
    pub captured_at: DateTime<Utc>,
    pub caption: String,
    pub image: RgbaImage,
}

/// A capture session.
#[derive(Debug, Clone)]
pub struct Session {
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub frames: Vec<Frame>,
}

impl Session {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            created_at: Utc::now(),
            frames: Vec::new(),
        }
    }

    /// Append a frame with an empty caption. Returns the new index so the
    /// UI can immediately scroll to it.
    pub fn push(&mut self, image: RgbaImage) -> usize {
        let frame = Frame {
            captured_at: Utc::now(),
            caption: String::new(),
            image,
        };
        self.frames.push(frame);
        self.frames.len() - 1
    }

    /// Save the session to ``dir``: a `session.json` manifest plus one
    /// PNG per frame named `frame-NNN.png`. The directory is created if
    /// needed; existing frame files are overwritten.
    pub fn save_to(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(dir)
            .with_context(|| format!("creating session dir {}", dir.display()))?;

        let mut manifest = SessionManifest {
            version: SESSION_FORMAT_VERSION,
            name: self.name.clone(),
            created_at: self.created_at,
            frames: Vec::with_capacity(self.frames.len()),
        };

        for (i, frame) in self.frames.iter().enumerate() {
            let file = format!("frame-{i:04}.png");
            let path = dir.join(&file);
            frame
                .image
                .save(&path)
                .with_context(|| format!("writing frame to {}", path.display()))?;
            manifest.frames.push(FrameManifest {
                file,
                captured_at: frame.captured_at,
                caption: frame.caption.clone(),
            });
        }

        let json = serde_json::to_string_pretty(&manifest).context("serialising manifest")?;
        let manifest_path = dir.join("session.json");
        fs::write(&manifest_path, json)
            .with_context(|| format!("writing manifest to {}", manifest_path.display()))?;
        Ok(())
    }

    /// Load a session previously written by [`Session::save_to`].
    pub fn load_from(dir: &Path) -> Result<Self> {
        let manifest_path = dir.join("session.json");
        let json = fs::read_to_string(&manifest_path)
            .with_context(|| format!("reading manifest {}", manifest_path.display()))?;
        let manifest: SessionManifest =
            serde_json::from_str(&json).context("parsing manifest JSON")?;

        if manifest.version != SESSION_FORMAT_VERSION {
            anyhow::bail!(
                "unsupported session format version {} (this build understands {})",
                manifest.version,
                SESSION_FORMAT_VERSION
            );
        }

        let mut frames = Vec::with_capacity(manifest.frames.len());
        for fm in manifest.frames {
            let path = dir.join(&fm.file);
            let img = image::open(&path)
                .with_context(|| format!("opening frame {}", path.display()))?
                .to_rgba8();
            frames.push(Frame {
                captured_at: fm.captured_at,
                caption: fm.caption,
                image: img,
            });
        }

        Ok(Session {
            name: manifest.name,
            created_at: manifest.created_at,
            frames,
        })
    }

    /// Convenience: pre-allocate a thumbnail of `frame_index` at the
    /// requested max width, preserving aspect ratio. Used by the GUI for
    /// the strip view.
    pub fn thumbnail(&self, frame_index: usize, max_width: u32) -> Option<RgbaImage> {
        let frame = self.frames.get(frame_index)?;
        Some(downscale_to_width(&frame.image, max_width))
    }
}

/// Scale an RGBA image down so its width is at most `max_width`, keeping
/// aspect ratio. If the source is already small enough we return a clone.
pub fn downscale_to_width(src: &RgbaImage, max_width: u32) -> RgbaImage {
    let (w, h) = src.dimensions();
    if w <= max_width {
        return src.clone();
    }
    let new_w = max_width;
    let new_h = ((h as f64) * (max_width as f64) / (w as f64))
        .round()
        .max(1.0) as u32;
    image::imageops::resize(src, new_w, new_h, image::imageops::FilterType::Triangle)
}

const SESSION_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct SessionManifest {
    version: u32,
    name: String,
    created_at: DateTime<Utc>,
    frames: Vec<FrameManifest>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FrameManifest {
    file: String,
    captured_at: DateTime<Utc>,
    caption: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn solid(w: u32, h: u32, color: [u8; 4]) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        for px in img.pixels_mut() {
            *px = Rgba(color);
        }
        img
    }

    #[test]
    fn push_returns_index_and_appends() {
        let mut s = Session::new("test");
        assert_eq!(s.push(solid(2, 2, [255, 0, 0, 255])), 0);
        assert_eq!(s.push(solid(2, 2, [0, 255, 0, 255])), 1);
        assert_eq!(s.frames.len(), 2);
    }

    #[test]
    fn round_trip_preserves_frames_and_captions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut s = Session::new("demo");
        s.push(solid(8, 8, [10, 20, 30, 255]));
        s.push(solid(8, 8, [200, 200, 200, 255]));
        s.frames[0].caption = "hello".into();
        s.frames[1].caption = "world".into();

        s.save_to(dir.path()).expect("save");
        let loaded = Session::load_from(dir.path()).expect("load");
        assert_eq!(loaded.name, "demo");
        assert_eq!(loaded.frames.len(), 2);
        assert_eq!(loaded.frames[0].caption, "hello");
        assert_eq!(loaded.frames[1].caption, "world");
        assert_eq!(loaded.frames[0].image.dimensions(), (8, 8));
    }

    #[test]
    fn downscale_keeps_aspect_ratio() {
        let src = solid(800, 400, [0, 0, 0, 255]);
        let out = downscale_to_width(&src, 400);
        assert_eq!(out.dimensions(), (400, 200));
    }

    #[test]
    fn downscale_passes_through_when_small_enough() {
        let src = solid(50, 50, [0, 0, 0, 255]);
        let out = downscale_to_width(&src, 400);
        assert_eq!(out.dimensions(), (50, 50));
    }

    #[test]
    fn loading_unknown_version_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("session.json"),
            r#"{"version":99,"name":"x","created_at":"2020-01-01T00:00:00Z","frames":[]}"#,
        )
        .unwrap();
        let err = Session::load_from(dir.path()).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("unsupported session format"), "got {msg}");
    }
}
