//! Paint engine: a pure-Rust raster editor core compiled to WebAssembly by `paint-wasm`.
//!
//! The [`editor::Editor`] owns the document (layers), the history, the current tool and
//! selection, and turns pointer events and commands into pixels. Everything is
//! deterministic and testable without a browser.

pub mod brush;
pub mod codec;
pub mod color;
pub mod document;
pub mod editor;
pub mod fill;
pub mod geom;
pub mod history;
pub mod image;
pub mod raster;
pub mod selection;
pub mod shapes;

pub use brush::BrushKind;
pub use codec::Format;
pub use color::Rgba;
pub use editor::{Button, Editor, EditorState, Mods, Tool};
pub use geom::{IRect, Pt};
pub use image::{Image, Mask, Transform};
pub use shapes::{FillStyle, OutlineStyle, ShapeKind};
