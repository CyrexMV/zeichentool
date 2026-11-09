//! Vorlagen- und Cover-Grundformen als SVG (für Vorschauen und Bibliothekskacheln).

use super::*;
use crate::engine::covers;
use crate::engine::Prim;
use std::cell::RefCell;
use std::fmt::Write as _;

fn rgba(c: [u8; 4]) -> String {
    format!("rgba({},{},{},{:.2})", c[0], c[1], c[2], c[3] as f32 / 255.0)
}

/// SVG-Inhalt (ohne `<svg>`-Hülle) für eine Liste von Grundformen.
pub fn body(prims: Vec<Prim>) -> String {
    let (mut defs, mut fills, mut lines, mut strong, mut labels) = (String::new(), String::new(), String::new(), String::new(), String::new());
    for p in prims {
        match p {
            Prim::Grad { x, y, w, h, c1, c2 } => {
                let id = format!("g{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}-{:.0}-{:.0}", c1[0], c1[1], c1[2], c2[0], c2[1], c2[2], w, h);
                let _ = write!(defs, "<linearGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"><stop offset=\"0\" stop-color=\"{}\"/><stop offset=\"1\" stop-color=\"{}\"/></linearGradient>", x, y, x + w, y + h, rgba(c1), rgba(c2));
                let _ = write!(fills, "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"url(#{id})\"/>", x, y, w, h);
            }
            Prim::Fill { x, y, w, h, r, color } => {
                let _ = write!(fills, "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"{:.1}\" fill=\"{}\"/>", x, y, w, h, r, rgba(color));
            }
            Prim::Circle { x, y, r, color } => {
                let _ = write!(fills, "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.2}\" fill=\"{}\"/>", x, y, r, rgba(color));
            }
            Prim::Seg { x0, y0, x1, y1, w, color, dash } => {
                let _ = write!(
                    fills,
                    "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{}\" stroke-width=\"{:.2}\" stroke-linecap=\"round\"{}/>",
                    x0,
                    y0,
                    x1,
                    y1,
                    rgba(color),
                    w,
                    if dash { format!(" stroke-dasharray=\"{:.1} {:.1}\"", w * 4.0, w * 3.0) } else { String::new() }
                );
            }
            Prim::Poly { pts, w, color } => {
                let d: String = pts.iter().map(|(x, y)| format!("{:.1},{:.1} ", x, y)).collect();
                let _ = write!(fills, "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.2}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>", d, rgba(color), w);
            }
            Prim::Line { x0, y0, x1, y1, strong: st, .. } => {
                let _ = write!(if st { &mut strong } else { &mut lines }, "M{:.1} {:.1}L{:.1} {:.1}", x0, y0, x1, y1);
            }
            Prim::Box { x, y, size } => {
                let _ = write!(strong, "M{:.1} {:.1}h{:.1}v{:.1}h{:.1}Z", x, y, size, size, -size);
            }
            Prim::Label { x, y, size, text, color, bold } => {
                let _ = write!(
                    labels,
                    "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"{:.1}\" font-weight=\"{}\" fill=\"{}\">{}</text>",
                    x,
                    y,
                    size,
                    if bold { 700 } else { 500 },
                    color.map(rgba).unwrap_or_else(|| "rgba(95,115,150,.9)".into()),
                    esc(&text)
                );
            }
            Prim::Dot { .. } => {}
        }
    }
    let mut out = String::new();
    if !defs.is_empty() {
        let _ = write!(out, "<defs>{}</defs>", defs);
    }
    out.push_str(&fills);
    if !lines.is_empty() {
        let _ = write!(out, "<path d=\"{lines}\" fill=\"none\" stroke=\"rgba(120,153,200,.7)\" stroke-width=\".5\" vector-effect=\"non-scaling-stroke\"/>");
    }
    if !strong.is_empty() {
        let _ = write!(out, "<path d=\"{strong}\" fill=\"none\" stroke=\"rgba(100,125,170,.85)\" stroke-width=\".8\" vector-effect=\"non-scaling-stroke\"/>");
    }
    out.push_str(&labels);
    out
}

thread_local! {
    static COVER_CACHE: RefCell<HashMap<(usize, u32), String>> = RefCell::new(HashMap::new());
}

/// Cover als SVG (füllt die Fläche, Seitenverhältnis w:h). Wird zwischengespeichert.
pub fn cover(idx: usize, w: f32, h: f32) -> String {
    let idx = idx % crate::engine::COVERS.len();
    let ratio = ((w / h.max(1.0)) * 1000.0) as u32;
    COVER_CACHE.with(|c| {
        c.borrow_mut()
            .entry((idx, ratio))
            .or_insert_with(|| {
                // in fester Größe rechnen, damit die Muster überall gleich dicht wirken
                let (vw, vh) = (300.0, 300.0 * h / w.max(1.0));
                format!(
                    "<svg class=\"cover-art\" viewBox=\"0 0 {vw:.0} {vh:.0}\" preserveAspectRatio=\"xMidYMid slice\" aria-hidden=\"true\">{}</svg>",
                    body(covers::cover_prims(idx, vw, vh))
                )
            })
            .clone()
    })
}

/// Ist das Cover gemustert (nicht nur ein Farbverlauf)?
pub fn patterned(idx: u8) -> bool {
    covers::pattern(idx as usize) != covers::Pattern::Gradient
}
