//! Zeichentool – ein modernes Zeichenprogramm in Rust.
//!
//! * `engine` enthält die komplette, plattformunabhängige Zeichen-Engine
//!   (Pinsel, Ebenen, Mischmodi, Füllen, Verlauf, Projektformat).
//! * `web` verbindet die Engine mit dem Browser (Canvas, S-Pen/Pointer-Events,
//!   Gesten, Oberfläche, Speicherung) und wird nur für WebAssembly gebaut.

pub mod engine;

#[cfg(target_arch = "wasm32")]
mod web;
