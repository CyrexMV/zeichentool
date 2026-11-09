//! Plattformunabhängige Notiz-Engine (reines Rust, ohne Browser-Abhängigkeiten).

pub mod cal;
pub mod color;
pub mod covers;
pub mod format;
pub mod formula;
pub mod geom;
pub mod history;
pub mod ink;
pub mod note;
pub mod paper;
pub mod pdf;
pub mod planner;
pub mod plot;
pub mod school;
pub mod sha;
pub mod shapes;

pub use color::Color;
pub use geom::{Rect, Xf};
pub use history::{Action, History, PageMeta};
pub use ink::{PenKind, Pt, Smoother, Stroke};
pub use note::{Card, Content, Format, ImageItem, Item, Link, LinkTarget, Notebook, Page, Region, TextBox, COVERS};
pub use paper::{Plan, Prim, Template, TemplateKind};
