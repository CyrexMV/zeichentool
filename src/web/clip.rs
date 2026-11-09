//! Zwischenablage über Notizbücher hinweg und Ausschnitte (z. B. aus importierten PDFs) als Bild.

use super::render::{draw_page, Caches, DrawOpts};
use super::*;
use crate::engine::note::hash_bytes;
use crate::engine::{Content, ImageItem, Item, Rect};
use std::rc::Rc;

impl App {
    /// Zwischenablage des aktiven Notizbuchs für alle Notizbücher merken.
    pub fn remember_clipboard(&mut self) {
        if let Some(ed) = &self.ed {
            if !ed.clipboard.is_empty() {
                self.clip = ed.clipboard.clone();
            }
        }
    }

    /// Gemerkte Zwischenablage ins (neu) aktive Notizbuch übernehmen.
    pub fn restore_clipboard(&mut self) {
        let clip = self.clip.clone();
        if let Some(ed) = &mut self.ed {
            if !clip.is_empty() {
                ed.clipboard = clip;
            }
        }
    }

    /// Eingekreisten Bereich (inkl. PDF-Hintergrund und Handschrift) als Bild in die Zwischenablage legen.
    /// Das Bild folgt der Lasso-Form; außerhalb ist es durchsichtig.
    pub fn copy_region_image(&mut self) -> bool {
        let images = &mut self.images;
        let Some(ed) = &mut self.ed else { return false };
        let Some(sel) = &ed.selection else { return false };
        let Some(i) = ed.page_index(sel.page) else { return false };
        let (pw, ph) = ed.nb.page_size(i);
        let mut b = sel.bbox;
        if !sel.poly.is_empty() {
            b = Rect::EMPTY;
            for &(x, y) in &sel.poly {
                b.add(x, y);
            }
        }
        let b = Rect::new(b.x0.max(0.0), b.y0.max(0.0), b.x1.min(pw), b.y1.min(ph));
        if b.w() < 2.0 || b.h() < 2.0 {
            return false;
        }
        let page = &ed.nb.pages[i];
        // Auflösung an den Hintergrund anpassen (PDF-Seiten haben ~2 Bildpunkte je pt)
        let scale = page
            .bg
            .as_ref()
            .and_then(|bg| images.get(&bg.key))
            .filter(|el| el.natural_width() > 0)
            .map(|el| el.natural_width() as f64 / pw as f64)
            .unwrap_or(2.5)
            .clamp(1.5, 3.0);
        let (cw, ch) = ((b.w() as f64 * scale).ceil().min(4096.0), (b.h() as f64 * scale).ceil().min(4096.0));
        let (canvas, ctx) = super::editor::new_canvas();
        canvas.set_width(cw as u32);
        canvas.set_height(ch as u32);
        let _ = ctx.set_transform(scale, 0.0, 0.0, scale, -b.x0 as f64 * scale, -b.y0 as f64 * scale);
        if sel.poly.len() > 2 {
            ctx.begin_path();
            ctx.move_to(sel.poly[0].0 as f64, sel.poly[0].1 as f64);
            for &(x, y) in &sel.poly[1..] {
                ctx.line_to(x as f64, y as f64);
            }
            ctx.close_path();
            ctx.clip();
        }
        {
            let mut c = Caches { paths: &mut ed.paths, tpl: &mut ed.tpl_cache, wraps: &mut ed.wraps, images };
            draw_page(&ctx, page, pw, ph, &mut c, &DrawOpts { skip: None, moving: None, clip: b.expand(2.0) });
        }
        let Ok(url) = canvas.to_data_url_with_type("image/png") else { return false };
        let Some(b64) = url.split(',').nth(1) else { return false };
        let Ok(bin) = window().atob(b64) else { return false };
        let png: Vec<u8> = bin.chars().map(|c| c as u32 as u8).collect();
        let key = hash_bytes(&png);
        let item = Item { id: 0, order: 0.0, content: Content::Image(ImageItem { x: b.x0, y: b.y0, w: b.w(), h: b.h(), png: Rc::new(png), key }), pinned: false };
        ed.clipboard = vec![item];
        self.clip = ed.clipboard.clone();
        true
    }
}
