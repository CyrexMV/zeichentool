//! Notizen – ein modernes Notiz- und Zeichenprogramm in Rust (WebAssembly + Android).
//!
//! * `engine`: Notizbücher, Vektor-Tinte, Formerkennung, Vorlagen, Verlauf, Dateiformat, PDF-Export.
//! * `web`: Browser-Anbindung (Canvas, S-Pen, Gesten, Oberfläche, Speicherung) – nur für WebAssembly.

pub mod engine;

#[cfg(target_arch = "wasm32")]
mod web;
