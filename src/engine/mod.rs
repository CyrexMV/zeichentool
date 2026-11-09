//! Plattformunabhängige Notiz-Engine (reines Rust, ohne Browser-Abhängigkeiten).

pub mod color;
pub mod format;
pub mod geom;
pub mod history;
pub mod ink;
pub mod note;
pub mod paper;
pub mod pdf;
pub mod shapes;

pub use color::Color;
pub use geom::{Rect, Xf};
pub use history::{Action, History, PageMeta};
pub use ink::{PenKind, Pt, Smoother, Stroke};
pub use note::{Card, Content, Format, ImageItem, Item, Notebook, Page, Region, TextBox, COVERS};
pub use paper::{Prim, Template, TemplateKind};
