//! Mindmap-Werkzeug: Blasen anlegen, Äste ergänzen, Text, Farbe, verschieben (Verbindungen folgen), löschen.

use super::editor::{Editor, Op};
use super::*;
use crate::engine::history::Action;
use crate::engine::note::Node;
use crate::engine::{Content, Item};

/// Blasenfarben (kräftig, mit weißer Schrift).
pub const MIND_COLORS: [[u8; 4]; 8] = [
    [109, 92, 255, 255],
    [36, 160, 237, 255],
    [31, 178, 110, 255],
    [240, 152, 25, 255],
    [240, 88, 88, 255],
    [214, 72, 186, 255],
    [0, 168, 155, 255],
    [118, 130, 155, 255],
];

const FONT_PT: f32 = 15.0;

impl Editor {
    pub fn node(&self, page: u32, id: u32) -> Option<&Node> {
        let pi = self.page_index(page)?;
        match &self.nb.pages[pi].get(id)?.content {
            Content::Node(n) => Some(n),
            _ => None,
        }
    }

    /// Oberste Blase an einem Bildschirmpunkt.
    pub fn node_at(&self, sx: f64, sy: f64) -> Option<(u32, u32)> {
        let (i, x, y) = self.hit_page(sx, sy, 0.0)?;
        let p = &self.nb.pages[i];
        p.items.iter().rev().find_map(|it| match &it.content {
            Content::Node(n) if x >= n.x && x <= n.x + n.w && y >= n.y && y <= n.y + n.h => Some((p.id, it.id)),
            _ => None,
        })
    }

    /// Breite/Höhe einer Blase für einen Text (gemessen mit der Seitenschrift).
    pub fn node_size(&self, text: &str) -> (f32, f32) {
        self.ctx.set_font(&format!("650 {}px {}", FONT_PT, super::render::FONT));
        let lines: Vec<&str> = if text.is_empty() { vec![" "] } else { text.split('\n').collect() };
        let widest = lines.iter().map(|l| self.ctx.measure_text(l).map(|m| m.width()).unwrap_or(0.0)).fold(0.0, f64::max) as f32;
        let pad = FONT_PT * 0.8;
        let w = (widest + pad * 2.0).clamp(70.0, 280.0);
        // grob: wie viele Zeilen nach dem Umbruch
        let wraps: usize = lines
            .iter()
            .map(|l| {
                let lw = self.ctx.measure_text(l).map(|m| m.width()).unwrap_or(0.0) as f32;
                ((lw / (w - pad * 2.0)).ceil() as usize).max(1)
            })
            .sum();
        let h = (FONT_PT * 1.25 * wraps as f32 + FONT_PT * 1.4).max(FONT_PT / 0.32 * 0.9);
        (w, h.max(40.0))
    }
}

impl App {
    pub fn mind_begin(&mut self, pointer: i32, sx: f64, sy: f64) {
        let Some(ed) = &mut self.ed else { return };
        let hit = ed.node_at(sx, sy);
        let Some((i, x, y)) = ed.hit_page(sx, sy, 0.0) else { return };
        let page = hit.map(|h| h.0).unwrap_or(ed.nb.pages[i].id);
        let orig = hit.and_then(|(_, id)| ed.nb.pages[i].get(id).cloned());
        ed.op = Some(Op::Mind { pointer, page, id: hit.map(|h| h.1), start: (sx, sy), at: (x, y), orig, moved: false });
        let _ = el("editor").set_pointer_capture(pointer);
    }

    pub fn mind_move(&mut self, _pointer: i32, sx: f64, sy: f64) {
        let Some(ed) = &mut self.ed else { return };
        let s = ed.view.s() as f32;
        let Some(Op::Mind { page, id: Some(id), start, orig: Some(orig), moved, .. }) = &mut ed.op else { return };
        let (dx, dy) = ((sx - start.0) as f32 / s, (sy - start.1) as f32 / s);
        if !*moved && (dx * s).hypot(dy * s) < 6.0 {
            return;
        }
        *moved = true;
        let (page, id) = (*page, *id);
        let moved_item = orig.transformed(&crate::engine::Xf::translate(dx, dy));
        if let Some(p) = ed.nb.page_mut(page) {
            if let Some(it) = p.items.iter_mut().find(|it| it.id == id) {
                it.content = moved_item.content;
            }
            p.touch();
        }
        ed.dirty = true;
        self.mind_sel = Some((page, id));
        self.position_overlays();
        self.request_frame();
    }

    pub fn mind_end(&mut self) {
        let Some(ed) = &mut self.ed else { return };
        let Some(Op::Mind { page, id, at, orig, moved, .. }) = ed.op.take() else { return };
        match (id, moved) {
            (Some(id), true) => {
                // Verschieben als rückgängig machbare Aktion
                let now = ed.nb.page_mut(page).and_then(|p| p.get(id).cloned());
                if let (Some(old), Some(new)) = (orig, now) {
                    ed.push(Action::Items { page, removed: vec![old], added: vec![new] });
                }
                self.mind_sel = Some((page, id));
                self.after_edit();
            }
            (Some(id), false) => {
                // Antippen: auswählen (zweites Antippen = Text bearbeiten)
                if self.mind_sel == Some((page, id)) {
                    self.mind_edit_text(page, id, false);
                } else {
                    self.mind_sel = Some((page, id));
                }
                if let Some(ed) = &mut self.ed {
                    ed.dirty = true;
                }
                self.position_overlays();
                self.request_frame();
            }
            (None, _) => {
                if self.mind_sel.take().is_some() {
                    // Tipp ins Leere hebt nur die Auswahl auf
                    if let Some(ed) = &mut self.ed {
                        ed.dirty = true;
                    }
                    self.position_overlays();
                    self.request_frame();
                    return;
                }
                let (w, h) = ed.node_size("Thema");
                let node = Node { x: at.0 - w * 0.5, y: at.1 - h * 0.5, w, h, text: "Thema".into(), color: MIND_COLORS[0], parent: 0 };
                let id = self.mind_insert(page, node);
                self.mind_sel = Some((page, id));
                self.mind_edit_text(page, id, true);
            }
        }
    }

    fn mind_insert(&mut self, page: u32, node: Node) -> u32 {
        let Some(ed) = &mut self.ed else { return 0 };
        let id = ed.nb.alloc_id();
        let Some(p) = ed.nb.page_mut(page) else { return 0 };
        let item = Item { id, order: p.max_order() + 1.0, content: Content::Node(node), pinned: false };
        p.insert([item.clone()]);
        ed.push(Action::Items { page, removed: vec![], added: vec![item] });
        self.after_edit();
        id
    }

    /// Neuen Ast an die ausgewählte Blase hängen.
    pub fn mind_add_child(&mut self) {
        let Some((page, pid)) = self.mind_sel else { return };
        let Some(ed) = &self.ed else { return };
        let Some(parent) = ed.node(page, pid).cloned() else { return };
        let Some(pi) = ed.nb.page_index(page) else { return };
        let siblings: Vec<Node> = ed.nb.pages[pi]
            .items
            .iter()
            .filter_map(|it| match &it.content {
                Content::Node(n) if n.parent == pid => Some(n.clone()),
                _ => None,
            })
            .collect();
        let k = siblings.len();
        let (w, h) = ed.node_size("Idee");
        let (pw, ph) = ed.page_size_id(page);
        let (pcx, pcy) = parent.center();
        // Richtung: Wurzel abwechselnd rechts/links, sonst weg von der eigenen Elternblase
        let dir = if parent.parent == 0 {
            if k.is_multiple_of(2) { 1.0 } else { -1.0 }
        } else {
            let gp = ed.node(page, parent.parent).map(|g| g.center().0).unwrap_or(pcx - 1.0);
            if pcx >= gp { 1.0 } else { -1.0 }
        };
        let same_side = siblings.iter().filter(|s| (s.center().0 - pcx) * dir > 0.0).count() as f32;
        let gap_x = parent.w * 0.5 + w * 0.5 + 60.0;
        let step_y = h + 22.0;
        // Geschwister untereinander, mittig um die Elternblase verteilt
        let offset = if same_side == 0.0 { 0.0 } else if same_side as usize % 2 == 1 { -((same_side + 1.0) / 2.0).floor() * step_y } else { (same_side / 2.0) * step_y };
        let cx = (pcx + dir * gap_x).clamp(w * 0.5 + 4.0, pw - w * 0.5 - 4.0);
        let cy = (pcy + offset).clamp(h * 0.5 + 4.0, ph - h * 0.5 - 4.0);
        let color = if parent.parent == 0 { MIND_COLORS[(k + 1) % MIND_COLORS.len()] } else { parent.color };
        let node = Node { x: cx - w * 0.5, y: cy - h * 0.5, w, h, text: "Idee".into(), color, parent: pid };
        let id = self.mind_insert(page, node);
        self.mind_sel = Some((page, id));
        self.mind_edit_text(page, id, true);
    }

    pub fn mind_edit_text(&mut self, page: u32, id: u32, new: bool) {
        let cur = self.ed.as_ref().and_then(|e| e.node(page, id)).map(|n| n.text.clone()).unwrap_or_default();
        let visual = "<div class=\"dlg-icon accent\"><svg><use href=\"#i-mind\"/></svg></div>";
        self.open_dialog(visual, if new { "Neue Blase" } else { "Text der Blase" }, "Was soll in der Blase stehen?", "Fertig", false, Some(&cur), Pending::NodeText(page, id));
    }

    /// Text übernehmen und Größe anpassen (Mitte bleibt).
    pub fn mind_set_text(&mut self, page: u32, id: u32, text: &str) {
        let text: String = text.trim().chars().take(200).collect();
        let text = if text.is_empty() { "Idee".to_string() } else { text };
        self.mind_update(page, id, |ed, n| {
            let (w, h) = ed.node_size(&text);
            let (cx, cy) = n.center();
            n.text = text.clone();
            n.x = cx - w * 0.5;
            n.y = cy - h * 0.5;
            n.w = w;
            n.h = h;
        });
    }

    pub fn mind_set_color(&mut self, idx: usize) {
        let Some((page, id)) = self.mind_sel else { return };
        let c = MIND_COLORS[idx % MIND_COLORS.len()];
        self.mind_update(page, id, |_, n| n.color = c);
    }

    fn mind_update(&mut self, page: u32, id: u32, f: impl FnOnce(&Editor, &mut Node)) {
        let Some(ed) = &mut self.ed else { return };
        let Some(old) = ed.nb.page_mut(page).and_then(|p| p.get(id).cloned()) else { return };
        let Content::Node(mut n) = old.content.clone() else { return };
        f(ed, &mut n);
        let new = Item { content: Content::Node(n), ..old.clone() };
        if let Some(p) = ed.nb.page_mut(page) {
            if let Some(it) = p.items.iter_mut().find(|it| it.id == id) {
                *it = new.clone();
            }
            p.touch();
        }
        ed.push(Action::Items { page, removed: vec![old], added: vec![new] });
        self.after_edit();
    }

    /// Blase löschen; ihre Äste hängen danach an der Elternblase.
    pub fn mind_delete(&mut self) {
        let Some((page, id)) = self.mind_sel.take() else { return };
        let Some(ed) = &mut self.ed else { return };
        let Some(pi) = ed.nb.page_index(page) else { return };
        let Some(Content::Node(gone)) = ed.nb.pages[pi].get(id).map(|i| i.content.clone()) else { return };
        let mut removed = Vec::new();
        let mut added = Vec::new();
        let p = &mut ed.nb.pages[pi];
        for it in p.items.iter_mut() {
            if matches!(&it.content, Content::Node(n) if n.parent == id) {
                removed.push(it.clone());
                if let Content::Node(n) = &mut it.content {
                    n.parent = gone.parent;
                }
                added.push(it.clone());
            }
        }
        removed.extend(p.remove_ids(&[id]));
        p.touch();
        ed.push(Action::Items { page, removed, added });
        self.after_edit();
    }

    /// Menü unter der ausgewählten Blase platzieren.
    pub fn position_mind_menu(&mut self) {
        let menu = el("mind-menu");
        let sel = self.mind_sel.filter(|_| self.tool == Tool::Mind);
        let Some((page, id)) = sel else {
            set_class(&menu, "hidden", true);
            return;
        };
        let Some(ed) = &self.ed else { return };
        let (Some(n), Some(i)) = (ed.node(page, id), ed.page_index(page)) else {
            set_class(&menu, "hidden", true);
            return;
        };
        let (x0, _) = ed.page_to_screen(i, n.x, n.y);
        let (x1, y1) = ed.page_to_screen(i, n.x + n.w, n.y + n.h);
        set_class(&menu, "hidden", false);
        let mw = menu.client_width() as f64;
        let left = ((x0 + x1) * 0.5 - mw * 0.5).clamp(8.0, (ed.vw - mw - 8.0).max(8.0));
        style(&menu, "left", &format!("{}px", left + ed.ox));
        style(&menu, "top", &format!("{}px", (y1 + 12.0).min(ed.vh - 70.0)));
    }
}
