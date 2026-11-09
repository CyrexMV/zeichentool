//! Rendering: Seiten (mit Bild-Zwischenspeicher je Zoomstufe), Tinte als Path2D,
//! Vorlagen, Text, Bilder, Auswahl, Live-Strich und Laserpointer.

use super::editor::{new_canvas, Editor, Op, PageBmp, GAP};
use super::*;
use crate::engine::geom::{Rect, Xf};
use crate::engine::paper::{line_colors, Prim};
use crate::engine::{Content, Page, PenKind, Stroke, TemplateKind, TextBox};
use std::fmt::Write as _;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, Path2d};

pub const FONT: &str = "\"Inter\", system-ui, -apple-system, \"Segoe UI\", Roboto, Helvetica, Arial, sans-serif";

fn fp_stroke(s: &Stroke) -> u64 {
    let mut h: u64 = 1469598103934665603;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(1099511628211);
    };
    mix(s.pts.len() as u64);
    mix(s.width.to_bits() as u64);
    mix(u32::from_le_bytes(s.color) as u64);
    mix(s.pen as u64);
    for i in [0, s.pts.len() / 2, s.pts.len().saturating_sub(1)] {
        if let Some(p) = s.pts.get(i) {
            mix(p.x.to_bits() as u64);
            mix(p.y.to_bits() as u64);
            mix(p.p.to_bits() as u64);
        }
    }
    h
}

pub fn outline_path(s: &Stroke) -> Path2d {
    let o = s.outline();
    let mut d = String::with_capacity(o.len() * 14);
    for (i, (x, y)) in o.iter().enumerate() {
        let _ = write!(d, "{}{:.2} {:.2}", if i == 0 { "M" } else { "L" }, x, y);
    }
    d.push('Z');
    Path2d::new_with_path_string(&d).unwrap_or_else(|_| Path2d::new().unwrap())
}

pub fn text_font(t: &TextBox) -> String {
    format!("{} {}px {}", if t.bold { 650 } else { 420 }, t.size, FONT)
}

/// Bricht Text in Zeilen um (Messung mit dem Canvas).
pub fn wrap_text(ctx: &CanvasRenderingContext2d, t: &TextBox) -> Vec<String> {
    ctx.set_font(&text_font(t));
    let width = |s: &str| ctx.measure_text(s).map(|m| m.width()).unwrap_or(0.0) as f32;
    let mut lines = Vec::new();
    for para in t.text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let cand = if line.is_empty() { word.to_string() } else { format!("{} {}", line, word) };
            if width(&cand) <= t.w || line.is_empty() {
                if width(&cand) > t.w && line.is_empty() {
                    // sehr langes Wort zeichenweise umbrechen
                    let mut part = String::new();
                    for ch in word.chars() {
                        let next = format!("{}{}", part, ch);
                        if width(&next) > t.w && !part.is_empty() {
                            lines.push(std::mem::take(&mut part));
                        }
                        part.push(ch);
                    }
                    line = part;
                } else {
                    line = cand;
                }
            } else {
                lines.push(std::mem::replace(&mut line, word.to_string()));
            }
        }
        lines.push(line);
    }
    lines
}

fn fp_text(t: &TextBox) -> u64 {
    crate::engine::note::hash_bytes(format!("{}|{}|{}|{}", t.text, t.w, t.size, t.bold).as_bytes())
}

/// Zeichen-Zwischenspeicher, gemeinsam für Ansicht, Vorschaubilder und Export.
/// Vorlagen-Zwischenspeicher: Linienpfade + Farbflächen/Beschriftungen.
pub type TplCache = HashMap<String, (Vec<(u8, Path2d)>, Vec<Prim>)>;

pub struct Caches<'a> {
    pub paths: &'a mut HashMap<u32, (u64, Path2d)>,
    pub tpl: &'a mut TplCache,
    pub wraps: &'a mut HashMap<u32, (u64, Vec<String>)>,
    pub images: &'a mut HashMap<u64, HtmlImageElement>,
}

impl Caches<'_> {
    fn path(&mut self, id: u32, s: &Stroke) -> Path2d {
        let f = fp_stroke(s);
        if let Some((k, p)) = self.paths.get(&id) {
            if *k == f {
                return p.clone();
            }
        }
        let p = outline_path(s);
        self.paths.insert(id, (f, p.clone()));
        p
    }

    pub fn lines(&mut self, ctx: &CanvasRenderingContext2d, id: u32, t: &TextBox) -> Vec<String> {
        let f = fp_text(t);
        if let Some((k, l)) = self.wraps.get(&id) {
            if *k == f {
                return l.clone();
            }
        }
        let l = wrap_text(ctx, t);
        self.wraps.insert(id, (f, l.clone()));
        l
    }
}

/// Bild aus PNG-Daten (wird asynchron geladen; danach neu zeichnen).
pub fn image_for(images: &mut HashMap<u64, HtmlImageElement>, key: u64, png: &[u8]) -> Option<HtmlImageElement> {
    if let Some(img) = images.get(&key) {
        return if img.complete() && img.natural_width() > 0 { Some(img.clone()) } else { None };
    }
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(png));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("image/png");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).ok()?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).ok()?;
    let img = HtmlImageElement::new().ok()?;
    let onload = Closure::once_into_js(move || {
        with_app(|a| {
            if let Some(ed) = &mut a.ed {
                ed.bmps.clear();
                ed.thumbs_dirty = true;
                ed.thumb_versions.clear();
            }
            a.redraw();
        });
    });
    img.set_onload(Some(onload.unchecked_ref()));
    img.set_src(&url);
    images.insert(key, img);
    None
}

pub struct DrawOpts<'a> {
    /// Diese Elemente nicht zeichnen (z. B. Text in Bearbeitung)
    pub skip: Option<u32>,
    /// Diese Elemente mit Transformation zeichnen (Auswahl wird verschoben)
    pub moving: Option<(&'a [u32], Xf)>,
    pub clip: Rect,
}

/// Zeichnet eine Seite in Seitenkoordinaten (der Aufrufer setzt die Transformation).
pub fn draw_page(ctx: &CanvasRenderingContext2d, page: &Page, w: f32, h: f32, c: &mut Caches, o: &DrawOpts) {
    ctx.save();
    ctx.begin_path();
    ctx.rect(0.0, 0.0, w as f64, h as f64);
    ctx.clip();
    ctx.set_fill_style_str(&page.paper.to_hex());
    ctx.fill_rect(0.0, 0.0, w as f64, h as f64);
    draw_template(ctx, page, w, h, c);
    for pass in 0..4 {
        for it in &page.items {
            if Some(it.id) == o.skip {
                continue;
            }
            let mv = o.moving.and_then(|(ids, xf)| if ids.contains(&it.id) { Some(xf) } else { None });
            if mv.is_none() && !it.bbox().intersects(&o.clip) {
                continue;
            }
            if let Some(xf) = mv {
                ctx.save();
                let _ = ctx.transform(xf.a as f64, xf.b as f64, xf.c as f64, xf.d as f64, xf.e as f64, xf.f as f64);
            }
            match (&it.content, pass) {
                (Content::Image(img), 0) => {
                    if let Some(el) = image_for(c.images, img.key, &img.png) {
                        let _ = ctx.draw_image_with_html_image_element_and_dw_and_dh(&el, img.x as f64, img.y as f64, img.w as f64, img.h as f64);
                    } else {
                        ctx.set_fill_style_str("rgba(127,127,127,.15)");
                        ctx.fill_rect(img.x as f64, img.y as f64, img.w as f64, img.h as f64);
                    }
                }
                (Content::Stroke(s), 1) if s.pen == PenKind::Highlighter => {
                    let p = c.path(it.id, s);
                    let _ = ctx.set_global_composite_operation("multiply");
                    ctx.set_fill_style_str(&rgba_css(s.color));
                    ctx.fill_with_path_2d(&p);
                    let _ = ctx.set_global_composite_operation("source-over");
                }
                (Content::Stroke(s), 2) if s.pen != PenKind::Highlighter => {
                    let p = c.path(it.id, s);
                    ctx.set_fill_style_str(&rgba_css(s.color));
                    ctx.fill_with_path_2d(&p);
                }
                (Content::Text(t), 3) => {
                    let lines = c.lines(ctx, it.id, t);
                    ctx.set_font(&text_font(t));
                    ctx.set_text_baseline("alphabetic");
                    ctx.set_fill_style_str(&rgba_css(t.color));
                    for (i, l) in lines.iter().enumerate() {
                        let _ = ctx.fill_text(l, t.x as f64, (t.y + t.size * (0.95 + 1.3 * i as f32)) as f64);
                    }
                }
                _ => {}
            }
            if mv.is_some() {
                ctx.restore();
            }
        }
    }
    draw_checks(ctx, page, w, h);
    ctx.restore();
}

/// Abgehakte Checklisten-Zeilen: Kästchen füllen, Häkchen, Inhalt ausgrauen und durchstreichen.
fn draw_checks(ctx: &CanvasRenderingContext2d, page: &Page, w: f32, h: f32) {
    if page.checks.is_empty() {
        return;
    }
    let fade = page.template.kind == TemplateKind::Checklist;
    let light = page.paper.luminance() > 0.5;
    let [pr, pg, pb] = page.paper.to_u8();
    for (i, row) in page.template.check_rows(w, h).iter().enumerate() {
        if !page.is_checked(i) {
            continue;
        }
        let s = row.size as f64;
        // Inhalt ausgrauen (nur auf der schlichten Checkliste – farbige Flächen bleiben sichtbar)
        if fade {
            ctx.set_fill_style_str(&format!("rgba({},{},{},0.55)", pr, pg, pb));
            ctx.fill_rect(row.text_x as f64 - 4.0, row.y0 as f64 + 1.0, (row.x1 - row.text_x) as f64 + 4.0, (row.y1 - row.y0) as f64 - 2.0);
        }
        // durchstreichen
        if let Some((x0, x1, y)) = page.row_strike(row) {
            ctx.set_stroke_style_str(if light { "rgba(35,37,50,.85)" } else { "rgba(235,236,245,.85)" });
            ctx.set_line_width(s * 0.09);
            ctx.set_line_cap("round");
            ctx.begin_path();
            ctx.move_to(x0 as f64, y as f64);
            ctx.line_to(x1 as f64, y as f64);
            ctx.stroke();
        }
        // gefülltes Kästchen mit Häkchen
        let (bx, by) = (row.bx as f64, row.by as f64);
        ctx.set_fill_style_str("#6d5cff");
        ctx.begin_path();
        let _ = ctx.round_rect_with_f64(bx - s * 0.04, by - s * 0.04, s * 1.08, s * 1.08, s * 0.18);
        ctx.fill();
        ctx.set_stroke_style_str("#ffffff");
        ctx.set_line_width(s * 0.13);
        ctx.set_line_cap("round");
        ctx.set_line_join("round");
        ctx.begin_path();
        ctx.move_to(bx + s * 0.22, by + s * 0.53);
        ctx.line_to(bx + s * 0.43, by + s * 0.73);
        ctx.line_to(bx + s * 0.79, by + s * 0.3);
        ctx.stroke();
    }
}

fn draw_template(ctx: &CanvasRenderingContext2d, page: &Page, w: f32, h: f32, c: &mut Caches) {
    let t = page.template;
    if t.kind == TemplateKind::Blank {
        return;
    }
    let light = page.paper.luminance() > 0.5;
    let key = format!("{}-{}-{}-{}", t.kind as u8, t.spacing, w, h);
    if !c.tpl.contains_key(&key) {
        let prims = t.primitives(w, h);
        let mut d = [String::new(), String::new(), String::new(), String::new()];
        for p in &prims {
            match *p {
                Prim::Line { x0, y0, x1, y1, accent, strong } => {
                    let k = if accent { 2 } else if strong { 1 } else { 0 };
                    let _ = write!(d[k], "M{:.2} {:.2}L{:.2} {:.2}", x0, y0, x1, y1);
                }
                Prim::Dot { x, y, r } => {
                    let _ = write!(d[3], "M{:.2} {:.2}A{r:.2} {r:.2} 0 1 0 {:.2} {:.2}A{r:.2} {r:.2} 0 1 0 {:.2} {:.2}", x + r, y, x - r, y, x + r, y, r = r);
                }
                Prim::Box { x, y, size } => {
                    let _ = write!(d[1], "M{:.2} {:.2}h{:.2}v{:.2}h{:.2}Z", x, y, size, size, -size);
                }
                Prim::Fill { .. } | Prim::Label { .. } => {}
            }
        }
        let v = d.iter().enumerate().filter(|(_, s)| !s.is_empty()).filter_map(|(k, s)| Path2d::new_with_path_string(s).ok().map(|p| (k as u8, p))).collect();
        let decor = prims.into_iter().filter(|p| matches!(p, Prim::Fill { .. } | Prim::Label { .. })).collect();
        c.tpl.insert(key.clone(), (v, decor));
    }
    let (line, accent) = line_colors(page.paper);
    let css = |c: [f32; 4]| format!("rgba({},{},{},{})", (c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, c[3]);
    let css8 = |c: [u8; 4]| format!("rgba({},{},{},{:.3})", c[0], c[1], c[2], c[3] as f32 / 255.0);
    let (paths, decor) = &c.tpl[&key];
    // Farbflächen unter den Linien
    for p in decor {
        if let Prim::Fill { x, y, w: fw, h: fh, r, color } = *p {
            ctx.set_fill_style_str(&css8(color));
            ctx.begin_path();
            if r > 0.0 {
                let _ = ctx.round_rect_with_f64(x as f64, y as f64, fw as f64, fh as f64, r as f64);
            } else {
                ctx.rect(x as f64, y as f64, fw as f64, fh as f64);
            }
            ctx.fill();
        }
    }
    let lw = t.line_width(w, h) as f64;
    for (k, p) in paths {
        match k {
            3 => {
                ctx.set_fill_style_str(&css([line[0], line[1], line[2], (line[3] * 1.4).min(1.0)]));
                ctx.fill_with_path_2d(p);
            }
            _ => {
                ctx.set_stroke_style_str(&css(if *k == 2 { accent } else { line }));
                ctx.set_line_width(if *k == 1 { lw * 1.8 } else { lw });
                ctx.stroke_with_path(p);
            }
        }
    }
    // Beschriftungen
    ctx.set_text_baseline("alphabetic");
    ctx.set_text_align("start");
    for p in decor {
        if let Prim::Label { x, y, size, text, color, bold } = *p {
            ctx.set_font(&format!("{} {}px {}", if bold { 700 } else { 500 }, size, FONT));
            ctx.set_fill_style_str(&match color {
                Some(c) => css8(c),
                None => css([line[0], line[1], line[2], if light { 0.9 } else { 0.7 }]),
            });
            let _ = ctx.fill_text(text, x as f64, y as f64);
        }
    }
}

impl Editor {
    /// Hauptansicht neu zeichnen.
    pub fn render(&mut self, images: &mut HashMap<u64, HtmlImageElement>, dark: bool) {
        self.dirty = false;
        let (dpr, s) = (self.dpr, self.view.s());
        let (sx, sy) = (self.view.sx, self.view.sy);
        let ctx = self.ctx.clone();
        ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).ok();
        ctx.set_fill_style_str(if dark { "#15161b" } else { "#e9ebf1" });
        ctx.fill_rect(0.0, 0.0, self.vw * dpr, self.vh * dpr);
        let (w, h) = (self.nb.format.w, self.nb.format.h);
        let span = self.ph() + GAP;
        let first = ((sy / span).floor().max(0.0)) as usize;
        let last = (((sy + self.vh / s) / span).floor().max(0.0) as usize).min(self.nb.pages.len() - 1);
        let moving_ids: Vec<u32>;
        let moving = match (&self.op, &self.selection) {
            (Some(Op::MoveSel { xf, .. }), Some(sel)) => {
                moving_ids = sel.ids.clone();
                Some((sel.page, *xf))
            }
            _ => {
                moving_ids = Vec::new();
                None
            }
        };
        let skip = self.text_edit.as_ref().and_then(|t| t.item);
        let skip_page = self.text_edit.as_ref().map(|t| t.page);
        let px_scale = s * dpr;
        let max_bmp = 4096.0;
        let use_bmp = (w as f64 * px_scale) <= max_bmp && (h as f64 * px_scale) <= max_bmp;
        for i in first..=last.max(first) {
            if i >= self.nb.pages.len() {
                break;
            }
            let top = self.page_top(i);
            let (ox, oy) = (-sx * s * dpr, (top - sy) * s * dpr);
            let pid = self.nb.pages[i].id;
            // Schatten
            ctx.set_fill_style_str(if dark { "rgba(0,0,0,.45)" } else { "rgba(30,35,60,.10)" });
            ctx.fill_rect(ox - 1.0, oy + 3.0 * dpr, w as f64 * px_scale + 2.0, h as f64 * px_scale + 2.0);
            let is_live = self.live_page == Some(pid) || moving.map(|m| m.0) == Some(pid) || skip_page == Some(pid);
            let version = self.nb.pages[i].version;
            if use_bmp && !is_live {
                let valid = self.bmps.get(&pid).map(|b| (b.scale - px_scale).abs() < 1e-6 && b.version == version && b.dark == dark).unwrap_or(false);
                if !valid && self.zooming && self.bmps.contains_key(&pid) {
                    // während des Zoomens altes Bild skaliert verwenden
                } else if !valid {
                    let entry = self.bmps.remove(&pid);
                    let (canvas, bctx) = match entry {
                        Some(b) => (b.canvas, b.ctx),
                        None => new_canvas(),
                    };
                    canvas.set_width((w as f64 * px_scale).ceil() as u32);
                    canvas.set_height((h as f64 * px_scale).ceil() as u32);
                    bctx.set_transform(px_scale, 0.0, 0.0, px_scale, 0.0, 0.0).ok();
                    {
                        let page = &self.nb.pages[i];
                        let mut c = Caches { paths: &mut self.paths, tpl: &mut self.tpl_cache, wraps: &mut self.wraps, images };
                        draw_page(&bctx, page, w, h, &mut c, &DrawOpts { skip: None, moving: None, clip: Rect::new(0.0, 0.0, w, h).expand(50.0) });
                    }
                    self.bmps.insert(pid, PageBmp { canvas, ctx: bctx, scale: px_scale, version, dark });
                }
                if let Some(b) = self.bmps.get(&pid) {
                    let _ = ctx.draw_image_with_html_canvas_element_and_dw_and_dh(&b.canvas, ox, oy, w as f64 * px_scale, h as f64 * px_scale);
                    continue;
                }
            }
            // direkt zeichnen (stark gezoomt oder Seite wird gerade bearbeitet)
            ctx.set_transform(px_scale, 0.0, 0.0, px_scale, ox, oy).ok();
            let clip = Rect::new((sx) as f32, (sy - top) as f32, (sx + self.vw / s) as f32, (sy - top + self.vh / s) as f32).expand(20.0);
            let mv = match moving {
                Some((p, xf)) if p == pid => Some((moving_ids.as_slice(), xf)),
                _ => None,
            };
            let page = &self.nb.pages[i];
            let mut c = Caches { paths: &mut self.paths, tpl: &mut self.tpl_cache, wraps: &mut self.wraps, images };
            draw_page(&ctx, page, w, h, &mut c, &DrawOpts { skip, moving: mv, clip });
            ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).ok();
        }
        // Auswahlrahmen
        if let Some(sel) = &self.selection {
            if let Some(i) = self.page_index(sel.page) {
                let mut b = sel.bbox;
                if let Some((_, xf)) = moving {
                    let (x0, y0) = xf.apply(b.x0, b.y0);
                    let (x1, y1) = xf.apply(b.x1, b.y1);
                    b = Rect::new(x0, y0, x1, y1);
                }
                let (x0, y0) = self.page_to_screen(i, b.x0, b.y0);
                let (x1, y1) = self.page_to_screen(i, b.x1, b.y1);
                ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0).ok();
                ctx.set_stroke_style_str("#6d5cff");
                ctx.set_line_width(1.5);
                let dash = js_sys::Array::of2(&6.0.into(), &5.0.into());
                let _ = ctx.set_line_dash(&dash);
                ctx.stroke_rect(x0 - 6.0, y0 - 6.0, x1 - x0 + 12.0, y1 - y0 + 12.0);
                let _ = ctx.set_line_dash(&js_sys::Array::new());
                ctx.set_fill_style_str("#ffffff");
                ctx.begin_path();
                let _ = ctx.arc(x1 + 6.0, y1 + 6.0, 9.0, 0.0, std::f64::consts::TAU);
                ctx.fill();
                ctx.set_line_width(2.5);
                ctx.stroke();
                ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).ok();
            }
        }
        // Lineal / Geodreieck
        if let Some(r) = self.ruler {
            r.draw(&ctx, dpr, 2.8346 * s, dark);
        }
        // ungenutzte Seitenbilder freigeben
        if self.bmps.len() > 8 {
            let keep: Vec<u32> = (first..=last).filter_map(|i| self.nb.pages.get(i).map(|p| p.id)).collect();
            self.bmps.retain(|k, _| keep.contains(k));
        }
        if self.paths.len() > 20_000 {
            self.paths.clear();
        }
    }

    /// Transformation für die Live-Ebene in Seitenkoordinaten der Seite `i`.
    pub fn live_xf(&self, i: usize) {
        let (dpr, s) = (self.dpr, self.view.s());
        let top = self.page_top(i);
        self.lctx.set_transform(s * dpr, 0.0, 0.0, s * dpr, -self.view.sx * s * dpr, (top - self.view.sy) * s * dpr).ok();
    }

    pub fn clear_live(&self) {
        self.lctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).ok();
        self.lctx.clear_rect(0.0, 0.0, self.vw * self.dpr, self.vh * self.dpr);
    }

    /// Aktuellen Strich / Lasso / Radierer auf der Live-Ebene zeichnen.
    pub fn draw_live(&self, cursor: Option<(f64, f64, f64)>) {
        self.clear_live();
        let ctx = &self.lctx;
        match &self.op {
            Some(Op::Ink { page, stroke, predicted, rule, .. }) => {
                let Some(i) = self.page_index(*page) else { return };
                // Länge der Linie am Lineal anzeigen
                if let (Some(_), Some(a), Some(b)) = (rule, stroke.pts.first(), stroke.pts.last()) {
                    let cm = (((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt() * 0.035278) as f64;
                    let (lx, ly) = self.page_to_screen(i, b.x, b.y);
                    ctx.set_transform(self.dpr, 0.0, 0.0, self.dpr, 0.0, 0.0).ok();
                    let label = format!("{:.1} cm", cm).replace('.', ",");
                    ctx.set_font("700 13px Inter, system-ui, sans-serif");
                    ctx.set_fill_style_str("rgba(20,22,32,.88)");
                    ctx.begin_path();
                    let _ = ctx.round_rect_with_f64(lx + 14.0, ly - 40.0, 70.0, 26.0, 13.0);
                    ctx.fill();
                    ctx.set_fill_style_str("#ffffff");
                    let _ = ctx.fill_text(&label, lx + 24.0, ly - 22.5);
                }
                self.live_xf(i);
                ctx.save();
                ctx.begin_path();
                ctx.rect(0.0, 0.0, self.pw(), self.ph());
                ctx.clip();
                let mut s = stroke.clone();
                s.pts.extend_from_slice(predicted);
                let p = outline_path(&s);
                if s.pen == PenKind::Highlighter {
                    let _ = ctx.set_global_composite_operation("multiply");
                }
                ctx.set_fill_style_str(&rgba_css(s.color));
                ctx.fill_with_path_2d(&p);
                let _ = ctx.set_global_composite_operation("source-over");
                ctx.restore();
            }
            Some(Op::Lasso { page, pts, .. }) => {
                let Some(i) = self.page_index(*page) else { return };
                self.live_xf(i);
                if pts.len() > 1 {
                    let s = self.view.s();
                    ctx.begin_path();
                    ctx.move_to(pts[0].0 as f64, pts[0].1 as f64);
                    for p in &pts[1..] {
                        ctx.line_to(p.0 as f64, p.1 as f64);
                    }
                    ctx.close_path();
                    ctx.set_fill_style_str("rgba(109,92,255,.08)");
                    ctx.fill();
                    ctx.set_stroke_style_str("#6d5cff");
                    ctx.set_line_width(1.6 / s);
                    let dash = js_sys::Array::of2(&(6.0 / s).into(), &(5.0 / s).into());
                    let _ = ctx.set_line_dash(&dash);
                    ctx.stroke();
                    let _ = ctx.set_line_dash(&js_sys::Array::new());
                }
            }
            _ => {}
        }
        if let Some((x, y, r)) = cursor {
            ctx.set_transform(self.dpr, 0.0, 0.0, self.dpr, 0.0, 0.0).ok();
            ctx.begin_path();
            let _ = ctx.arc(x, y, r.max(3.0), 0.0, std::f64::consts::TAU);
            ctx.set_fill_style_str("rgba(255,255,255,.35)");
            ctx.fill();
            ctx.set_stroke_style_str("rgba(40,40,60,.7)");
            ctx.set_line_width(1.2);
            ctx.stroke();
        }
    }

    pub fn draw_laser(&mut self, t: f64, color: [u8; 4]) {
        self.laser.retain(|p| t - p.2 < 650.0);
        self.clear_live();
        if self.laser.is_empty() {
            return;
        }
        let ctx = &self.lctx;
        ctx.set_transform(self.dpr, 0.0, 0.0, self.dpr, 0.0, 0.0).ok();
        ctx.set_line_cap("round");
        ctx.set_line_join("round");
        ctx.set_shadow_blur(14.0);
        ctx.set_shadow_color(&rgba_css(color));
        for w in self.laser.windows(2) {
            let age = (t - w[1].2) / 650.0;
            let a = (1.0 - age).clamp(0.0, 1.0);
            ctx.set_stroke_style_str(&rgba_css([color[0], color[1], color[2], (a * 255.0) as u8]));
            ctx.set_line_width(4.0 + 3.0 * a);
            ctx.begin_path();
            ctx.move_to(w[0].0, w[0].1);
            ctx.line_to(w[1].0, w[1].1);
            ctx.stroke();
        }
        if let Some(p) = self.laser.last() {
            ctx.set_fill_style_str("#ffffff");
            ctx.begin_path();
            let _ = ctx.arc(p.0, p.1, 4.0, 0.0, std::f64::consts::TAU);
            ctx.fill();
        }
        ctx.set_shadow_blur(0.0);
    }

    /// Vorschaubilder der Seitenübersicht aktualisieren.
    pub fn render_thumbs(&mut self, images: &mut HashMap<u64, HtmlImageElement>, _dark: bool) {
        self.thumbs_dirty = false;
        let (w, h) = (self.nb.format.w, self.nb.format.h);
        let tw = 132.0f64;
        let scale = tw / w as f64 * self.dpr;
        for page in &self.nb.pages {
            if self.thumb_versions.get(&page.id) == Some(&page.version) {
                continue;
            }
            let Ok(Some(c)) = dom().query_selector(&format!("#nav-list canvas[data-page=\"{}\"]", page.id)) else { continue };
            let canvas: HtmlCanvasElement = c.unchecked_into();
            canvas.set_width((w as f64 * scale).round() as u32);
            canvas.set_height((h as f64 * scale).round() as u32);
            let Some(ctx) = canvas.get_context("2d").ok().flatten() else { continue };
            let ctx: CanvasRenderingContext2d = ctx.unchecked_into();
            ctx.set_transform(scale, 0.0, 0.0, scale, 0.0, 0.0).ok();
            let mut c = Caches { paths: &mut self.paths, tpl: &mut self.tpl_cache, wraps: &mut self.wraps, images };
            draw_page(&ctx, page, w, h, &mut c, &DrawOpts { skip: None, moving: None, clip: Rect::new(0.0, 0.0, w, h) });
            self.thumb_versions.insert(page.id, page.version);
        }
    }

    /// Seite als PNG (Data-URL) in hoher Auflösung.
    pub fn page_png(&mut self, i: usize, images: &mut HashMap<u64, HtmlImageElement>) -> Option<String> {
        let (w, h) = (self.nb.format.w, self.nb.format.h);
        let scale = 3.0;
        let (canvas, ctx) = new_canvas();
        canvas.set_width((w as f64 * scale) as u32);
        canvas.set_height((h as f64 * scale) as u32);
        ctx.set_transform(scale, 0.0, 0.0, scale, 0.0, 0.0).ok();
        let page = self.nb.pages.get(i)?;
        let mut c = Caches { paths: &mut self.paths, tpl: &mut self.tpl_cache, wraps: &mut self.wraps, images };
        draw_page(&ctx, page, w, h, &mut c, &DrawOpts { skip: None, moving: None, clip: Rect::new(0.0, 0.0, w, h) });
        canvas.to_data_url_with_type("image/png").ok()
    }
}
