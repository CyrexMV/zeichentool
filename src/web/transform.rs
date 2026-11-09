//! Auswahl drehen/spiegeln, Elemente fixieren und Seiten drehen (Hoch-/Querformat).

use super::*;
use crate::engine::note::hash_bytes;
use crate::engine::{Action, Content, ImageItem, Item, PageMeta, Rect, Xf};
use std::rc::Rc;
use web_sys::HtmlImageElement;

/// Bildpunkte eines Bildes mit dem linearen Anteil von `xf` (Drehung/Spiegelung) umrechnen.
/// Liefert neue Bilddaten (JPEG bleibt JPEG) samt Schlüssel; `None`, falls das Bild noch nicht geladen ist.
fn transform_pixels(images: &mut HashMap<u64, HtmlImageElement>, img: &ImageItem, xf: &Xf) -> Option<(Vec<u8>, u64)> {
    let el = super::render::image_for(images, img.key, &img.png)?;
    let (nw, nh) = (el.natural_width() as f64, el.natural_height() as f64);
    let (a, b, c, d) = (xf.a as f64, xf.b as f64, xf.c as f64, xf.d as f64);
    let pts = [(0.0, 0.0), (nw, 0.0), (0.0, nh), (nw, nh)].map(|(x, y)| (a * x + c * y, b * x + d * y));
    let min_x = pts.iter().map(|p| p.0).fold(f64::MAX, f64::min);
    let max_x = pts.iter().map(|p| p.0).fold(f64::MIN, f64::max);
    let min_y = pts.iter().map(|p| p.1).fold(f64::MAX, f64::min);
    let max_y = pts.iter().map(|p| p.1).fold(f64::MIN, f64::max);
    let (canvas, ctx) = super::editor::new_canvas();
    canvas.set_width((max_x - min_x).round().max(1.0) as u32);
    canvas.set_height((max_y - min_y).round().max(1.0) as u32);
    ctx.set_transform(a, b, c, d, -min_x, -min_y).ok()?;
    ctx.draw_image_with_html_image_element(&el, 0.0, 0.0).ok()?;
    let jpeg = img.png.first() == Some(&0xFF);
    let url = canvas.to_data_url_with_type_and_encoder_options(if jpeg { "image/jpeg" } else { "image/png" }, &JsValue::from_f64(0.92)).ok()?;
    let bin = window().atob(url.split(',').nth(1)?).ok()?;
    let bytes: Vec<u8> = bin.chars().map(|ch| ch as u32 as u8).collect();
    let key = hash_bytes(&bytes);
    Some((bytes, key))
}

/// Element transformieren; Bilder werden dabei auch pixelweise gedreht/gespiegelt.
fn transform_item(images: &mut HashMap<u64, HtmlImageElement>, it: &Item, xf: &Xf) -> Item {
    let mut n = it.transformed(xf);
    if let (Content::Image(src), Content::Image(dst)) = (&it.content, &mut n.content) {
        if let Some((png, key)) = transform_pixels(images, src, xf) {
            dst.png = Rc::new(png);
            dst.key = key;
        }
    }
    n
}

impl App {
    /// Auswahl drehen (`rot-cw`, `rot-ccw`) oder spiegeln (`flip-h`, `flip-v`).
    pub fn transform_selection_kind(&mut self, kind: &str) {
        let images = &mut self.images;
        let Some(ed) = &mut self.ed else { return };
        let Some(sel) = &ed.selection else { return };
        if ed.sel_pinned() {
            self.toast("Fixierte Elemente zuerst lösen");
            return;
        }
        let (cx, cy) = sel.bbox.center();
        let xf = match kind {
            "rot-cw" => Xf::rot90_about(cx, cy),
            "rot-ccw" => Xf { a: 0.0, b: -1.0, c: 1.0, d: 0.0, e: cx - cy, f: cy + cx },
            "flip-h" => Xf::flip_about(true, cx, cy),
            "flip-v" => Xf::flip_about(false, cx, cy),
            _ => return,
        };
        let items: Vec<Item> = ed.selected_items().iter().map(|it| transform_item(images, it, &xf)).collect();
        ed.replace_selected(items);
        ed.dirty = true;
        self.after_edit();
    }

    /// Auswahl fixieren bzw. lösen.
    pub fn toggle_pin(&mut self) {
        let Some(ed) = &mut self.ed else { return };
        match ed.toggle_pin_selection() {
            Some(true) => {
                ed.selection = None;
                show("sel-menu", false);
                self.toast("Fixiert – lässt sich nicht mehr verschieben oder radieren. Zum Lösen mit dem Lasso einkreisen.");
            }
            Some(false) => self.toast("Gelöst – lässt sich wieder verschieben"),
            None => {}
        }
        self.refresh_pin_button();
        self.after_edit();
    }

    /// Fixieren-Knopf im Auswahlmenü an den Zustand anpassen.
    pub fn refresh_pin_button(&self) {
        let pinned = self.ed.as_ref().is_some_and(|e| e.selection.is_some() && e.sel_pinned());
        let b = el("sel-pin");
        set_class(&b, "active", pinned);
        let _ = b.set_attribute("title", if pinned { "Lösen" } else { "Fixieren" });
    }

    /// Aktuelle Seite um 90° im Uhrzeigersinn drehen (Hoch- ↔ Querformat), Inhalt dreht mit.
    pub fn rotate_page(&mut self) {
        let Some(i) = self.ed.as_ref().map(|e| e.cur_page) else { return };
        let Some(action) = self.rotate_page_at(i) else { return };
        let Some(ed) = &mut self.ed else { return };
        let land = ed.nb.pages[i].land;
        ed.push(action);
        ed.update_fit();
        ed.bmps.clear();
        ed.goto_page(i);
        self.toast(if land { "Seite ins Querformat gedreht" } else { "Seite ins Hochformat gedreht" });
        self.after_pages_change();
    }

    /// Seite `i` drehen (ohne Verlaufseintrag) und die Aktion dafür liefern.
    pub fn rotate_page_at(&mut self, i: usize) -> Option<Action> {
        let images = &mut self.images;
        let ed = self.ed.as_mut()?;
        let page = ed.nb.pages.get(i)?;
        let (_, h) = ed.nb.page_size(i);
        // (x, y) → (h − y, x)
        let xf = Xf { a: 0.0, b: 1.0, c: -1.0, d: 0.0, e: h, f: 0.0 };
        let pid = page.id;
        let before = PageMeta::of(page);
        let mut after = before.clone();
        after.land = !page.land;
        if let Some(bg) = &page.bg {
            let mut nbg = bg.clone();
            (nbg.w, nbg.h) = (bg.h, bg.w);
            if let Some((png, key)) = transform_pixels(images, bg, &xf) {
                nbg.png = Rc::new(png);
                nbg.key = key;
            }
            after.bg = Some(nbg);
        }
        for l in after.links.iter_mut() {
            let (x0, y0) = xf.apply(l.rect.x0, l.rect.y0);
            let (x1, y1) = xf.apply(l.rect.x1, l.rect.y1);
            l.rect = Rect::new(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
        }
        let removed = page.items.clone();
        let added: Vec<Item> = removed.iter().map(|it| transform_item(images, it, &xf)).collect();
        ed.selection = None;
        let p = &mut ed.nb.pages[i];
        after.apply(p);
        p.items.clear();
        p.insert(added.clone());
        Some(Action::Group(vec![Action::PageMeta { page: pid, before, after }, Action::Items { page: pid, removed, added }]))
    }
}
