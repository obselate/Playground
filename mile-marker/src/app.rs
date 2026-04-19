//! The egui front-end.
//!
//! The window has three regions:
//!
//! ```text
//! +-----------------------------------------------+
//! | toolbar: project name | save/load | exports   |
//! +-----------------------------------------------+
//! | capture controls: capture now | auto + slider |
//! +-----------------------------------------------+
//! | frame strip: thumbnail | caption | meta | rm  |
//! |                                               |
//! +-----------------------------------------------+
//! | status bar                                    |
//! +-----------------------------------------------+
//! ```
//!
//! When auto-capture is on, the app fires a capture every N seconds and
//! then *pauses the timer* until the user presses "Continue" or focuses
//! out of the caption field — that's the namesake "auto-pause-for-
//! caption" behaviour. The next interval starts after the user moves on.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, ColorImage, RichText, TextureHandle, TextureOptions, Vec2};
use image::RgbaImage;

use crate::capture::{self, Target};
use crate::export::{gif, markdown};
use crate::session::Session;

const THUMB_MAX_WIDTH: u32 = 240;
const DEFAULT_INTERVAL_SECS: u32 = 8;

pub struct MileMarkerApp {
    session: Session,
    target: Target,
    monitors: Vec<String>,

    auto_enabled: bool,
    interval_secs: u32,
    next_capture_at: Option<Instant>,
    /// While `Some`, we're paused after a capture waiting for the user to
    /// finish their caption. Holds the frame index they're editing.
    awaiting_caption: Option<usize>,

    project_dir: Option<PathBuf>,
    last_export: Option<PathBuf>,
    status: String,

    /// Lazy texture cache keyed by frame index. Rebuilt on demand when a
    /// thumbnail is needed; cleared whenever the session changes shape
    /// (load, delete, fresh).
    thumbnails: HashMap<usize, TextureHandle>,
}

impl MileMarkerApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let monitors = capture::list_monitors().unwrap_or_default();
        Self {
            session: Session::new("untitled"),
            target: Target::Primary,
            monitors,
            auto_enabled: false,
            interval_secs: DEFAULT_INTERVAL_SECS,
            next_capture_at: None,
            awaiting_caption: None,
            project_dir: None,
            last_export: None,
            status: "ready".into(),
            thumbnails: HashMap::new(),
        }
    }

    fn capture_now(&mut self) {
        match capture::capture(self.target) {
            Ok(img) => {
                let idx = self.session.push(img);
                self.thumbnails.remove(&idx);
                self.status = format!("captured frame {} ({})", idx + 1, self.session.frames.len());
                self.awaiting_caption = Some(idx);
            }
            Err(e) => {
                self.status = format!("capture failed: {e:#}");
            }
        }
    }

    fn schedule_next(&mut self) {
        if self.auto_enabled && self.awaiting_caption.is_none() {
            self.next_capture_at =
                Some(Instant::now() + Duration::from_secs(self.interval_secs as u64));
        } else {
            self.next_capture_at = None;
        }
    }

    fn tick_auto_capture(&mut self, ctx: &egui::Context) {
        // If we're paused on a caption, the timer simply doesn't run.
        if !self.auto_enabled || self.awaiting_caption.is_some() {
            return;
        }

        let now = Instant::now();
        let due = self.next_capture_at.unwrap_or(now);
        if now >= due {
            self.capture_now();
            self.schedule_next();
        }

        // Wake the UI again at the next interesting moment so the
        // countdown stays accurate without us spinning.
        if let Some(t) = self.next_capture_at {
            let remaining = t.saturating_duration_since(Instant::now());
            ctx.request_repaint_after(remaining.min(Duration::from_millis(250)));
        }
    }

    fn ensure_thumbnail(&mut self, ctx: &egui::Context, idx: usize) -> Option<egui::TextureHandle> {
        if let Some(tex) = self.thumbnails.get(&idx) {
            return Some(tex.clone());
        }
        let frame = self.session.frames.get(idx)?;
        let small = crate::session::downscale_to_width(&frame.image, THUMB_MAX_WIDTH);
        let tex = ctx.load_texture(
            format!("frame-{idx}"),
            rgba_to_color_image(&small),
            TextureOptions::LINEAR,
        );
        self.thumbnails.insert(idx, tex.clone());
        Some(tex)
    }

    fn export_gif(&mut self) {
        let path = self.suggest_path("mile-marker.gif");
        match gif::write_gif(&self.session, &path, &gif::GifOptions::default()) {
            Ok(()) => {
                self.last_export = Some(path.clone());
                self.status = format!("wrote GIF: {}", path.display());
            }
            Err(e) => self.status = format!("GIF export failed: {e:#}"),
        }
    }

    fn export_markdown(&mut self) {
        let dir = self.suggest_path("mile-marker-walkthrough");
        match markdown::write_markdown(&self.session, &dir) {
            Ok(()) => {
                self.last_export = Some(dir.clone());
                self.status = format!("wrote walkthrough: {}", dir.display());
            }
            Err(e) => self.status = format!("walkthrough export failed: {e:#}"),
        }
    }

    fn save_session(&mut self) {
        let dir = self.suggest_path("mile-marker-session");
        match self.session.save_to(&dir) {
            Ok(()) => {
                self.project_dir = Some(dir.clone());
                self.status = format!("saved session: {}", dir.display());
            }
            Err(e) => self.status = format!("save failed: {e:#}"),
        }
    }

    fn load_session(&mut self) {
        let Some(dir) = &self.project_dir.clone() else {
            self.status = "no session directory chosen yet (use Save first)".into();
            return;
        };
        match Session::load_from(dir) {
            Ok(loaded) => {
                self.session = loaded;
                self.thumbnails.clear();
                self.awaiting_caption = None;
                self.status = format!("loaded session: {}", dir.display());
            }
            Err(e) => self.status = format!("load failed: {e:#}"),
        }
    }

    /// Where exports / saves go when no folder picker is wired up.
    /// Defaults to the current working directory; users can re-export
    /// after editing the path field if we ever expose one.
    fn suggest_path(&self, leaf: &str) -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(leaf)
    }
}

impl eframe::App for MileMarkerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick_auto_capture(ctx);

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("project:");
                ui.text_edit_singleline(&mut self.session.name);
                ui.separator();
                if ui.button("Save").clicked() {
                    self.save_session();
                }
                if ui.button("Load").clicked() {
                    self.load_session();
                }
                ui.separator();
                if ui.button("Export GIF").clicked() {
                    self.export_gif();
                }
                if ui.button("Export Markdown").clicked() {
                    self.export_markdown();
                }
            });
        });

        egui::TopBottomPanel::top("capture-controls").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new("Capture now").color(Color32::WHITE))
                            .fill(Color32::from_rgb(40, 100, 220)),
                    )
                    .clicked()
                {
                    self.capture_now();
                }

                ui.separator();
                ui.checkbox(&mut self.auto_enabled, "auto");
                ui.add(
                    egui::Slider::new(&mut self.interval_secs, 2..=120)
                        .text("interval (s)")
                        .clamp_to_range(true),
                );

                if self.auto_enabled && self.awaiting_caption.is_none() {
                    if let Some(t) = self.next_capture_at {
                        let remaining = t.saturating_duration_since(Instant::now()).as_secs_f32();
                        ui.label(format!("next in {remaining:>4.1}s"));
                    } else {
                        // Just enabled; arm the timer.
                        self.schedule_next();
                    }
                }
                if self.awaiting_caption.is_some() {
                    ui.colored_label(
                        Color32::from_rgb(220, 180, 60),
                        "paused — finish your caption, then Continue",
                    );
                    if ui.button("Continue").clicked() {
                        self.awaiting_caption = None;
                        self.schedule_next();
                    }
                }

                ui.separator();
                if !self.monitors.is_empty() {
                    ui.label("monitor:");
                    let current_label = match self.target {
                        Target::Primary => "primary".to_string(),
                        Target::Index(i) => self
                            .monitors
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| format!("#{i}")),
                    };
                    egui::ComboBox::from_id_source("monitor")
                        .selected_text(current_label)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.target, Target::Primary, "primary");
                            for (i, name) in self.monitors.iter().enumerate() {
                                ui.selectable_value(
                                    &mut self.target,
                                    Target::Index(i),
                                    format!("{i}: {name}"),
                                );
                            }
                        });
                }
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("frames: {}", self.session.frames.len()));
                ui.separator();
                ui.label(&self.status);
                if let Some(p) = &self.last_export {
                    ui.separator();
                    ui.label(format!("last export: {}", p.display()));
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.session.frames.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        RichText::new("No frames yet. Press Capture now or enable auto.")
                            .size(16.0)
                            .color(Color32::GRAY),
                    );
                });
                return;
            }

            // Iterate over frames by index so we can mutate captions
            // without borrowing the whole session.
            let mut delete_idx: Option<usize> = None;
            egui::ScrollArea::vertical().show(ui, |ui| {
                for i in 0..self.session.frames.len() {
                    self.draw_frame_row(ui, ctx, i, &mut delete_idx);
                    ui.separator();
                }
            });
            if let Some(i) = delete_idx {
                self.session.frames.remove(i);
                // Texture cache is keyed by index; everything shifts so
                // the safe move is to drop the lot and let them rebuild.
                self.thumbnails.clear();
                if self.awaiting_caption == Some(i) {
                    self.awaiting_caption = None;
                    self.schedule_next();
                }
                self.status = format!(
                    "removed frame {} ({} remain)",
                    i + 1,
                    self.session.frames.len()
                );
            }
        });
    }
}

impl MileMarkerApp {
    fn draw_frame_row(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        idx: usize,
        delete_idx: &mut Option<usize>,
    ) {
        let tex = self.ensure_thumbnail(ctx, idx);
        let dims = self
            .session
            .frames
            .get(idx)
            .map(|f| f.image.dimensions())
            .unwrap_or((0, 0));
        let captured_at = self.session.frames.get(idx).map(|f| f.captured_at).unwrap();

        ui.horizontal(|ui| {
            if let Some(tex) = tex {
                let size = tex.size_vec2();
                ui.image((tex.id(), Vec2::new(size.x, size.y)));
            }
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(format!("Step {}", idx + 1))
                        .strong()
                        .size(15.0),
                );
                ui.label(
                    RichText::new(format!(
                        "{}  ·  {}×{}",
                        captured_at.format("%H:%M:%S"),
                        dims.0,
                        dims.1,
                    ))
                    .small()
                    .color(Color32::GRAY),
                );
                if let Some(frame) = self.session.frames.get_mut(idx) {
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut frame.caption)
                            .desired_rows(2)
                            .hint_text("caption…")
                            .desired_width(f32::INFINITY),
                    );
                    if response.lost_focus() && self.awaiting_caption == Some(idx) {
                        // User finished captioning — resume the timer.
                        self.awaiting_caption = None;
                        self.schedule_next();
                    }
                }
                ui.horizontal(|ui| {
                    if ui.small_button("delete").clicked() {
                        *delete_idx = Some(idx);
                    }
                });
            });
        });
    }
}

fn rgba_to_color_image(img: &RgbaImage) -> ColorImage {
    let (w, h) = img.dimensions();
    ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw())
}
