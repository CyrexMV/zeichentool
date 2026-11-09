//! Lernkarten: Vorder- und Rückseite sind Bereiche im Notizbuch (per Lasso gewählt).
//! Lernen nach dem Karteikasten-Prinzip (Leitner): gewusst → ein Fach weiter, sonst zurück auf Fach 1.

use super::editor::Editor;
use super::render::{draw_page, Caches, DrawOpts};
use super::*;
use crate::engine::{Card, Region};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

pub struct Session {
    pub queue: Vec<u32>,
    pub idx: usize,
    pub flipped: bool,
    pub right: u32,
}

const LEVELS: [(&str, &str); 5] = [("Neu / Nochmal", "#e5484d"), ("Fach 2", "#ff9f43"), ("Fach 3", "#f5c400"), ("Fach 4", "#2fb5ff"), ("Gelernt", "#1fbf75")];

impl Editor {
    /// Zeichnet einen Seitenbereich passend in ein Canvas.
    pub fn render_region(&mut self, canvas: &HtmlCanvasElement, region: &Region, images: &mut HashMap<u64, HtmlImageElement>) {
        let dpr = self.dpr;
        let (mut cw, mut ch) = (canvas.client_width() as f64, canvas.client_height() as f64);
        if cw < 10.0 || ch < 10.0 {
            cw = 600.0;
            ch = 375.0;
        }
        canvas.set_width((cw * dpr) as u32);
        canvas.set_height((ch * dpr) as u32);
        let Some(ctx) = canvas.get_context("2d").ok().flatten() else { return };
        let ctx: CanvasRenderingContext2d = ctx.unchecked_into();
        ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).ok();
        ctx.clear_rect(0.0, 0.0, cw * dpr, ch * dpr);
        let Some(pi) = self.nb.page_index(region.page) else { return };
        let r = region.rect;
        let (rw, rh) = (r.w().max(1.0) as f64, r.h().max(1.0) as f64);
        let scale = (cw / rw).min(ch / rh).min(4.0) * dpr;
        let ox = (cw * dpr - rw * scale) * 0.5 - r.x0 as f64 * scale;
        let oy = (ch * dpr - rh * scale) * 0.5 - r.y0 as f64 * scale;
        ctx.set_transform(scale, 0.0, 0.0, scale, ox, oy).ok();
        ctx.save();
        ctx.begin_path();
        let _ = ctx.round_rect_with_f64(r.x0 as f64, r.y0 as f64, rw, rh, 8.0);
        ctx.clip();
        let (w, h) = (self.nb.format.w, self.nb.format.h);
        let page = &self.nb.pages[pi];
        let mut c = Caches { paths: &mut self.paths, tpl: &mut self.tpl_cache, wraps: &mut self.wraps, images };
        draw_page(&ctx, page, w, h, &mut c, &DrawOpts { skip: None, moving: None, clip: r });
        ctx.restore();
    }
}

fn shuffle<T>(v: &mut [T]) {
    for i in (1..v.len()).rev() {
        let j = (js_sys::Math::random() * (i + 1) as f64) as usize;
        v.swap(i, j.min(i));
    }
}

impl App {
    /// Lasso-Auswahl als Vorder- bzw. Rückseite einer Lernkarte übernehmen.
    pub fn selection_to_card(&mut self) {
        let Some(ed) = &mut self.ed else { return };
        let Some(sel) = &ed.selection else { return };
        let region = Region { page: sel.page, rect: sel.bbox.expand(8.0) };
        ed.selection = None;
        ed.dirty = true;
        match ed.card_front.take() {
            None => {
                ed.card_front = Some(region);
                set_class(&el("sel-card"), "pending", true);
                self.toast("Frage gespeichert – jetzt die Antwort einkreisen und wieder „Lernkarte“ tippen");
            }
            Some(front) => {
                let id = ed.nb.alloc_id();
                ed.nb.cards.push(Card { id, front, back: region, level: 0, right: 0, wrong: 0, due: 0.0 });
                ed.nb.modified = wall_clock();
                let n = ed.nb.cards.len();
                set_class(&el("sel-card"), "pending", false);
                self.toast(&format!("Lernkarte erstellt – {} {} in diesem Notizbuch", n, if n == 1 { "Karte" } else { "Karten" }));
                self.mark_unsaved();
            }
        }
        show("sel-menu", false);
        self.request_frame();
    }

    pub fn open_study(&mut self) {
        self.close_popovers();
        self.study_from_lib = false;
        let Some(ed) = &mut self.ed else { return };
        if ed.text_edit.is_some() {
            self.commit_text();
        }
        if let Some(ed) = &mut self.ed {
            ed.nb.sync_template_cards();
        }
        show("study", true);
        self.study = None;
        self.render_study_home();
        self.sync_ui_rects();
    }

    pub fn close_study(&mut self) {
        show("study", false);
        self.study = None;
        if std::mem::take(&mut self.study_from_lib) {
            self.close_editor();
            return;
        }
        self.sync_ui_rects();
        self.request_frame();
    }

    pub fn render_study_home(&mut self) {
        let streak = self.streak_line();
        show("study-home", true);
        show("study-run", false);
        show("study-done", false);
        style(&el("study-bar"), "width", "0%");
        let Some(ed) = &mut self.ed else { return };
        let cards = ed.nb.study_cards();
        let n = cards.len();
        set_text("study-sub", &format!("{} · {} {}", ed.nb.title, n, if n == 1 { "Karte" } else { "Karten" }));
        let mut boxes = String::new();
        for (lvl, (name, color)) in LEVELS.iter().enumerate() {
            let k = cards.iter().filter(|c| c.level as usize == lvl).count();
            let pct = if n == 0 { 0.0 } else { k as f64 / n as f64 * 100.0 };
            boxes.push_str(&format!(
                "<div class=\"box\"><b>{}</b><small>{}</small><div class=\"lvl\"><span style=\"width:{:.0}%;background:{}\"></span></div></div>",
                k, name, pct, color
            ));
        }
        el("study-boxes").set_inner_html(&boxes);
        set_text("study-start-label", &format!("Alle lernen ({})", n));
        let now = wall_clock();
        let due = cards.iter().filter(|c| c.is_due(now)).count();
        let due_mode = self.settings.due_mode;
        show("study-due-btn", due_mode);
        set_text("study-due-label", &format!("Fällige lernen ({})", due));
        set_class(&el("study-due-btn"), "primary", due_mode && due > 0);
        if let Ok(Some(cb)) = dom().query_selector("#study-due-toggle input") {
            cb.unchecked_into::<web_sys::HtmlInputElement>().set_checked(due_mode);
        }
        let next = cards.iter().filter(|c| !c.is_due(now)).map(|c| c.due).fold(f64::MAX, f64::min);
        let info = if !due_mode {
            String::new()
        } else if due > 0 {
            format!("{} {} heute fällig", due, if due == 1 { "Karte ist" } else { "Karten sind" })
        } else if next < f64::MAX {
            let days = ((next - now) / 86_400_000.0).ceil().max(1.0) as i64;
            format!("Alles erledigt – nächste Wiederholung in {} {}", days, if days == 1 { "Tag" } else { "Tagen" })
        } else {
            String::new()
        };
        set_text("study-due-info", &info);
        el("study-streak").set_inner_html(&streak);
        let mut grid = String::new();
        for c in cards.iter() {
            let dots: String = (0..4).map(|i| format!("<i class=\"{}\"></i>", if (i as u8) < c.level { "on" } else { "" })).collect();
            grid.push_str(&format!(
                "<div class=\"mini-card\"><canvas data-card=\"{}\"></canvas><div class=\"meta\"><span class=\"lvl-dots\">{}</span><span>{} ✓ · {} ✗</span><button class=\"del\" data-act=\"fc-del\" data-arg=\"{}\" title=\"Karte löschen\"><svg><use href=\"#i-trash\"/></svg></button></div></div>",
                c.id, dots, c.right, c.wrong, c.id
            ));
        }
        if grid.is_empty() {
            grid = "<p class=\"muted\" style=\"grid-column:1/-1;text-align:center\">Noch keine Lernkarten in diesem Notizbuch.</p>".into();
        }
        el("study-cards").set_inner_html(&grid);
        let fronts: Vec<(u32, Region)> = cards.iter().map(|c| (c.id, c.front)).collect();
        for (id, region) in fronts {
            if let Ok(Some(cv)) = dom().query_selector(&format!("canvas[data-card=\"{}\"]", id)) {
                ed.render_region(&cv.unchecked_into(), &region, &mut self.images);
            }
        }
    }

    /// Lernen starten: `mode` = "all", "hard" (Fach 1–2) oder "due" (fällige Karten).
    pub fn study_start(&mut self, mode: &str) {
        let Some(ed) = &self.ed else { return };
        let now = wall_clock();
        let mut cards: Vec<&Card> = ed
            .nb
            .study_cards()
            .into_iter()
            .filter(|c| match mode {
                "hard" => c.level <= 1,
                "due" => c.is_due(now),
                _ => true,
            })
            .collect();
        if cards.is_empty() {
            self.toast(match mode {
                "hard" => "Keine schwierigen Karten – super!",
                "due" => "Heute ist nichts mehr fällig – gut gemacht!",
                _ => "Noch keine Lernkarten – schreibe zuerst Frage und Antwort auf eine Karte",
            });
            return;
        }
        // nach Fach sortieren, innerhalb des Fachs mischen
        shuffle(&mut cards);
        cards.sort_by_key(|c| c.level);
        let queue = cards.iter().map(|c| c.id).collect();
        self.study = Some(Session { queue, idx: 0, flipped: false, right: 0 });
        show("study-home", false);
        show("study-done", false);
        show("study-run", true);
        self.show_card();
    }

    fn show_card(&mut self) {
        let Some(s) = &self.study else { return };
        let (idx, total, flipped) = (s.idx, s.queue.len(), s.flipped);
        let Some(&cid) = s.queue.get(idx) else { return };
        let Some(ed) = &mut self.ed else { return };
        let Some(card) = ed.nb.cards.iter().find(|c| c.id == cid).cloned() else { return };
        let fc = el("flashcard");
        set_class(&fc, "back", flipped);
        set_text("card-side", if flipped { "Antwort" } else { "Frage" });
        show("study-answer", flipped);
        set_text("study-sub", &format!("Karte {} von {}", idx + 1, total));
        style(&el("study-bar"), "width", &format!("{:.0}%", idx as f64 / total as f64 * 100.0));
        let canvas: HtmlCanvasElement = el("card-canvas").unchecked_into();
        ed.render_region(&canvas, if flipped { &card.back } else { &card.front }, &mut self.images);
    }

    pub fn study_flip(&mut self) {
        if self.study.is_none() {
            return;
        }
        set_class(&el("flashcard"), "flipping", true);
        set_timeout(160, || {
            with_app(|a| {
                if let Some(s) = &mut a.study {
                    s.flipped = !s.flipped;
                }
                a.show_card();
                set_class(&el("flashcard"), "flipping", false);
            });
        });
    }

    pub fn study_answer(&mut self, known: bool) {
        let Some(s) = &mut self.study else { return };
        let Some(&cid) = s.queue.get(s.idx) else { return };
        if known {
            s.right += 1;
        }
        self.record_answer(known);
        let Some(s) = &mut self.study else { return };
        s.idx += 1;
        s.flipped = false;
        let done = s.idx >= s.queue.len();
        let (right, total) = (s.right, s.queue.len());
        if let Some(ed) = &mut self.ed {
            if let Some(c) = ed.nb.cards.iter_mut().find(|c| c.id == cid) {
                c.answer(known, wall_clock());
            }
        }
        self.mark_unsaved();
        if done {
            show("study-run", false);
            show("study-done", true);
            style(&el("study-bar"), "width", "100%");
            set_text("study-done-title", if right as usize == total { "Perfekt – alles gewusst!" } else { "Geschafft!" });
            let plan = if self.settings.due_mode { " Gewusste Karten kommen nach 1, 3, 7 bzw. 21 Tagen wieder dran." } else { "" };
            set_text("study-done-text", &format!("{} von {} Karten gewusst. Nicht gewusste Karten kommen beim nächsten Mal zuerst.{}", right, total, plan));
            el("study-done-streak").set_inner_html(&self.streak_line());
        } else {
            self.show_card();
        }
    }

    pub fn delete_card(&mut self, id: u32) {
        if let Some(ed) = &mut self.ed {
            // Karten eines Lernkarten-Sets sind Seiten: dann die Seite löschen (rückgängig machbar)
            let page = ed.nb.cards.iter().find(|c| c.id == id).filter(|c| ed.nb.is_auto_card(c)).map(|c| c.front.page);
            if let Some(pi) = page.and_then(|p| ed.nb.page_index(p)) {
                if ed.delete_page(pi) {
                    ed.nb.sync_template_cards();
                    self.after_pages_change();
                    self.toast("Karte gelöscht – Rückgängig ist möglich");
                } else {
                    self.toast("Die letzte Karte bleibt – radiere ihren Inhalt einfach weg");
                }
            } else if let Some(ed) = &mut self.ed {
                ed.nb.cards.retain(|c| c.id != id);
            }
        }
        self.mark_unsaved();
        self.render_study_home();
    }
}
