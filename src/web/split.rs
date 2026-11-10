//! Angedockte Seitenleiste und Split-Ansicht (zwei Notizbücher nebeneinander).
//!
//! `App::ed` ist immer der aktive Bereich; der zweite Bereich liegt in `App::ed2`.
//! Tippen in den anderen Bereich tauscht beide – so arbeitet aller übrige Code unverändert
//! mit `self.ed`. Jeder Editor hat eigene Zeichenflächen („view/live“ bzw. „view2/live2“)
//! und kennt seine Lage im Fenster (`ox`, `pane_w`).

use super::editor::Editor;
use super::*;

/// Ab dieser Fensterbreite dockt die Seitenleiste an (sonst schwebt sie über der Seite).
const DOCK_MIN_W: f64 = 900.0;
/// Platz für die angedockte Seitenleiste (links 10 + 212 breit + 10 Abstand).
const DOCK_W: f64 = 232.0;

fn window_w() -> f64 {
    window().inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1280.0)
}

impl App {
    /// Wird die Seitenleiste neben (statt über) der Seite angezeigt?
    pub fn nav_docked(&self) -> bool {
        window_w() >= DOCK_MIN_W && self.ed.as_ref().is_some_and(|e| e.nav_open && e.presenting.is_none())
    }

    /// Bereiche anordnen: Seitenleiste, aktiver und zweiter Editor.
    pub fn layout_panes(&mut self) {
        let ww = window_w();
        let docked = self.nav_docked();
        set_class(&el("editor"), "nav-docked", docked);
        let left0 = if docked { DOCK_W } else { 0.0 };
        let presenting = self.ed.as_ref().is_some_and(|e| e.presenting.is_some());
        let split = self.ed2.is_some() && !presenting;
        let avail = (ww - left0).max(200.0);
        let half = (avail * 0.5).floor();
        let active_left = self.active_left;
        if let Some(ed) = &mut self.ed {
            if split {
                ed.ox = if active_left { left0 } else { left0 + half };
                ed.pane_w = if active_left { half } else { avail - half };
            } else {
                ed.ox = if presenting { 0.0 } else { left0 };
                ed.pane_w = 0.0;
            }
            ed.resize();
            super::input::set_pane_ox(ed.ox);
        }
        if let (Some(ed2), true) = (&mut self.ed2, split) {
            ed2.ox = if active_left { left0 + half } else { left0 };
            ed2.pane_w = if active_left { avail - half } else { half };
            ed2.resize();
        }
        // Zeichenflächen: nur die der vorhandenen Editoren zeigen
        let mut used: Vec<String> = Vec::new();
        for e in [self.ed.as_ref(), if split { self.ed2.as_ref() } else { None }].into_iter().flatten() {
            used.push(e.canvas.id());
            used.push(e.live.id());
        }
        for id in ["view", "live", "view2", "live2"] {
            set_class(&el(id), "hidden", !used.iter().any(|u| u == id));
        }
        show("split-divider", split);
        show("pane-focus", split);
        show("pane-other", split);
        set_class(&el("btn-split"), "active", self.ed2.is_some());
        if split {
            style(&el("split-divider"), "left", &format!("{}px", left0 + half));
            if let Some(ed) = &self.ed {
                let f = el("pane-focus");
                style(&f, "left", &format!("{}px", ed.ox));
                style(&f, "width", &format!("{}px", ed.vw));
            }
            if let Some(ed2) = &self.ed2 {
                let o = el("pane-other");
                style(&o, "left", &format!("{}px", ed2.ox + ed2.vw * 0.5));
                set_text("pane-other-title", &ed2.nb.title);
            }
        }
        // Seitenanzeige mittig und Schieberegler rechts im aktiven Bereich
        if let Some(ed) = &self.ed {
            style(&el("page-pill"), "left", &format!("{}px", ed.ox + ed.vw * 0.5));
            style(&el("replay-bar"), "left", &format!("{}px", ed.ox + ed.vw * 0.5));
            style(&el("scrubber"), "right", &format!("{}px", (ww - ed.ox - ed.vw).max(0.0) + 4.0));
        }
        self.position_overlays();
        self.request_frame();
    }

    /// Gehört das Ziel des Zeigers zum anderen (inaktiven) Bereich?
    pub fn is_other_pane(&self, target_id: &str) -> bool {
        self.ed2.as_ref().is_some_and(|e| e.live.id() == target_id || e.canvas.id() == target_id)
    }

    /// Den anderen Bereich aktiv machen (Werkzeuge, Seitenleiste usw. wirken dann dort).
    pub fn activate_other(&mut self) {
        if self.ed2.is_none() {
            return;
        }
        if self.ed.as_ref().is_some_and(|e| e.text_edit.is_some()) {
            self.commit_text();
        }
        self.close_popovers();
        self.hide_paste();
        if let Some(ed) = &mut self.ed {
            ed.selection = None;
            ed.op = None;
            ed.clear_live();
            ed.dirty = true;
        }
        self.save_now();
        self.remember_clipboard();
        let nav_open = self.ed.as_ref().is_some_and(|e| e.nav_open);
        std::mem::swap(&mut self.ed, &mut self.ed2);
        self.restore_clipboard();
        self.active_left = !self.active_left;
        if let Some(ed) = &mut self.ed {
            ed.nav_open = nav_open;
            ed.thumb_versions.clear();
            ed.thumbs_dirty = true;
            ed.dirty = true;
            input_el("ed-title").set_value(&ed.nb.title);
        }
        if let Some(ed2) = &mut self.ed2 {
            ed2.nav_open = false;
            ed2.dirty = true;
        }
        show("sel-menu", false);
        show("text-editor", false);
        self.layout_panes();
        self.refresh_toolbar();
        self.update_pill();
        self.render_tabs();
        if nav_open {
            self.render_nav();
        }
    }

    /// Split-Ansicht öffnen (Notizbuch wählen) bzw. schließen.
    pub fn toggle_split(&mut self) {
        self.close_popovers();
        if self.ed2.is_some() {
            self.close_split();
            return;
        }
        let Some(ed) = &self.ed else { return };
        if window_w() < 700.0 {
            self.toast("Für die Split-Ansicht ist das Fenster zu schmal");
            return;
        }
        let cur = ed.nb.id.clone();
        let mut list: Vec<&NoteMeta> = self.library.iter().filter(|m| m.is_notebook() && m.id != cur).collect();
        list.sort_by(|a, b| b.modified.partial_cmp(&a.modified).unwrap_or(std::cmp::Ordering::Equal));
        if list.is_empty() {
            self.toast("Lege zuerst ein weiteres Notizbuch an");
            return;
        }
        let items: String = list
            .iter()
            .map(|m| {
                format!(
                    "<button class=\"pick-item\" data-act=\"pick\" data-arg=\"{}\"><span class=\"mini\" style=\"{}\"></span><span>{}{}<small>{} {}{}</small></span></button>",
                    esc(&m.id),
                    super::library::cover_css(m.cover),
                    esc(&m.title),
                    if m.locked() { " 🔒" } else { "" },
                    m.pages,
                    if m.pages == 1 { "Seite" } else { "Seiten" },
                    if m.folder.is_empty() { String::new() } else { format!(" · {}", esc(&m.folder)) }
                )
            })
            .collect();
        self.open_pick("i-split", "Split-Ansicht", "Welches Notizbuch soll <b>rechts daneben</b> geöffnet werden? Zum Arbeiten einfach in die jeweilige Hälfte tippen.", &items, false, Pending::Split);
    }

    /// Gewähltes Notizbuch rechts daneben öffnen.
    pub fn open_split(&mut self, id: String) {
        let locked = self.library.iter().find(|m| m.id == id).is_some_and(|m| m.locked()) && !self.unlocked.contains(&id);
        if locked {
            self.toast("Dieses Notizbuch ist gesperrt – zuerst normal öffnen und entsperren");
            return;
        }
        self.with_stored(id, |a, nb| {
            if a.ed.is_none() {
                return;
            }
            // freie Zeichenflächen für den zweiten Bereich
            let (view, live) = if a.ed.as_ref().is_some_and(|e| e.canvas.id() == "view") { ("view2", "live2") } else { ("view", "live") };
            let mut ed2 = Editor::new_in(nb, view, live);
            ed2.nav_open = false;
            a.ed2 = Some(ed2);
            a.active_left = true;
            a.layout_panes();
            if let Some(ed2) = &mut a.ed2 {
                ed2.fit_page(0);
            }
            if let Some(ed) = &mut a.ed {
                let i = ed.cur_page;
                ed.fit_page(i);
            }
            a.layout_panes();
            a.toast("Split-Ansicht: in eine Hälfte tippen, um dort zu schreiben");
        });
    }

    pub fn close_split(&mut self) {
        if self.ed2.take().is_none() {
            return;
        }
        self.active_left = true;
        self.layout_panes();
        if let Some(ed) = &mut self.ed {
            let i = ed.cur_page;
            ed.fit_page(i);
        }
        self.request_frame();
    }

    /// Seitenleiste ein-/ausblenden (angedockt neben der Seite, auf schmalen Bildschirmen darüber).
    pub fn set_nav_open(&mut self, open: bool) {
        let Some(ed) = &mut self.ed else { return };
        ed.nav_open = open;
        if !open {
            ed.nav_sel = None;
        }
        set_class(&el("navigator"), "open", open);
        set_class(&el("btn-nav"), "active", open);
        if window_w() >= DOCK_MIN_W && self.nav_pinned != open {
            self.nav_pinned = open;
            self.save_settings();
        }
        self.layout_panes();
        if open {
            self.render_nav();
        }
    }
}
