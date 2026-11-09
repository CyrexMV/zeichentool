//! Sticker und Stempel: eigene Vektor-Motive, die als PNG (mit Transparenz) eingefügt werden –
//! danach ganz normale Bilder: verschieb-, skalier- und drehbar, auch im PDF-Export.

use super::*;
use crate::engine::note::hash_bytes;
use crate::engine::{Content, ImageItem};
use std::rc::Rc;
use web_sys::{CanvasRenderingContext2d, Path2d};

/// Ebene eines Stickers im 100 × 100-Raster.
enum L {
    /// Fläche (SVG-Pfad, Farbe)
    Fill(&'static str, &'static str),
    /// Linie (SVG-Pfad, Farbe, Breite)
    Line(&'static str, &'static str, f64),
    /// Text (Zeichen, x, y, Größe, Farbe)
    Text(&'static str, f64, f64, f64, &'static str),
}

const BURST: &str = "M50 4L59.8 13.3L73 10.2L76.9 23.1L89.8 27L86.7 40.2L96 50L86.7 59.8L89.8 73L76.9 76.9L73 89.8L59.8 86.7L50 96L40.2 86.7L27 89.8L23.1 76.9L10.2 73L13.3 59.8L4 50L13.3 40.2L10.2 27L23.1 23.1L27 10.2L40.2 13.3Z";
const CIRCLE: &str = "M50 8a42 42 0 1 1 0 84a42 42 0 1 1 0-84Z";

const STICKERS: &[(&str, &[L])] = &[
    ("Stern", &[L::Fill("M50 10L61.2 38.6L91.8 40.4L68.1 59.9L75.9 89.6L50 73L24.1 89.6L31.9 59.9L8.2 40.4L38.8 38.6Z", "#ffc531"), L::Fill("M50 22L57 40L50 60L43 40Z", "#ffe08a")]),
    ("Erledigt", &[L::Fill(CIRCLE, "#22b573"), L::Line("M30 51l13 13l27-28", "#ffffff", 10.0)]),
    ("Wichtig", &[L::Fill(CIRCLE, "#f0443a"), L::Text("!", 50.0, 70.0, 58.0, "#ffffff")]),
    ("Frage", &[L::Fill(CIRCLE, "#3d7bf7"), L::Text("?", 50.0, 70.0, 56.0, "#ffffff")]),
    ("Herz", &[L::Fill("M50 88C20 68 8 50 8 33C8 19 19 9 32 9C40 9 46 13 50 20C54 13 60 9 68 9C81 9 92 19 92 33C92 50 80 68 50 88Z", "#ff4f7b"), L::Fill("M27 22c-7 2-10 8-9 14c4-6 8-9 13-10z", "#ff9bb3")]),
    ("Idee", &[L::Fill("M50 8C31 8 18 22 18 39C18 51 25 59 32 65L34 74H66L68 65C75 59 82 51 82 39C82 22 69 8 50 8Z", "#ffd43b"), L::Fill("M35 78H65V85A6 6 0 0 1 59 91H41A6 6 0 0 1 35 85Z", "#8e909c"), L::Fill("M30 36C31 25 38 18 48 16C40 21 36 27 36 37Z", "#fff3b0"), L::Line("M44 74V64M56 74V64", "#e0a000", 3.0)]),
    ("Achtung", &[L::Fill("M50 9L93 86H7Z", "#ffb020"), L::Text("!", 50.0, 79.0, 50.0, "#3a2a00")]),
    ("Pfeil", &[L::Fill("M8 38H52V18L92 50L52 82V62H8Z", "#ff7a1a")]),
    ("Flagge", &[L::Line("M22 10V92", "#5b5d6b", 7.0), L::Fill("M25 12C40 4 52 22 82 12V52C52 62 40 44 25 52Z", "#e8364f")]),
    ("Smiley", &[L::Fill(CIRCLE, "#ffd43b"), L::Fill("M36 34a6 7 0 1 1 0 14a6 7 0 1 1 0-14Z", "#3a2a00"), L::Fill("M64 34a6 7 0 1 1 0 14a6 7 0 1 1 0-14Z", "#3a2a00"), L::Line("M30 60C38 74 62 74 70 60", "#3a2a00", 6.0)]),
    ("Blitz", &[L::Fill("M58 6L18 56H46L38 94L82 40H54Z", "#ffc531")]),
    ("Frist", &[L::Fill(CIRCLE, "#ffffff"), L::Line("M50 12a38 38 0 1 1 0 76a38 38 0 1 1 0-76", "#3d7bf7", 8.0), L::Line("M50 26V50L66 60", "#1e3a8a", 7.0)]),
    ("Pokal", &[L::Line("M30 24H18C18 40 26 46 36 46M70 24H82C82 40 74 46 64 46", "#e0a000", 6.0), L::Fill("M28 12H72V36C72 50 62 60 50 60C38 60 28 50 28 36Z", "#ffc531"), L::Fill("M44 58H56V72H44Z", "#e0a000"), L::Fill("M32 74H68V88H32Z", "#8a5a1a"), L::Text("1", 50.0, 45.0, 24.0, "#ffffff")]),
    ("Buch", &[L::Fill("M14 18C26 14 40 16 50 24C60 16 74 14 86 18V82C74 78 60 80 50 88C40 80 26 78 14 82Z", "#3d7bf7"), L::Line("M50 24V86", "#1e3a8a", 4.0), L::Line("M22 30C30 28 38 30 44 34M22 44C30 42 38 44 44 48M56 34C62 30 70 28 78 30M56 48C62 44 70 42 78 44", "#bcd2ff", 3.5)]),
    ("Neu", &[L::Fill(BURST, "#e8364f"), L::Text("NEU", 50.0, 59.0, 25.0, "#ffffff")]),
    ("Top", &[L::Fill(BURST, "#7c5cff"), L::Text("TOP", 50.0, 59.0, 25.0, "#ffffff")]),
];

/// Stempel: (Text, Farbe)
const STAMPS: &[(&str, &str)] = &[
    ("WICHTIG", "#e0322a"),
    ("PRÜFUNG", "#7c3aed"),
    ("ERLEDIGT ✓", "#17945a"),
    ("HAUSAUFGABE", "#1e5ae6"),
    ("WIEDERHOLEN", "#e8730c"),
    ("MERKEN!", "#0f8f86"),
    ("FRAGE", "#4a5a8a"),
    ("SEHR GUT", "#17945a"),
    ("NACHFRAGEN", "#c2185b"),
    ("ENTWURF", "#6b6e7c"),
    ("KLAUSUR", "#e0322a"),
    ("ABGEGEBEN", "#1e5ae6"),
];

/// Auflösung: Bildpunkte je Rastereinheit.
const RES: f64 = 4.0;

fn png_of(canvas: &web_sys::HtmlCanvasElement) -> Option<(String, Vec<u8>)> {
    let url = canvas.to_data_url_with_type("image/png").ok()?;
    let bin = window().atob(url.split(',').nth(1)?).ok()?;
    Some((url, bin.chars().map(|c| c as u32 as u8).collect()))
}

fn path(d: &str) -> Option<Path2d> {
    Path2d::new_with_path_string(d).ok()
}

/// Sticker zeichnen: weißer Rand mit Schatten, darüber die Ebenen.
fn render_sticker(layers: &[L]) -> Option<(String, Vec<u8>)> {
    let (canvas, ctx) = super::editor::new_canvas();
    let px = (112.0 * RES) as u32;
    canvas.set_width(px);
    canvas.set_height(px);
    ctx.set_transform(RES, 0.0, 0.0, RES, 6.0 * RES, 4.0 * RES).ok()?;
    ctx.set_line_join("round");
    ctx.set_line_cap("round");
    // Aufkleber-Rand
    ctx.set_shadow_color("rgba(20,22,40,0.28)");
    ctx.set_shadow_blur(4.0 * RES);
    ctx.set_shadow_offset_y(1.5 * RES);
    ctx.set_stroke_style_str("#ffffff");
    ctx.set_fill_style_str("#ffffff");
    for l in layers {
        match l {
            L::Fill(d, _) => {
                let p = path(d)?;
                ctx.set_line_width(9.0);
                ctx.stroke_with_path(&p);
                ctx.fill_with_path_2d(&p);
            }
            L::Line(d, _, w) => {
                ctx.set_line_width(w + 9.0);
                ctx.stroke_with_path(&path(d)?);
            }
            L::Text(..) => {}
        }
    }
    ctx.set_shadow_color("rgba(0,0,0,0)");
    ctx.set_shadow_blur(0.0);
    ctx.set_shadow_offset_y(0.0);
    for l in layers {
        match l {
            L::Fill(d, c) => {
                ctx.set_fill_style_str(c);
                ctx.fill_with_path_2d(&path(d)?);
            }
            L::Line(d, c, w) => {
                ctx.set_stroke_style_str(c);
                ctx.set_line_width(*w);
                ctx.stroke_with_path(&path(d)?);
            }
            L::Text(t, x, y, size, c) => {
                ctx.set_fill_style_str(c);
                ctx.set_font(&format!("900 {}px Inter, system-ui, sans-serif", size));
                ctx.set_text_align("center");
                let _ = ctx.fill_text(t, *x, *y);
            }
        }
    }
    png_of(&canvas)
}

/// Stempel zeichnen: doppelter Rahmen, leicht schräg, mit Stempel-Struktur.
fn render_stamp(text: &str, color: &str) -> Option<(String, Vec<u8>, f64, f64)> {
    let (canvas, ctx) = super::editor::new_canvas();
    let font = "800 26px Inter, system-ui, sans-serif";
    ctx.set_font(font);
    let tw = ctx.measure_text(text).ok()?.width() + text.chars().count() as f64 * 2.0;
    let (bw, bh) = (tw + 36.0, 50.0);
    let angle = -5f64.to_radians();
    // Platz für die Drehung
    let (w, h) = (bw * angle.cos().abs() + bh * angle.sin().abs() + 8.0, bw * angle.sin().abs() + bh * angle.cos().abs() + 8.0);
    canvas.set_width((w * RES).ceil() as u32);
    canvas.set_height((h * RES).ceil() as u32);
    let ctx: CanvasRenderingContext2d = ctx;
    ctx.set_transform(RES, 0.0, 0.0, RES, 0.0, 0.0).ok()?;
    ctx.translate(w * 0.5, h * 0.5).ok()?;
    ctx.rotate(angle).ok()?;
    ctx.set_global_alpha(0.92);
    ctx.set_stroke_style_str(color);
    ctx.set_fill_style_str(color);
    ctx.set_line_width(3.5);
    ctx.begin_path();
    let _ = ctx.round_rect_with_f64(-bw * 0.5, -bh * 0.5, bw, bh, 9.0);
    ctx.stroke();
    ctx.set_line_width(1.6);
    ctx.begin_path();
    let _ = ctx.round_rect_with_f64(-bw * 0.5 + 5.0, -bh * 0.5 + 5.0, bw - 10.0, bh - 10.0, 6.0);
    ctx.stroke();
    ctx.set_font(font);
    ctx.set_text_align("center");
    ctx.set_text_baseline("middle");
    // Buchstaben einzeln mit etwas Abstand
    let chars: Vec<String> = text.chars().map(|c| c.to_string()).collect();
    let widths: Vec<f64> = chars.iter().map(|c| ctx.measure_text(c).map(|m| m.width()).unwrap_or(14.0) + 2.0).collect();
    let mut x = -widths.iter().sum::<f64>() * 0.5;
    for (c, cw) in chars.iter().zip(&widths) {
        let _ = ctx.fill_text(c, x + cw * 0.5, 1.5);
        x += cw;
    }
    // Stempel-Struktur: kleine Fehlstellen (immer gleich, damit der Stempel nicht flackert)
    ctx.set_global_alpha(1.0);
    let _ = ctx.set_global_composite_operation("destination-out");
    let mut seed: u32 = text.bytes().fold(2166136261, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed % 10_000) as f64 / 10_000.0
    };
    for _ in 0..(bw * 0.9) as usize {
        let (px, py, r) = ((rnd() - 0.5) * bw, (rnd() - 0.5) * bh, 0.4 + rnd() * 1.3);
        ctx.set_fill_style_str(&format!("rgba(0,0,0,{:.2})", 0.35 + rnd() * 0.55));
        ctx.begin_path();
        let _ = ctx.arc(px, py, r, 0.0, std::f64::consts::TAU);
        ctx.fill();
    }
    let _ = ctx.set_global_composite_operation("source-over");
    let (url, png) = png_of(&canvas)?;
    Some((url, png, w, h))
}

impl App {
    pub fn open_sticker_dialog(&mut self) {
        self.close_popovers();
        let grid = el("sticker-grid");
        if grid.child_element_count() == 0 {
            let mut html = String::new();
            for (k, (name, layers)) in STICKERS.iter().enumerate() {
                if let Some((url, _)) = render_sticker(layers) {
                    html.push_str(&format!("<button class=\"sticker\" data-act=\"sticker\" data-arg=\"s{k}\" title=\"{name}\"><img src=\"{url}\" alt=\"{name}\"></button>"));
                }
            }
            grid.set_inner_html(&html);
            let mut html = String::new();
            for (k, (text, color)) in STAMPS.iter().enumerate() {
                if let Some((url, ..)) = render_stamp(text, color) {
                    html.push_str(&format!("<button class=\"stamp\" data-act=\"sticker\" data-arg=\"t{k}\" title=\"{text}\"><img src=\"{url}\" alt=\"{text}\"></button>"));
                }
            }
            el("stamp-grid").set_inner_html(&html);
        }
        self.open_modal("modal-stickers");
    }

    /// Sticker (`s<n>`) oder Stempel (`t<n>`) einfügen.
    pub fn insert_sticker(&mut self, arg: &str) {
        let Ok(k) = arg[1..].parse::<usize>() else { return };
        let (png, w, h) = if arg.starts_with('s') {
            let Some((_, layers)) = STICKERS.get(k) else { return };
            let Some((_, png)) = render_sticker(layers) else { return };
            (png, 60.0, 60.0)
        } else {
            let Some((text, color)) = STAMPS.get(k) else { return };
            let Some((_, png, w, h)) = render_stamp(text, color) else { return };
            // Stempelhöhe ca. 34 pt
            let s = 34.0 / h as f32 * 1.12;
            (png, w as f32 * s, h as f32 * s)
        };
        self.close_modals();
        let key = hash_bytes(&png);
        let img = ImageItem { x: 0.0, y: 0.0, w, h, png: Rc::new(png), key };
        self.insert_contents(vec![Content::Image(img)], w, h);
    }
}
