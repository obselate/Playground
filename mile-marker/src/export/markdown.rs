//! Markdown walkthrough exporter.
//!
//! Produces a directory laid out as:
//!
//! ```text
//! output/
//!   README.md            # the walkthrough
//!   frames/
//!     001.png            # uncomposed screenshots
//!     002.png
//!     ...
//! ```
//!
//! The walkthrough renders each step as a level-2 heading
//! (`## Step N`), the screenshot, and the caption beneath it. We don't
//! burn the caption into the screenshot for this exporter; markdown is
//! editable, and people will want to tweak the words after the fact.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::SecondsFormat;

use crate::session::Session;

/// Write `session` as a Markdown walkthrough rooted at `dir`.
///
/// The directory is created if missing; existing files inside are
/// overwritten without warning.
pub fn write_markdown(session: &Session, dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("creating output dir {}", dir.display()))?;
    let frames_dir = dir.join("frames");
    fs::create_dir_all(&frames_dir)
        .with_context(|| format!("creating frames dir {}", frames_dir.display()))?;

    let readme_path = dir.join("README.md");
    let mut md = File::create(&readme_path)
        .with_context(|| format!("creating {}", readme_path.display()))?;

    writeln!(md, "# {}", escape_md_inline(&session.name))?;
    writeln!(md)?;
    writeln!(
        md,
        "_Recorded {} with [mile-marker](https://github.com/obselate/Playground)._",
        session
            .created_at
            .to_rfc3339_opts(SecondsFormat::Secs, true)
    )?;
    writeln!(md)?;

    if session.frames.is_empty() {
        writeln!(md, "_No frames captured._")?;
        return Ok(());
    }

    for (i, frame) in session.frames.iter().enumerate() {
        let n = i + 1;
        let file = format!("{n:03}.png");
        let path = frames_dir.join(&file);
        frame
            .image
            .save(&path)
            .with_context(|| format!("writing frame {}", path.display()))?;

        writeln!(md, "## Step {n}")?;
        writeln!(md)?;
        writeln!(
            md,
            "![{alt}](frames/{file})",
            alt = escape_md_inline(&format!("Step {n}")),
        )?;
        writeln!(md)?;
        if frame.caption.trim().is_empty() {
            writeln!(md, "_(no caption)_")?;
        } else {
            // Captions are emitted verbatim so writers can use markdown
            // (links, code, emphasis) inside them.
            writeln!(md, "{}", frame.caption)?;
        }
        writeln!(md)?;
    }

    Ok(())
}

/// Escape characters that have inline-markdown meaning so a session name
/// or alt text doesn't accidentally activate formatting. Multi-line
/// captions intentionally do *not* go through this; see the call site.
fn escape_md_inline(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' | '*' | '_' | '[' | ']' | '`' | '<' | '>' | '#' | '|' => {
                out.push('\\');
                out.push(ch);
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn solid(w: u32, h: u32, color: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba(color))
    }

    #[test]
    fn writes_readme_and_frame_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Session::new("Walkthrough");
        s.push(solid(8, 8, [10, 20, 30, 255]));
        s.push(solid(8, 8, [40, 50, 60, 255]));
        s.frames[0].caption = "Open the menu.".into();
        s.frames[1].caption = "Click *Save*.".into();

        write_markdown(&s, dir.path()).unwrap();

        let body = std::fs::read_to_string(dir.path().join("README.md")).unwrap();
        assert!(body.starts_with("# Walkthrough\n"), "title missing: {body}");
        assert!(body.contains("## Step 1"));
        assert!(body.contains("## Step 2"));
        assert!(body.contains("![Step 1](frames/001.png)"));
        assert!(body.contains("Click *Save*."), "caption missing: {body}");
        assert!(dir.path().join("frames/001.png").exists());
        assert!(dir.path().join("frames/002.png").exists());
    }

    #[test]
    fn empty_session_writes_marker_text() {
        let dir = tempfile::tempdir().unwrap();
        let s = Session::new("Empty");
        write_markdown(&s, dir.path()).unwrap();
        let body = std::fs::read_to_string(dir.path().join("README.md")).unwrap();
        assert!(body.contains("No frames captured"), "got {body}");
    }

    #[test]
    fn empty_caption_renders_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Session::new("x");
        s.push(solid(4, 4, [0, 0, 0, 255]));
        write_markdown(&s, dir.path()).unwrap();
        let body = std::fs::read_to_string(dir.path().join("README.md")).unwrap();
        assert!(body.contains("(no caption)"), "got {body}");
    }

    #[test]
    fn name_escaping_blocks_markdown_injection() {
        let dir = tempfile::tempdir().unwrap();
        let s = Session::new("**bad**");
        write_markdown(&s, dir.path()).unwrap();
        let body = std::fs::read_to_string(dir.path().join("README.md")).unwrap();
        assert!(body.contains(r"\*\*bad\*\*"), "title not escaped: {body}");
    }
}
