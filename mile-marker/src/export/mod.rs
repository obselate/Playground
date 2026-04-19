//! Render a finished [`crate::session::Session`] as an output artifact.
//!
//! Two backends, both pure functions of the session:
//! * [`gif`] writes an animated GIF with caption banners burned in.
//! * [`markdown`] writes a directory of PNG frames plus a README.md
//!   walkthrough that interleaves screenshots and captions.

pub mod gif;
pub mod markdown;
