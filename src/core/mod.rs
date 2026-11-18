//! Core services shared by every diagram type.
//!
//! The core is deliberately diagram-agnostic. It knows about fonts, pixel
//! geometry, SVG, PNG/PDF and the HTML bundle, but nothing about swimlanes,
//! lifelines or any other diagram vocabulary. That knowledge lives in the
//! diagram-type plugins (see [`crate::plugin`]), which turn a source file
//! into [`html::Section`]s — a key, a label and an SVG document.
//!
//! Because every plugin produces SVG, every plugin gets PNG, PDF and the
//! interactive HTML bundle for free.

pub mod formats;
pub mod geom;
pub mod html;
pub mod overlay;
pub mod svg;
pub mod view;

pub use html::Section;
