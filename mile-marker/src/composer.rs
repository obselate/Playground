//! Burn caption banners onto captured frames.
//!
//! Output frames look like this (cross-section):
//!
//! ```text
//! +-----------------------------+
//! |                             |
//! |        captured pixels      |
//! |                             |
//! +-----------------------------+
//! |  caption text on a strip    |  <-- banner, fixed pixel height
//! +-----------------------------+
//! ```
//!
//! The banner sits below the captured pixels rather than overlaying them,
//! so the screenshot is never obscured by text. Banner height scales with
//! the chosen font size; multi-line captions wrap to fit the frame width.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;

/// Banner appearance.
#[derive(Debug, Clone)]
pub struct Style {
    pub font_px: f32,
    pub padding_px: u32,
    pub background: [u8; 4],
    pub foreground: [u8; 4],
}

impl Default for Style {
    fn default() -> Self {
        Self {
            font_px: 28.0,
            padding_px: 16,
            // Slightly translucent dark grey so the seam between
            // screenshot and banner is visible but not jarring.
            background: [22, 22, 28, 255],
            foreground: [240, 240, 240, 255],
        }
    }
}

/// Compose a captioned frame: original image on top, caption banner
/// below. Returns a fresh `RgbaImage` whose width matches the input and
/// whose height grows by however much the banner needs.
///
/// `font_bytes` should be a TTF/OTF buffer — typically
/// [`crate::EMBEDDED_FONT`].
pub fn compose_frame(
    src: &RgbaImage,
    caption: &str,
    style: &Style,
    font_bytes: &[u8],
) -> RgbaImage {
    let font = FontRef::try_from_slice(font_bytes).expect("embedded font is valid");
    let (w, h) = src.dimensions();

    // Wrap the caption to the available pixel width, then size the banner
    // to fit.
    let inner_w = w.saturating_sub(2 * style.padding_px).max(1);
    let lines = wrap_caption(caption, &font, style.font_px, inner_w);
    let line_height = line_height_px(&font, style.font_px);
    let banner_inner_h = (lines.len() as u32) * line_height;
    let banner_h = banner_inner_h + 2 * style.padding_px;

    let mut out = RgbaImage::from_pixel(w, h + banner_h, Rgba(style.background));

    // Stamp the original screenshot into the top region.
    image::imageops::overlay(&mut out, src, 0, 0);

    // Draw caption lines into the banner.
    let scale = PxScale::from(style.font_px);
    for (i, line) in lines.iter().enumerate() {
        let x = style.padding_px as i32;
        let y = (h + style.padding_px + (i as u32) * line_height) as i32;
        draw_text_mut(&mut out, Rgba(style.foreground), x, y, scale, &font, line);
    }

    out
}

/// Greedy word-wrap to a pixel width.
///
/// Words longer than `max_px` on their own are still emitted on a line by
/// themselves rather than truncated, since silent truncation in a
/// caption would be a footgun.
fn wrap_caption(text: &str, font: &FontRef<'_>, font_px: f32, max_px: u32) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let scale = PxScale::from(font_px);
    let scaled = font.as_scaled(scale);
    let space_w = advance_width(&scaled, ' ');

    let mut lines: Vec<String> = Vec::new();
    for paragraph in text.split('\n') {
        let mut current = String::new();
        let mut current_w = 0.0_f32;
        for word in paragraph.split_whitespace() {
            let word_w = word.chars().map(|c| advance_width(&scaled, c)).sum::<f32>();
            let needed = if current.is_empty() {
                word_w
            } else {
                current_w + space_w + word_w
            };
            if needed <= max_px as f32 || current.is_empty() {
                if !current.is_empty() {
                    current.push(' ');
                    current_w += space_w;
                }
                current.push_str(word);
                current_w += word_w;
            } else {
                lines.push(std::mem::take(&mut current));
                current.push_str(word);
                current_w = word_w;
            }
        }
        lines.push(current);
    }
    lines
}

fn advance_width<F: Font>(scaled: &ab_glyph::PxScaleFont<&F>, c: char) -> f32 {
    let id = scaled.glyph_id(c);
    scaled.h_advance(id)
}

fn line_height_px(_font: &FontRef<'_>, font_px: f32) -> u32 {
    // Approximate line height as 1.4 × font size; ab_glyph's metric APIs
    // expose ascent/descent but for our banner-style captions this looks
    // tidier than the strict typographic line height.
    ((font_px * 1.4).round() as u32).max(font_px as u32 + 4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EMBEDDED_FONT;

    fn solid(w: u32, h: u32, color: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba(color))
    }

    #[test]
    fn output_width_matches_input() {
        let src = solid(640, 480, [255, 255, 255, 255]);
        let out = compose_frame(&src, "hi", &Style::default(), EMBEDDED_FONT);
        assert_eq!(out.width(), 640);
        assert!(out.height() > 480, "banner should add height");
    }

    #[test]
    fn empty_caption_still_yields_a_banner() {
        let src = solid(320, 240, [0, 0, 0, 255]);
        let out = compose_frame(&src, "", &Style::default(), EMBEDDED_FONT);
        // Banner takes at least padding * 2 + one line height.
        assert!(out.height() > 240);
    }

    #[test]
    fn long_word_does_not_panic() {
        let src = solid(200, 100, [255, 255, 255, 255]);
        let out = compose_frame(
            &src,
            "supercalifragilisticexpialidocious",
            &Style::default(),
            EMBEDDED_FONT,
        );
        assert_eq!(out.width(), 200);
    }

    #[test]
    fn multiline_caption_wraps_to_more_lines() {
        let src = solid(300, 100, [255, 255, 255, 255]);
        let style = Style {
            font_px: 28.0,
            padding_px: 8,
            ..Style::default()
        };
        let one = compose_frame(&src, "short", &style, EMBEDDED_FONT);
        let many = compose_frame(
            &src,
            "this is a noticeably longer caption that should wrap onto multiple lines when rendered at this font size into a narrow frame",
            &style,
            EMBEDDED_FONT,
        );
        assert!(
            many.height() > one.height(),
            "wrapped caption should be taller"
        );
    }

    #[test]
    fn explicit_newlines_force_breaks() {
        let lines = wrap_caption(
            "a\nb\nc",
            &FontRef::try_from_slice(EMBEDDED_FONT).unwrap(),
            24.0,
            1000,
        );
        assert_eq!(lines, vec!["a".to_string(), "b".into(), "c".into()]);
    }

    #[test]
    fn screenshot_pixels_survive_in_output() {
        // The very first row should be the screenshot's top edge,
        // unchanged by the composer.
        let src = solid(10, 10, [123, 45, 67, 255]);
        let out = compose_frame(&src, "hello", &Style::default(), EMBEDDED_FONT);
        let p = out.get_pixel(0, 0);
        assert_eq!(*p, Rgba([123, 45, 67, 255]));
    }
}
