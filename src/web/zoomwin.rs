//! Zoom-Fenster (wie in GoodNotes): unten ein vergrößerter Schreibstreifen; ein Rahmen auf der Seite
//! zeigt die Stelle. Am rechten Rand rückt der Rahmen weiter, am Zeilenende in die nächste Zeile.

use super::editor::{Editor, Op};
use super::render::{draw_page, outline_path, Caches, DrawOpts};
use super::*;
use crate::engine::{Content, PenKind, Rect};
use std::cell::Cell;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

#[derive(Clone, Copy, Debug)]
pub struct ZoomWin {
    /// Seiten-ID und linke obere Ecke des Rahmens (Seitenkoordinaten)
    pub page: u32,
    pub x: f32,
    pub y: f32,
    /// Vergrößerung gegenüber der ganzen Seitenbreite
    pub zoom: f32,
    /// Zeilenanfang (für den Zeilenumbruch)
    pub left: f32,
}

thread_local! {
    /// Zeiger, der gerade im Zoom-Fenster schreibt: (Zeiger-ID, k, ox, oy) – Bildschirm = Fenster · k + o
    static MAP: Cell<Option<(i32, f64, f64, f64)>> = const { Cell::new(None) };
}

/// Fensterkoordinaten eines Zeigers in Koordinaten der Hauptansicht umrechnen.
pub fn map_xy(id: i32, x: f64, y: f64) -> (f64, f64) {
    MAP.with(|m| match m.get() {
        Some((pid, k, ox, oy)) if pid == id => (x * k + ox, y * k + oy),
        _ => (x, y),
    })
}

pub fn clear_map(id: i32) {
    MAP.with(|m| {
        if m.get().map(|v| v.0) == Some(id) {
            m.set(None);
        }
    });
}

pub fn mapped(id: i32) -> bool {
    MAP.with(|m| m.get().map(|v| v.0) == Some(id))
}

fn panel() -> HtmlCanvasElement {
    el("zw-canvas").unchecked_into()
}

impl Editor {
    /// Rahmengröße (Seitenkoordinaten) passend zum Seitenverhältnis des Fensters.
    pub fn zw_size(&self, z: &ZoomWin) -> (f32, f32) {
        let c = panel();
        let (cw, ch) = (c.client_width().max(1) as f32, c.client_height().max(1) as f32);
        let w = self.page_size_id(z.page).0 / z.zoom;
        (w, w * ch / cw)
    }

    /// Fenster-Pixel je Seiteneinheit.
    pub fn zw_scale(&self, z: &ZoomWin) -> f64 {
        let (w, _) = self.zw_size(z);
        panel().client_width().max(1) as f64 / w as f64
    }

    pub fn zw_rect(&self) -> Option<(usize, Rect)> {
        let z = self.zoomwin?;
        let i = self.page_index(z.page)?;
        let (w, h) = self.zw_size(&z);
        Some((i, Rect::new(z.x, z.y, z.x + w, z.y + h)))
    }

    /// Rahmen auf der Hauptansicht zeichnen.
    pub fn draw_zw_frame(&self, ctx: &CanvasRenderingContext2d) {
        let Some((i, r)) = self.zw_rect() else { return };
        let (x0, y0) = self.page_to_screen(i, r.x0, r.y0);
        let (x1, y1) = self.page_to_screen(i, r.x1, r.y1);
        let dpr = self.dpr;
        ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0).ok();
        ctx.set_fill_style_str("rgba(109,92,255,0.07)");
        ctx.fill_rect(x0, y0, x1 - x0, y1 - y0);
        ctx.set_stroke_style_str("rgba(109,92,255,0.9)");
        ctx.set_line_width(2.0);
        ctx.begin_path();
        let _ = ctx.round_rect_with_f64(x0, y0, x1 - x0, y1 - y0, 4.0);
        ctx.stroke();
        // Weiterrück-Zone
        let zx = x0 + (x1 - x0) * 0.75;
        ctx.set_fill_style_str("rgba(109,92,255,0.10)");
        ctx.fill_rect(zx, y0, x1 - zx, y1 - y0);
    }

    /// Inhalt des Zoom-Fensters zeichnen (Seite + laufender Strich).
    pub fn render_zw(&mut self, images: &mut HashMap<u64, HtmlImageElement>, dark: bool) {
        let Some(z) = self.zoomwin else { return };
        let Some((i, r)) = self.zw_rect() else { return };
        let c = panel();
        let dpr = self.dpr;
        let (cw, ch) = (c.client_width() as f64, c.client_height() as f64);
        if c.width() != (cw * dpr) as u32 || c.height() != (ch * dpr) as u32 {
            c.set_width((cw * dpr) as u32);
            c.set_height((ch * dpr) as u32);
        }
        let Some(ctx) = c.get_context("2d").ok().flatten() else { return };
        let ctx: CanvasRenderingContext2d = ctx.unchecked_into();
        ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).ok();
        ctx.set_fill_style_str(if dark { "#15161b" } else { "#e9ebf1" });
        ctx.fill_rect(0.0, 0.0, cw * dpr, ch * dpr);
        let k = self.zw_scale(&z) * dpr;
        ctx.set_transform(k, 0.0, 0.0, k, -(r.x0 as f64) * k, -(r.y0 as f64) * k).ok();
        let (w, h) = self.nb.page_size(i);
        {
            let page = &self.nb.pages[i];
            let mut cc = Caches { paths: &mut self.paths, tpl: &mut self.tpl_cache, wraps: &mut self.wraps, images };
            draw_page(&ctx, page, w, h, &mut cc, &DrawOpts { skip: None, moving: None, clip: r.expand(2.0) });
        }
        if let Some(Op::Ink { page, stroke, predicted, .. }) = &self.op {
            if *page == z.page {
                let mut s = stroke.clone();
                s.pts.extend_from_slice(predicted);
                let p = outline_path(&s);
                if s.pen == PenKind::Highlighter {
                    let _ = ctx.set_global_composite_operation("multiply");
                }
                ctx.set_fill_style_str(&rgba_css(s.color));
                ctx.fill_with_path_2d(&p);
                let _ = ctx.set_global_composite_operation("source-over");
            }
        }
        // Weiterrück-Zone rechts
        ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0).ok();
        ctx.set_fill_style_str("rgba(109,92,255,0.08)");
        ctx.fill_rect(cw * 0.75, 0.0, cw * 0.25, ch);
        ctx.set_stroke_style_str("rgba(109,92,255,0.35)");
        ctx.set_line_width(1.0);
        ctx.begin_path();
        ctx.move_to(cw * 0.75, 0.0);
        ctx.line_to(cw * 0.75, ch);
        ctx.stroke();
    }

    /// Ansicht so verschieben, dass der Rahmen im oberen Teil sichtbar ist.
    pub fn zw_reveal(&mut self) {
        let Some((i, r)) = self.zw_rect() else { return };
        let (_, y0) = self.page_to_screen(i, r.x0, r.y0);
        let (_, y1) = self.page_to_screen(i, r.x1, r.y1);
        let visible_bottom = self.vh * 0.6;
        if y0 < self.top_pad + 10.0 || y1 > visible_bottom {
            let s = self.view.s();
            self.view.sy = self.page_top(i) + r.y0 as f64 - (self.top_pad + 40.0) / s;
            self.clamp();
        }
        self.dirty = true;
    }

    /// Zeilenhöhe für den Umbruch: Linienabstand der Vorlage bzw. halbe Rahmenhöhe.
    fn zw_line(&self, z: &ZoomWin) -> f32 {
        let i = self.page_index(z.page).unwrap_or(0);
        let t = self.nb.pages[i].template;
        let (w, h) = self.nb.page_size(i);
        let (_, rh) = self.zw_size(z);
        match t.kind {
            crate::engine::TemplateKind::Lined | crate::engine::TemplateKind::Checklist | crate::engine::TemplateKind::Cornell => t.unit(w, h),
            _ => rh * 0.6,
        }
    }

    /// Rahmen weiterschieben: `dir` = 1 vor, -1 zurück, 0 neue Zeile.
    pub fn zw_step(&mut self, dir: i32) {
        let Some(mut z) = self.zoomwin else { return };
        let (rw, rh) = self.zw_size(&z);
        let (pw, ph) = self.page_size_id(z.page);
        let line = self.zw_line(&z);
        match dir {
            1 => {
                if z.x + rw >= pw - 1.0 {
                    z.x = z.left;
                    z.y += line;
                } else {
                    z.x = (z.x + rw * 0.55).min(pw - rw);
                }
            }
            -1 => z.x = (z.x - rw * 0.55).max(0.0),
            _ => {
                z.x = z.left;
                z.y += line;
            }
        }
        // ans Seitenende: nächste Seite
        if z.y + rh > ph {
            if let Some(i) = self.page_index(z.page) {
                if i + 1 < self.nb.pages.len() {
                    z.page = self.nb.pages[i + 1].id;
                    z.y = line * 2.0;
                } else {
                    z.y = ph - rh;
                }
            }
        }
        z.y = z.y.clamp(0.0, (ph - rh).max(0.0));
        self.zoomwin = Some(z);
        self.zw_reveal();
    }
}

impl App {
    pub fn toggle_zoomwin(&mut self) {
        let Some(ed) = &mut self.ed else { return };
        if ed.zoomwin.is_some() {
            ed.zoomwin = None;
            ed.dirty = true;
            show("zoomwin", false);
            set_class(&el("btn-zoomwin"), "active", false);
            set_class(&el("editor"), "zw-open", false);
            self.request_frame();
            return;
        }
        show("zoomwin", true);
        set_class(&el("btn-zoomwin"), "active", true);
        set_class(&el("editor"), "zw-open", true);
        let i = ed.cur_page;
        let page = ed.nb.pages[i].id;
        let (pw, ph) = ed.nb.page_size(i);
        let t = ed.nb.pages[i].template;
        // Start: linker Rand (bei Linien: Randlinie), im oberen sichtbaren Bereich
        let left = if t.kind == crate::engine::TemplateKind::Lined { t.unit(pw, ph) * 2.4 } else { pw * 0.06 };
        let (_, top) = ed.to_nb(0.0, ed.top_pad + ed.vh * 0.12);
        let y = (top - ed.page_top(i)) as f32;
        let mut z = ZoomWin { page, x: left, y: y.clamp(0.0, ph * 0.8), zoom: 3.0, left };
        let (_, rh) = ed.zw_size(&z);
        z.y = z.y.clamp(0.0, (ph - rh).max(0.0));
        ed.zoomwin = Some(z);
        ed.zw_reveal();
        self.toast("Zoom-Fenster: unten schreiben – der Rahmen rückt am rechten Rand automatisch weiter");
        self.request_frame();
    }

    pub fn zw_action(&mut self, act: &str) {
        let Some(ed) = &mut self.ed else { return };
        match act {
            "zw-next" => ed.zw_step(1),
            "zw-prev" => ed.zw_step(-1),
            "zw-line" => ed.zw_step(0),
            "zw-in" | "zw-out" => {
                if let Some(z) = &mut ed.zoomwin {
                    z.zoom = if act == "zw-in" { (z.zoom * 1.25).min(6.0) } else { (z.zoom / 1.25).max(1.5) };
                }
                if let Some(z) = ed.zoomwin {
                    let (rw, rh) = ed.zw_size(&z);
                    let (pw, ph) = ed.page_size_id(z.page);
                    let z2 = ZoomWin { x: z.x.min(pw - rw).max(0.0), y: z.y.min(ph - rh).max(0.0), ..z };
                    ed.zoomwin = Some(z2);
                }
                ed.dirty = true;
            }
            "zw-close" => {
                self.toggle_zoomwin();
                return;
            }
            _ => {}
        }
        self.request_frame();
    }

    /// Zeiger setzt im Zoom-Fenster auf: Umrechnung in Hauptansicht-Koordinaten einrichten.
    pub fn zw_pointer_down(&mut self, id: i32) -> bool {
        let Some(ed) = &self.ed else { return false };
        let Some(z) = ed.zoomwin else { return false };
        let Some((i, r)) = ed.zw_rect() else { return false };
        let rc = panel().get_bounding_client_rect();
        let kp = ed.zw_scale(&z);
        let s = ed.view.s();
        let k = s / kp;
        // Bildschirm = ((cx - left)/kp + r.x0 - sx) * s  → cx·k + ox
        let (sx0, sy0) = ed.page_to_screen(i, r.x0, r.y0);
        let ox = sx0 - rc.left() * k;
        let oy = sy0 - rc.top() * k;
        MAP.with(|m| m.set(Some((id, k, ox, oy))));
        true
    }

    /// Nach dem Absetzen im Zoom-Fenster: weiterrücken, wenn in die rechte Zone geschrieben wurde.
    pub fn zw_after_stroke(&mut self) {
        let Some(ed) = &mut self.ed else { return };
        let Some((i, r)) = ed.zw_rect() else { return };
        let last = ed.nb.pages[i].items.iter().max_by(|a, b| a.order.partial_cmp(&b.order).unwrap_or(std::cmp::Ordering::Equal));
        let Some(it) = last else { return };
        if !matches!(it.content, Content::Stroke(_)) {
            return;
        }
        let b = it.bbox();
        if b.x1 > r.x0 + r.w() * 0.75 && b.intersects(&r) {
            ed.zw_step(1);
            self.request_frame();
        }
    }

    /// Rahmen auf der Hauptansicht mit dem Finger anfassen?
    pub fn zw_frame_hit(&self, x: f64, y: f64) -> bool {
        let Some(ed) = &self.ed else { return false };
        let Some((i, r)) = ed.zw_rect() else { return false };
        let (x0, y0) = ed.page_to_screen(i, r.x0, r.y0);
        let (x1, y1) = ed.page_to_screen(i, r.x1, r.y1);
        x >= x0 - 10.0 && x <= x1 + 10.0 && y >= y0 - 10.0 && y <= y1 + 10.0
    }

    /// Rahmen ziehen: neue Position aus Bildschirmverschiebung.
    pub fn zw_drag(&mut self, start: (f32, f32, u32), dx: f64, dy: f64) {
        let Some(ed) = &mut self.ed else { return };
        let Some(mut z) = ed.zoomwin else { return };
        let s = ed.view.s() as f32;
        let (rw, rh) = ed.zw_size(&z);
        let (pw, ph) = ed.page_size_id(start.2);
        z.page = start.2;
        z.x = (start.0 + dx as f32 / s).clamp(0.0, (pw - rw).max(0.0));
        z.y = (start.1 + dy as f32 / s).clamp(0.0, (ph - rh).max(0.0));
        z.left = z.x;
        ed.zoomwin = Some(z);
        ed.dirty = true;
        self.request_frame();
    }
}
