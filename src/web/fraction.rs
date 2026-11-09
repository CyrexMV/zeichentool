//! Formeln mit Brüchen einfügen: „x = (a+b)/(c-d) + 1/2“ wird zu Text, Bruchstrichen und
//! Zähler/Nenner – als normale Elemente (Text + Linie), damit es überall scharf bleibt, auch im PDF.

use super::*;
use crate::engine::formula::{parse, Seg};
use crate::engine::{Content, PenKind, Pt, Stroke, TextBox};
use web_sys::HtmlInputElement;

fn formula_input() -> HtmlInputElement {
    el("frac-text").unchecked_into()
}

/// Ein platziertes Teil der Formel (Seitenkoordinaten relativ zur Formel).
enum Part {
    Text { x: f32, y: f32, size: f32, text: String, w: f32 },
    Bar { x0: f32, x1: f32, y: f32, width: f32 },
}

/// Formel setzen; liefert Teile und Gesamtgröße.
fn layout(src: &str, size: f32) -> (Vec<Part>, f32, f32) {
    let (_, ctx) = super::editor::new_canvas();
    let measure = |t: &str, s: f32| -> f32 {
        let tb = TextBox { x: 0.0, y: 0.0, w: 0.0, size: s, color: [0; 4], bold: false, text: String::new() };
        ctx.set_font(&super::render::text_font(&tb));
        ctx.measure_text(t).map(|m| m.width() as f32).unwrap_or(t.chars().count() as f32 * s * 0.55)
    };
    let segs = parse(src);
    let has_frac = segs.iter().any(|s| matches!(s, Seg::Frac(..)));
    let fs = if has_frac { size * 0.9 } else { size };
    // Bruchstrich-Höhe; Text steht so, dass seine Mitte auf dem Strich liegt
    let bar_y = if has_frac { fs * 1.32 } else { size * 0.6 };
    let text_top = bar_y - size * 0.62;
    let mut parts = Vec::new();
    let mut x = 0.0f32;
    for s in &segs {
        match s {
            Seg::Text(t) => {
                let w = measure(t, size);
                parts.push(Part::Text { x, y: text_top, size, text: t.clone(), w });
                x += w;
            }
            Seg::Frac(n, d) => {
                let (wn, wd) = (measure(n, fs), measure(d, fs));
                let pad = fs * 0.22;
                let wb = wn.max(wd) + pad * 2.0;
                parts.push(Part::Text { x: x + (wb - wn) * 0.5, y: bar_y - fs * 1.3, size: fs, text: n.clone(), w: wn });
                parts.push(Part::Bar { x0: x + fs * 0.05, x1: x + wb - fs * 0.05, y: bar_y, width: (fs * 0.07).max(0.9) });
                parts.push(Part::Text { x: x + (wb - wd) * 0.5, y: bar_y + fs * 0.12, size: fs, text: d.clone(), w: wd });
                x += wb;
            }
        }
    }
    let h = if has_frac { bar_y + fs * 1.4 } else { size * 1.4 };
    (parts, x.max(1.0), h)
}

impl App {
    pub fn open_fraction_dialog(&mut self) {
        self.close_popovers();
        self.open_modal("modal-frac");
        let inp = formula_input();
        if inp.value().is_empty() {
            inp.set_value("1/2");
        }
        self.update_fraction_preview();
        let _ = inp.focus();
        inp.select();
    }

    /// Vorschau als SVG.
    pub fn update_fraction_preview(&self) {
        let src = formula_input().value();
        let (parts, w, h) = layout(&src, 22.0);
        let mut svg = format!("<svg viewBox=\"-6 -4 {} {}\" xmlns=\"http://www.w3.org/2000/svg\">", w + 12.0, h + 8.0);
        for p in &parts {
            match p {
                Part::Text { x, y, size, text, .. } => {
                    svg.push_str(&format!(
                        "<text x=\"{x:.1}\" y=\"{:.1}\" font-size=\"{size:.1}\" font-family=\"Inter, system-ui, sans-serif\" fill=\"#1c1e2c\" xml:space=\"preserve\">{}</text>",
                        y + size * 0.95,
                        esc(text)
                    ));
                }
                Part::Bar { x0, x1, y, width } => {
                    svg.push_str(&format!("<line x1=\"{x0:.1}\" y1=\"{y:.1}\" x2=\"{x1:.1}\" y2=\"{y:.1}\" stroke=\"#1c1e2c\" stroke-width=\"{width:.2}\" stroke-linecap=\"round\"/>"));
                }
            }
        }
        svg.push_str("</svg>");
        el("frac-preview").set_inner_html(&svg);
    }

    pub fn fraction_action(&mut self, act: &str, arg: &str) {
        let inp = formula_input();
        match act {
            "frac-add" => {
                inp.set_value(&format!("{}{}", inp.value(), arg));
                self.update_fraction_preview();
            }
            "frac-insert" => {
                let src = inp.value();
                if src.trim().is_empty() {
                    self.toast("Formel eingeben, z. B. 3/4 oder (a+b)/(c-d)");
                    return;
                }
                let size = self.prefs.text_size.max(12.0);
                let color = self.ink_color();
                let (parts, w, h) = layout(&src, size);
                let contents: Vec<Content> = parts
                    .into_iter()
                    .filter_map(|p| match p {
                        Part::Text { x, y, size, text, w } => {
                            (!text.trim().is_empty()).then(|| Content::Text(TextBox { x, y, w: w + size * 0.3, size, color, bold: false, text }))
                        }
                        Part::Bar { x0, x1, y, width } => Some(Content::Stroke(Stroke { pen: PenKind::Ball, color, width, pts: vec![Pt::new(x0, y, 1.0), Pt::new(x1, y, 1.0)] })),
                    })
                    .collect();
                self.close_modals();
                self.insert_contents(contents, w, h);
                self.toast("Formel eingefügt – mit dem Lasso verschieben oder vergrößern");
            }
            _ => {}
        }
    }
}
