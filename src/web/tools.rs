//! Werkzeuge: Stifte, Textmarker, Formen, Radierer, Lasso/Auswahl, Text, Bilder, Laserpointer.

use super::editor::{Op, Selection, TextEdit};
use super::*;
use crate::engine::geom::{dist, point_in_polygon, Rect, Xf};
use crate::engine::history::Action;
use crate::engine::note::hash_bytes;
use crate::engine::shapes::{recognize, Shape};
use crate::engine::{Content, ImageItem, Item, Pt, Smoother, Stroke, TextBox};
use std::rc::Rc;
use web_sys::HtmlTextAreaElement;

impl App {
    pub fn effective_tool(&self) -> Tool {
        if self.hold_eraser {
            Tool::Eraser
        } else {
            self.tool
        }
    }

    fn ink_params(&self, tool: Tool) -> (PenKind, [u8; 4], f32) {
        let p = &self.prefs;
        match tool {
            Tool::Highlighter => (PenKind::Highlighter, p.hl_colors[p.hl_ci], p.hl_sizes[p.hl_si]),
            _ => {
                let mut c = p.pen_colors[p.pen_ci];
                if p.pen_kind == PenKind::Pencil {
                    c[3] = 215;
                }
                (p.pen_kind, c, p.pen_sizes[p.pen_si])
            }
        }
    }

    pub fn eraser_radius(&self) -> f32 {
        self.prefs.er_sizes[self.prefs.er_si] * 0.5
    }

    /// Beginn einer Eingabe (Stift aufgesetzt / Finger zeichnet / Maus gedrückt).
    pub fn begin_op(&mut self, pointer: i32, sx: f64, sy: f64, pressure: f32) {
        self.close_popovers();
        if self.ed.as_ref().map(|e| e.text_edit.is_some()).unwrap_or(false) {
            self.commit_text();
        }
        self.hide_paste();
        let tool = self.effective_tool();
        let pressure = if self.settings.pressure { pressure } else { 0.55 };
        let smoothing = self.settings.smoothing;
        let ink = self.ink_params(tool);
        let er = (self.prefs.er_mode == EraserMode::Precise, self.prefs.er_hl_only, self.eraser_radius());
        let straight = tool == Tool::Highlighter && self.prefs.hl_straight;
        let Some(ed) = &mut self.ed else { return };
        // Checkliste: Kästchen antippen hakt ab (mit allen Werkzeugen außer Radierer/Laser)
        if !matches!(tool, Tool::Eraser | Tool::Laser) && ed.toggle_check_at(sx, sy) {
            vibrate(10);
            self.after_edit();
            return;
        }
        if tool != Tool::Lasso {
            ed.selection = None;
            show("sel-menu", false);
        }
        match tool {
            Tool::Laser => {
                ed.op = Some(Op::Laser { pointer });
                ed.laser.push((sx, sy, now()));
            }
            Tool::Text => {
                if let Some((i, x, y)) = ed.hit_page(sx, sy, 0.0) {
                    let hit = ed.nb.pages[i].items.iter().rev().find(|it| match &it.content {
                        Content::Text(t) => {
                            let lines = ed.wraps.get(&it.id).map(|w| w.1.len()).unwrap_or(1);
                            t.approx_rect(lines).expand(4.0).contains(x, y)
                        }
                        _ => false,
                    });
                    let id = hit.map(|it| it.id);
                    self.start_text(i, x, y, id);
                }
            }
            Tool::Lasso => {
                // Auswahl verschieben/skalieren?
                if let Some(sel) = &ed.selection {
                    if let Some(i) = ed.page_index(sel.page) {
                        let b = sel.bbox;
                        let (x0, y0) = ed.page_to_screen(i, b.x0, b.y0);
                        let (x1, y1) = ed.page_to_screen(i, b.x1, b.y1);
                        let (_, px, py) = ed.hit_page(sx, sy, 1e6).unwrap_or((i, 0.0, 0.0));
                        let (nx, ny) = ed.to_nb(sx, sy);
                        let local = (nx as f32, (ny - ed.page_top(i)) as f32);
                        let _ = (px, py);
                        if dist((x1 + 6.0) as f32, (y1 + 6.0) as f32, sx as f32, sy as f32) < 26.0 {
                            ed.op = Some(Op::MoveSel { pointer, start: local, scale: true, xf: Xf::IDENTITY });
                            return;
                        }
                        if sx >= x0 - 14.0 && sx <= x1 + 14.0 && sy >= y0 - 14.0 && sy <= y1 + 14.0 {
                            ed.op = Some(Op::MoveSel { pointer, start: local, scale: false, xf: Xf::IDENTITY });
                            show("sel-menu", false);
                            return;
                        }
                    }
                }
                ed.selection = None;
                show("sel-menu", false);
                if let Some((i, x, y)) = ed.hit_page(sx, sy, 40.0) {
                    let page = ed.nb.pages[i].id;
                    ed.op = Some(Op::Lasso { page, pointer, pts: vec![(x, y)] });
                }
                ed.dirty = true;
            }
            Tool::Eraser => {
                if let Some((i, x, y)) = ed.hit_page(sx, sy, 40.0) {
                    let page = ed.nb.pages[i].id;
                    let before = ed.nb.pages[i].items.clone();
                    ed.live_page = Some(page);
                    ed.op = Some(Op::Erase { page, pointer, last: (x, y), before, precise: er.0, hl_only: er.1, radius: er.2 });
                    ed.erase_segment((x, y), (x, y));
                }
            }
            Tool::Pen | Tool::Highlighter | Tool::Shape => {
                // Am Lineal/Geodreieck entlang: Startpunkt auf die Kante legen
                let (pen, color, width) = ink;
                let half = width as f64 * 0.5 * ed.view.s() + 1.0;
                let rule = ed.ruler.and_then(|r| r.snap(sx, sy, 34.0, half));
                let (sx, sy) = match &rule {
                    Some(seg) => super::ruler::project(seg, sx, sy),
                    None => (sx, sy),
                };
                if let Some((i, x, y)) = ed.hit_page(sx, sy, 30.0) {
                    let page = ed.nb.pages[i].id;
                    let p = Pt::new(x, y, pressure);
                    let mut smoother = Smoother::new(if straight || rule.is_some() { 0.0 } else { smoothing });
                    let first = smoother.push(p);
                    ed.op = Some(Op::Ink {
                        page,
                        pointer,
                        stroke: Stroke { pen, color, width, pts: vec![first] },
                        smoother,
                        raw: vec![p],
                        hold_t: now(),
                        hold_pos: (x, y),
                        quick: None,
                        shape_tool: tool == Tool::Shape,
                        straight,
                        predicted: Vec::new(),
                        rule,
                    });
                    ed.draw_live(None);
                }
            }
        }
        self.request_frame();
    }

    /// Weitere Eingabepunkte (Bildschirmkoordinaten + Druck).
    pub fn move_op(&mut self, pointer: i32, pts: &[(f64, f64, f32)], predicted: &[(f64, f64, f32)]) {
        let pressure_on = self.settings.pressure;
        let er_cursor = self.eraser_radius();
        let Some(ed) = &mut self.ed else { return };
        let s = ed.view.s();
        let Some(op) = &ed.op else { return };
        // Strich am Lineal: alle Punkte auf die Kante projizieren
        type Samples = Vec<(f64, f64, f32)>;
        let (pts, predicted): (Samples, Samples) = match op {
            Op::Ink { rule: Some(seg), .. } => {
                let pr = |v: &[(f64, f64, f32)]| v.iter().map(|&(x, y, p)| {
                    let (a, b) = super::ruler::project(seg, x, y);
                    (a, b, p)
                }).collect();
                (pr(pts), pr(predicted))
            }
            _ => (pts.to_vec(), predicted.to_vec()),
        };
        let (pts, predicted) = (&pts[..], &predicted[..]);
        let page_idx = match op {
            Op::Ink { page, .. } | Op::Erase { page, .. } | Op::Lasso { page, .. } => ed.page_index(*page),
            Op::MoveSel { .. } => ed.selection.as_ref().and_then(|sel| ed.page_index(sel.page)),
            Op::Laser { .. } => None,
        };
        let top = page_idx.map(|i| ed.page_top(i)).unwrap_or(0.0);
        let to_page = |sx: f64, sy: f64, ed: &Editor| {
            let (x, y) = ed.to_nb(sx, sy);
            (x as f32, (y - top) as f32)
        };
        let local: Vec<(f32, f32, f32)> = pts.iter().map(|&(sx, sy, p)| {
            let (x, y) = to_page(sx, sy, ed);
            (x, y, if pressure_on { p } else { 0.55 })
        }).collect();
        let pred: Vec<Pt> = predicted.iter().map(|&(sx, sy, p)| {
            let (x, y) = to_page(sx, sy, ed);
            Pt::new(x, y, if pressure_on { p } else { 0.55 })
        }).collect();
        let t = now();
        let mut erase_segs = Vec::new();
        let mut cursor = None;
        match ed.op.as_mut() {
            Some(Op::Ink { pointer: pid, stroke, smoother, raw, hold_t, hold_pos, quick, straight, predicted, .. }) if *pid == pointer => {
                for &(x, y, p) in &local {
                    let pt = Pt::new(x, y, p);
                    raw.push(pt);
                    if dist(hold_pos.0, hold_pos.1, x, y) * s as f32 > 4.0 {
                        *hold_pos = (x, y);
                        *hold_t = t;
                    }
                    if *straight || *quick == Some(Shape::Line) {
                        let first = stroke.pts[0];
                        stroke.pts = vec![first, Pt::new(x, y, first.p)];
                    } else if quick.is_none() {
                        stroke.pts.push(smoother.push(pt));
                    }
                }
                *predicted = if quick.is_none() && !*straight { pred } else { Vec::new() };
            }
            Some(Op::Erase { pointer: pid, last, .. }) if *pid == pointer => {
                for &(x, y, _) in &local {
                    erase_segs.push((*last, (x, y)));
                    *last = (x, y);
                }
                if let Some(&(sx, sy, _)) = pts.last() {
                    cursor = Some((sx, sy, er_cursor as f64 * s));
                }
            }
            Some(Op::Lasso { pointer: pid, pts: lp, .. }) if *pid == pointer => {
                for &(x, y, _) in &local {
                    let l = *lp.last().unwrap();
                    if dist(l.0, l.1, x, y) * s as f32 > 3.0 {
                        lp.push((x, y));
                    }
                }
            }
            Some(Op::MoveSel { pointer: pid, start, scale, xf }) if *pid == pointer => {
                if let (Some(&(x, y, _)), Some(sel)) = (local.last(), &ed.selection) {
                    if *scale {
                        let (ax, ay) = (sel.bbox.x0, sel.bbox.y0);
                        let d0 = dist(ax, ay, start.0, start.1).max(1.0);
                        let f = (dist(ax, ay, x, y) / d0).clamp(0.1, 10.0);
                        *xf = Xf::scale_about(f, f, ax, ay);
                    } else {
                        *xf = Xf::translate(x - start.0, y - start.1);
                    }
                    ed.dirty = true;
                }
            }
            Some(Op::Laser { pointer: pid }) if *pid == pointer => {
                for &(sx, sy, _) in pts {
                    ed.laser.push((sx, sy, t));
                }
            }
            _ => return,
        }
        for (a, b) in erase_segs {
            ed.erase_segment(a, b);
        }
        if !matches!(ed.op, Some(Op::Laser { .. }) | Some(Op::MoveSel { .. })) {
            ed.draw_live(cursor);
        }
        self.request_frame();
    }

    /// Ende der Eingabe.
    pub fn end_op(&mut self, pointer: i32) {
        let Some(ed) = &mut self.ed else { return };
        let mine = match &ed.op {
            Some(Op::Ink { pointer: p, .. } | Op::Erase { pointer: p, .. } | Op::Lasso { pointer: p, .. } | Op::MoveSel { pointer: p, .. } | Op::Laser { pointer: p }) => *p == pointer,
            None => false,
        };
        if !mine {
            return;
        }
        let op = ed.op.take().unwrap();
        let mut auto_back = false;
        match op {
            Op::Ink { page, mut stroke, raw, quick, shape_tool, straight, .. } => {
                // Glättung zum letzten echten Punkt aufholen, damit Striche nicht zu kurz enden
                if quick.is_none() && !straight {
                    if let (Some(l), Some(r)) = (stroke.pts.last().copied(), raw.last().copied()) {
                        if dist(l.x, l.y, r.x, r.y) > 0.3 {
                            let mut m = r;
                            m.p = l.p;
                            stroke.pts.push(Pt::new((l.x + r.x) * 0.5, (l.y + r.y) * 0.5, l.p));
                            stroke.pts.push(m);
                        }
                    }
                }
                if shape_tool && quick.is_none() {
                    if let Some((shape, pts)) = recognize(&raw) {
                        stroke.pts = pts;
                        let name = shape.name_de();
                        set_timeout(0, move || {
                            with_app(|a| a.toast(name));
                        });
                    }
                }
                if let Some(p) = ed.nb.page_mut(page) {
                    let order = p.max_order() + 1.0;
                    let _ = p;
                    let id = ed.nb.alloc_id();
                    let item = Item { id, order, content: Content::Stroke(stroke) };
                    if let Some(p) = ed.nb.page_mut(page) {
                        p.insert([item.clone()]);
                    }
                    ed.push(Action::Items { page, removed: vec![], added: vec![item] });
                }
            }
            Op::Erase { page, before, .. } => {
                ed.live_page = None;
                if let Some(p) = ed.nb.page_mut(page) {
                    let now_ids: Vec<u32> = p.items.iter().map(|i| i.id).collect();
                    let before_ids: Vec<u32> = before.iter().map(|i| i.id).collect();
                    let removed: Vec<Item> = before.into_iter().filter(|i| !now_ids.contains(&i.id)).collect();
                    let added: Vec<Item> = p.items.iter().filter(|i| !before_ids.contains(&i.id)).cloned().collect();
                    if !removed.is_empty() || !added.is_empty() {
                        ed.push(Action::Items { page, removed, added });
                    }
                }
                ed.dirty = true;
                auto_back = true;
            }
            Op::Lasso { page, pts, .. } => {
                let len: f32 = pts.windows(2).map(|w| dist(w[0].0, w[0].1, w[1].0, w[1].1)).sum();
                if len * (ed.view.s() as f32) < 12.0 {
                    if !ed.clipboard.is_empty() {
                        ed.paste_at = Some((page, pts[0].0, pts[0].1));
                    }
                } else {
                    ed.select_in(page, &pts, (self.prefs.lasso_ink, self.prefs.lasso_text, self.prefs.lasso_img));
                }
                ed.dirty = true;
            }
            Op::MoveSel { xf, .. } => {
                if xf != Xf::IDENTITY {
                    ed.transform_selection(&xf);
                }
                ed.dirty = true;
            }
            Op::Laser { .. } => {}
        }
        ed.clear_live();
        if auto_back && self.prefs.er_auto_back && self.tool == Tool::Eraser && !self.hold_eraser {
            let back = if self.prev_tool == Tool::Eraser { Tool::Pen } else { self.prev_tool };
            self.set_tool(back);
        }
        self.after_edit();
    }

    /// Abbrechen (z. B. Handballen erkannt).
    pub fn cancel_op(&mut self) {
        let Some(ed) = &mut self.ed else { return };
        if let Some(Op::Erase { page, before, .. }) = ed.op.take() {
            if let Some(p) = ed.nb.page_mut(page) {
                p.items = before;
                p.touch();
            }
        }
        ed.live_page = None;
        ed.clear_live();
        ed.dirty = true;
        self.request_frame();
    }

    /// Nach jeder Änderung: Oberfläche, Autospeichern.
    pub fn after_edit(&mut self) {
        self.mark_unsaved();
        self.refresh_history_buttons();
        self.position_overlays();
        self.request_frame();
    }

    /// QuickShape: Stift am Strichende halten → saubere Form.
    pub fn check_quickshape(&mut self) {
        if !self.settings.quickshape {
            return;
        }
        let Some(ed) = &mut self.ed else { return };
        let s = ed.view.s() as f32;
        let mut found = None;
        if let Some(Op::Ink { stroke, raw, hold_t, quick, straight, .. }) = &mut ed.op {
            if quick.is_none() && !*straight && now() - *hold_t > 600.0 && raw.len() > 5 && stroke.length() * s > 30.0 {
                match recognize(raw) {
                    Some((shape, pts)) => {
                        stroke.pts = pts;
                        found = Some(shape.name_de());
                        *quick = Some(shape);
                    }
                    None => *hold_t = f64::MAX / 2.0,
                }
            }
        }
        if let Some(name) = found {
            ed.draw_live(None);
            vibrate(12);
            self.toast(name);
        }
    }

    // -----------------------------------------------------------------------
    // Auswahl
    // -----------------------------------------------------------------------

    pub fn selection_action(&mut self, act: &str, arg: &str) {
        let Some(ed) = &mut self.ed else { return };
        match act {
            "sel-delete" => ed.delete_selection(),
            "sel-copy" => {
                ed.copy_selection();
                self.toast("Kopiert – mit dem Lasso auf eine freie Stelle tippen zum Einfügen");
            }
            "sel-cut" => {
                ed.copy_selection();
                ed.delete_selection();
                self.toast("Ausgeschnitten – mit dem Lasso auf eine freie Stelle tippen zum Einfügen");
            }
            "sel-dup" => ed.duplicate_selection(),
            "sel-color" => {
                if let Some(c) = Color::from_hex(arg) {
                    ed.recolor_selection(c);
                }
            }
            "paste" => {
                if let Some((page, x, y)) = ed.paste_at.take() {
                    ed.paste(page, x, y);
                    self.tool = Tool::Lasso;
                    self.refresh_toolbar();
                }
                self.hide_paste();
            }
            _ => {}
        }
        self.after_edit();
    }

    pub fn hide_paste(&mut self) {
        show("paste-btn", false);
        if let Some(ed) = &mut self.ed {
            if self.screen == Screen::Editor && ed.op.is_none() {
                ed.paste_at = None;
            }
        }
    }

    /// Auswahlmenü, Einfügen-Knopf und Textfeld an die Ansicht anpassen.
    pub fn position_overlays(&mut self) {
        let Some(ed) = &self.ed else { return };
        let menu = el("sel-menu");
        match (&ed.selection, &ed.op) {
            (Some(sel), None) => {
                if let Some(i) = ed.page_index(sel.page) {
                    let (x0, y0) = ed.page_to_screen(i, sel.bbox.x0, sel.bbox.y0);
                    let (x1, _) = ed.page_to_screen(i, sel.bbox.x1, sel.bbox.y1);
                    let left = ((x0 + x1) * 0.5).clamp(170.0, ed.vw - 170.0);
                    let top = (y0 - 64.0).max(ed.top_pad);
                    style(&menu, "left", &format!("{}px", left));
                    style(&menu, "top", &format!("{}px", top));
                    set_class(&menu, "hidden", false);
                }
            }
            _ => set_class(&menu, "hidden", true),
        }
        if let Some((page, x, y)) = ed.paste_at {
            if let Some(i) = ed.page_index(page) {
                let (sx, sy) = ed.page_to_screen(i, x, y);
                let b = el("paste-btn");
                style(&b, "left", &format!("{}px", sx));
                style(&b, "top", &format!("{}px", sy - 52.0));
                set_class(&b, "hidden", false);
            }
        }
        if let Some(te) = &ed.text_edit {
            if let Some(i) = ed.page_index(te.page) {
                let s = ed.view.s();
                let (sx, sy) = ed.page_to_screen(i, te.x, te.y);
                let ta = el("text-editor");
                style(&ta, "left", &format!("{}px", sx));
                style(&ta, "top", &format!("{}px", sy));
                style(&ta, "width", &format!("{}px", te.w as f64 * s));
                style(&ta, "font-size", &format!("{}px", te.size as f64 * s));
                style(&ta, "font-weight", if te.bold { "650" } else { "420" });
                style(&ta, "color", &rgba_css(te.color));
                autosize(&ta);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Text
    // -----------------------------------------------------------------------

    pub fn start_text(&mut self, page_idx: usize, x: f32, y: f32, existing: Option<u32>) {
        let p = &self.prefs;
        let (color, size, bold) = (p.text_colors[p.text_ci], p.text_size, p.text_bold);
        let Some(ed) = &mut self.ed else { return };
        let page = &ed.nb.pages[page_idx];
        let te = match existing.and_then(|id| page.get(id)).map(|it| (it.id, it.content.clone())) {
            Some((id, Content::Text(t))) => {
                set_textarea(&t.text);
                TextEdit { page: page.id, item: Some(id), x: t.x, y: t.y, w: t.w, size: t.size, color: t.color, bold: t.bold }
            }
            _ => {
                set_textarea("");
                let w = (ed.nb.format.w - x - 16.0).clamp(80.0, 320.0);
                TextEdit { page: page.id, item: None, x, y: y - size * 0.5, w, size, color, bold }
            }
        };
        ed.text_edit = Some(te);
        ed.dirty = true;
        show("text-editor", true);
        self.position_overlays();
        let ta: HtmlTextAreaElement = el("text-editor").unchecked_into();
        let _ = ta.focus();
        self.request_frame();
    }

    pub fn commit_text(&mut self) {
        let ta: HtmlTextAreaElement = el("text-editor").unchecked_into();
        let text = ta.value().trim_end().to_string();
        show("text-editor", false);
        let _ = ta.blur();
        let Some(ed) = &mut self.ed else { return };
        let Some(te) = ed.text_edit.take() else { return };
        let Some(page) = ed.nb.page_mut(te.page) else { return };
        let old = te.item.and_then(|id| page.get(id).cloned());
        let tb = TextBox { x: te.x, y: te.y, w: te.w, size: te.size, color: te.color, bold: te.bold, text };
        let mut added = Vec::new();
        if !tb.text.is_empty() {
            let (id, order) = match &old {
                Some(o) => (o.id, o.order),
                None => (0, page.max_order() + 1.0),
            };
            let id = if id == 0 { ed.nb.alloc_id() } else { id };
            added.push(Item { id, order, content: Content::Text(tb) });
        }
        let removed: Vec<Item> = old.into_iter().collect();
        if removed.is_empty() && added.is_empty() {
            ed.dirty = true;
            return;
        }
        if removed.first().map(|r| &r.content) == added.first().map(|a| &a.content) {
            ed.dirty = true;
            return;
        }
        if let Some(page) = ed.nb.page_mut(te.page) {
            let ids: Vec<u32> = removed.iter().map(|i| i.id).collect();
            page.remove_ids(&ids);
            page.insert(added.clone());
        }
        ed.push(Action::Items { page: te.page, removed, added });
        self.after_edit();
    }

    // -----------------------------------------------------------------------
    // Bilder
    // -----------------------------------------------------------------------

    /// Fügt ein Bild (PNG) auf der aktuellen Seite ein und wählt es aus.
    pub fn insert_image(&mut self, png: Vec<u8>, pw: f32, ph: f32) {
        let Some(ed) = &mut self.ed else { return };
        let i = ed.cur_page;
        let (w, h) = (ed.nb.format.w, ed.nb.format.h);
        let mut iw = pw * 0.75;
        let mut ih = ph * 0.75;
        let f = (w * 0.7 / iw).min(h * 0.6 / ih).min(1.0);
        iw *= f;
        ih *= f;
        // Mitte des sichtbaren Bereichs der Seite
        let (cx, cy) = ed.to_nb(ed.vw * 0.5, ed.vh * 0.5);
        let cy = cy - ed.page_top(i);
        let x = (cx as f32 - iw * 0.5).clamp(0.0, (w - iw).max(0.0));
        let y = (cy as f32 - ih * 0.5).clamp(0.0, (h - ih).max(0.0));
        let key = hash_bytes(&png);
        let id = ed.nb.alloc_id();
        let page = &mut ed.nb.pages[i];
        let item = Item { id, order: page.max_order() + 1.0, content: Content::Image(ImageItem { x, y, w: iw, h: ih, png: Rc::new(png), key }) };
        page.insert([item.clone()]);
        let pid = page.id;
        ed.push(Action::Items { page: pid, removed: vec![], added: vec![item.clone()] });
        ed.selection = Some(Selection { page: pid, ids: vec![id], bbox: item.bbox() });
        self.tool = Tool::Lasso;
        self.refresh_toolbar();
        self.after_edit();
        self.toast("Bild eingefügt – ziehen zum Verschieben, Ecke zum Skalieren");
    }
}

fn set_textarea(v: &str) {
    let ta: HtmlTextAreaElement = el("text-editor").unchecked_into();
    ta.set_value(v);
}

pub fn autosize(ta: &HtmlElement) {
    style(ta, "height", "auto");
    let h = ta.scroll_height();
    style(ta, "height", &format!("{}px", h + 2));
}

impl Editor {
    /// Checklisten-Kästchen an einem Bildschirmpunkt umschalten. `true`, wenn eines getroffen wurde.
    pub fn toggle_check_at(&mut self, sx: f64, sy: f64) -> bool {
        let Some((i, x, y)) = self.hit_page(sx, sy, 0.0) else { return false };
        let (w, h) = (self.nb.format.w, self.nb.format.h);
        let page = &self.nb.pages[i];
        // Trefferfläche mindestens ~11 Bildschirmpixel um das Kästchen (gut mit dem Finger)
        let pad = (11.0 / self.view.s() as f32).max(2.0);
        let Some(row) = page.template.check_rows(w, h).iter().position(|r| r.box_contains(x, y, pad)) else { return false };
        let row = row as u16;
        self.set_page_meta(i, |m| {
            if let Some(pos) = m.checks.iter().position(|c| *c == row) {
                m.checks.remove(pos);
            } else {
                m.checks.push(row);
                m.checks.sort_unstable();
            }
        });
        self.dirty = true;
        true
    }

    /// Radiert entlang der Strecke a→b (Seitenkoordinaten).
    pub fn erase_segment(&mut self, a: (f32, f32), b: (f32, f32)) {
        let Some(Op::Erase { page, precise, hl_only, radius, .. }) = &self.op else { return };
        let (page, precise, hl_only, radius) = (*page, *precise, *hl_only, *radius);
        let Some(pi) = self.page_index(page) else { return };
        let mut changes: Vec<(u32, f64, Vec<Stroke>)> = Vec::new();
        for it in &self.nb.pages[pi].items {
            if let Content::Stroke(s) = &it.content {
                if hl_only && s.pen != PenKind::Highlighter {
                    continue;
                }
                if precise {
                    if let Some(pieces) = s.erase_part(a, b, radius) {
                        changes.push((it.id, it.order, pieces));
                    }
                } else if s.hit(a, b, radius) {
                    changes.push((it.id, it.order, Vec::new()));
                }
            }
        }
        if changes.is_empty() {
            return;
        }
        let mut new_items = Vec::new();
        for (_, order, pieces) in &changes {
            for (k, s) in pieces.iter().enumerate() {
                let id = self.nb.alloc_id();
                new_items.push(Item { id, order: order + (k as f64 + 1.0) * 1e-7, content: Content::Stroke(s.clone()) });
            }
        }
        let ids: Vec<u32> = changes.iter().map(|c| c.0).collect();
        let p = &mut self.nb.pages[pi];
        p.remove_ids(&ids);
        p.insert(new_items);
        self.dirty = true;
    }

    pub fn select_in(&mut self, page: u32, poly: &[(f32, f32)], kinds: (bool, bool, bool)) {
        let Some(pi) = self.page_index(page) else { return };
        let mut ids = Vec::new();
        let mut bbox = Rect::EMPTY;
        for it in &self.nb.pages[pi].items {
            let take = match &it.content {
                Content::Stroke(s) if kinds.0 => {
                    let inside = s.pts.iter().filter(|p| point_in_polygon(p.x, p.y, poly)).count();
                    inside * 2 >= s.pts.len().max(1)
                }
                Content::Text(_) | Content::Image(_) if (kinds.1 && matches!(it.content, Content::Text(_))) || (kinds.2 && matches!(it.content, Content::Image(_))) => {
                    let (cx, cy) = it.bbox().center();
                    point_in_polygon(cx, cy, poly)
                }
                _ => false,
            };
            if take {
                ids.push(it.id);
                bbox = bbox.union(&it.bbox());
            }
        }
        self.selection = if ids.is_empty() { None } else { Some(Selection { page, ids, bbox }) };
    }

    fn selected_items(&self) -> Vec<Item> {
        let Some(sel) = &self.selection else { return Vec::new() };
        let Some(pi) = self.page_index(sel.page) else { return Vec::new() };
        self.nb.pages[pi].items.iter().filter(|i| sel.ids.contains(&i.id)).cloned().collect()
    }

    fn replace_selected(&mut self, new_items: Vec<Item>) {
        let Some(sel) = &self.selection else { return };
        let page = sel.page;
        let old = self.selected_items();
        if let Some(p) = self.nb.page_mut(page) {
            let ids: Vec<u32> = old.iter().map(|i| i.id).collect();
            p.remove_ids(&ids);
            p.insert(new_items.clone());
        }
        let mut bbox = Rect::EMPTY;
        for i in &new_items {
            bbox = bbox.union(&i.bbox());
        }
        if let Some(sel) = &mut self.selection {
            sel.bbox = bbox;
        }
        self.push(Action::Items { page, removed: old, added: new_items });
    }

    pub fn transform_selection(&mut self, xf: &Xf) {
        let items: Vec<Item> = self.selected_items().iter().map(|i| i.transformed(xf)).collect();
        self.replace_selected(items);
    }

    pub fn recolor_selection(&mut self, c: Color) {
        let items: Vec<Item> = self
            .selected_items()
            .into_iter()
            .map(|mut it| {
                match &mut it.content {
                    Content::Stroke(s) => {
                        let a = s.color[3];
                        s.color = color4(c, a);
                    }
                    Content::Text(t) => t.color = color4(c, 255),
                    _ => {}
                }
                it
            })
            .collect();
        self.replace_selected(items);
    }

    pub fn delete_selection(&mut self) {
        let Some(sel) = self.selection.take() else { return };
        let old = {
            let Some(pi) = self.page_index(sel.page) else { return };
            self.nb.pages[pi].remove_ids(&sel.ids)
        };
        self.push(Action::Items { page: sel.page, removed: old, added: vec![] });
    }

    pub fn copy_selection(&mut self) {
        self.clipboard = self.selected_items();
    }

    pub fn duplicate_selection(&mut self) {
        let Some(sel) = &self.selection else { return };
        let page = sel.page;
        let (x, y) = (sel.bbox.x0 + 16.0, sel.bbox.y0 + 16.0);
        self.clipboard = self.selected_items();
        self.paste(page, x, y);
    }

    pub fn paste(&mut self, page: u32, x: f32, y: f32) {
        if self.clipboard.is_empty() {
            return;
        }
        let mut bb = Rect::EMPTY;
        for i in &self.clipboard {
            bb = bb.union(&i.bbox());
        }
        let xf = Xf::translate(x - bb.x0, y - bb.y0);
        let Some(pi) = self.page_index(page) else { return };
        let mut order = self.nb.pages[pi].max_order();
        let mut items = Vec::new();
        for it in self.clipboard.clone() {
            order += 1.0;
            let id = self.nb.alloc_id();
            let mut n = it.transformed(&xf);
            n.id = id;
            n.order = order;
            items.push(n);
        }
        self.nb.pages[pi].insert(items.clone());
        let mut bbox = Rect::EMPTY;
        for i in &items {
            bbox = bbox.union(&i.bbox());
        }
        self.selection = Some(Selection { page, ids: items.iter().map(|i| i.id).collect(), bbox });
        self.push(Action::Items { page, removed: vec![], added: items });
    }
}
