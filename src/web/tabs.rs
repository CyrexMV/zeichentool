//! Tabs (mehrere Notizbücher schnell wechseln) und Präsentationsmodus.

use super::*;

/// Ein geöffneter Tab: Notizbuch-ID + zuletzt gezeigte Ansicht (Zoom, x, y).
#[derive(Clone, Debug)]
pub struct Tab {
    pub id: String,
    pub view: Option<(f64, f64, f64)>,
}

const MAX_TABS: usize = 8;

impl App {
    /// Beim Öffnen eines Notizbuchs: Tab anlegen bzw. nach vorne holen.
    pub fn tab_opened(&mut self, id: &str) {
        if !self.tabs.iter().any(|t| t.id == id) {
            self.tabs.push(Tab { id: id.to_string(), view: None });
            if self.tabs.len() > MAX_TABS {
                self.tabs.remove(0);
            }
            self.save_settings();
        }
        self.render_tabs();
    }

    /// Ansicht des aktuellen Notizbuchs im Tab merken.
    pub fn tab_store_view(&mut self) {
        let Some(ed) = &self.ed else { return };
        let (id, v) = (ed.nb.id.clone(), (ed.view.zoom, ed.view.sx, ed.view.sy));
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.view = Some(v);
        }
    }

    /// Gemerkte Ansicht wiederherstellen (nach dem Öffnen).
    pub fn tab_restore_view(&mut self) {
        let Some(ed) = &mut self.ed else { return };
        let id = ed.nb.id.clone();
        if let Some((z, sx, sy)) = self.tabs.iter().find(|t| t.id == id).and_then(|t| t.view) {
            ed.view.zoom = z;
            ed.view.sx = sx;
            ed.view.sy = sy;
            ed.bmps.clear();
            ed.clamp();
            ed.dirty = true;
        }
    }

    pub fn render_tabs(&mut self) {
        // verschwundene Notizbücher (gelöscht/Papierkorb) entfernen
        let lib = &self.library;
        self.tabs.retain(|t| lib.iter().any(|m| m.id == t.id && m.trashed == 0.0) || self.ed.as_ref().map(|e| e.nb.id == t.id).unwrap_or(false));
        let cur = self.ed.as_ref().map(|e| e.nb.id.clone());
        let show_bar = self.tabs.len() >= 2 && self.screen == Screen::Editor;
        let mut h = String::new();
        for t in &self.tabs {
            let m = self.library.iter().find(|m| m.id == t.id);
            let title = if cur.as_deref() == Some(t.id.as_str()) {
                self.ed.as_ref().map(|e| e.nb.title.clone()).unwrap_or_default()
            } else {
                m.map(|m| m.title.clone()).unwrap_or_else(|| "Notizbuch".into())
            };
            let cover = m.map(|m| m.cover).unwrap_or(0);
            h.push_str(&format!(
                "<div class=\"tab{}\" data-act=\"tab\" data-arg=\"{id}\" title=\"{title}\"><span class=\"tab-dot\" style=\"{css}\"></span><span class=\"tab-title\">{title}</span><button class=\"tab-x\" data-act=\"tab-close\" data-arg=\"{id}\" title=\"Tab schließen\"><svg><use href=\"#i-close\"/></svg></button></div>",
                if cur.as_deref() == Some(t.id.as_str()) { " active" } else { "" },
                id = esc(&t.id),
                title = esc(&title),
                css = super::library::cover_css(cover)
            ));
        }
        h.push_str("<button class=\"tab-add\" data-act=\"back\" title=\"Weiteres Notizbuch öffnen\"><svg><use href=\"#i-plus\"/></svg></button>");
        let bar = el("ed-tabs");
        bar.set_inner_html(&h);
        let was = !bar.class_list().contains("hidden");
        set_class(&bar, "hidden", !show_bar);
        set_class(&el("editor"), "has-tabs", show_bar);
        if was != show_bar {
            self.layout_panes();
        }
    }

    pub fn tab_switch(&mut self, id: String) {
        if self.ed.as_ref().map(|e| e.nb.id == id).unwrap_or(false) {
            return;
        }
        // in der Split-Ansicht schon offen → nur den Bereich wechseln
        if self.ed2.as_ref().is_some_and(|e| e.nb.id == id) {
            self.activate_other();
            return;
        }
        self.tab_store_view();
        if self.ed.as_ref().map(|e| e.text_edit.is_some()).unwrap_or(false) {
            self.commit_text();
        }
        self.save_now();
        if self.present.is_some() {
            self.present_exit();
        }
        self.open_guarded(id);
    }

    pub fn tab_close(&mut self, id: &str) {
        let pos = self.tabs.iter().position(|t| t.id == id);
        self.tabs.retain(|t| t.id != id);
        self.save_settings();
        let is_cur = self.ed.as_ref().map(|e| e.nb.id == id).unwrap_or(false);
        if is_cur {
            let next = pos.and_then(|p| self.tabs.get(p.min(self.tabs.len().saturating_sub(1)))).map(|t| t.id.clone());
            match next {
                Some(n) => {
                    self.save_now();
                    self.open_guarded(n);
                }
                None => self.close_editor(),
            }
        } else {
            self.render_tabs();
        }
    }

    // -----------------------------------------------------------------
    // Präsentationsmodus
    // -----------------------------------------------------------------

    pub fn present_start(&mut self) {
        self.close_popovers();
        let Some(ed) = &mut self.ed else { return };
        if ed.text_edit.is_some() {
            self.commit_text();
        }
        let Some(ed) = &mut self.ed else { return };
        ed.selection = None;
        let i = ed.cur_page;
        let prev_view = (ed.view.zoom, ed.view.sx, ed.view.sy);
        self.present = Some((self.tool, prev_view));
        set_class(&el("editor"), "presenting", true);
        show("sel-menu", false);
        let _ = dom().document_element().map(|d| d.request_fullscreen());
        self.tool = Tool::Laser;
        self.refresh_toolbar();
        if let Some(ed) = &mut self.ed {
            ed.presenting = Some(i);
        }
        self.layout_panes();
        if let Some(ed) = &mut self.ed {
            ed.present_fit(i);
        }
        self.present_update();
        self.toast("Präsentation: wischen oder rechts/links tippen zum Blättern, Stift = Laserpointer");
    }

    pub fn present_exit(&mut self) {
        let Some((tool, (z, sx, sy))) = self.present.take() else { return };
        set_class(&el("editor"), "presenting", false);
        if dom().fullscreen_element().is_some() {
            dom().exit_fullscreen();
        }
        self.tool = tool;
        self.refresh_toolbar();
        let cur = self.ed.as_ref().and_then(|e| e.presenting).unwrap_or(0);
        if let Some(ed) = &mut self.ed {
            ed.presenting = None;
        }
        self.layout_panes();
        if let Some(ed) = &mut self.ed {
            ed.view.zoom = z;
            ed.view.sx = sx;
            ed.view.sy = sy;
            ed.bmps.clear();
            ed.goto_page(cur);
        }
        self.after_view_change();
    }

    /// Blättern im Präsentationsmodus.
    pub fn present_step(&mut self, d: i32) {
        let Some(ed) = &mut self.ed else { return };
        let Some(i) = ed.presenting else { return };
        let n = ed.nb.pages.len() as i32;
        let j = (i as i32 + d).clamp(0, n - 1) as usize;
        if j == i {
            return;
        }
        ed.presenting = Some(j);
        ed.present_fit(j);
        self.present_update();
    }

    fn present_update(&mut self) {
        let Some(ed) = &self.ed else { return };
        let Some(i) = ed.presenting else { return };
        set_text("present-count", &format!("{} / {}", i + 1, ed.nb.pages.len()));
        self.request_frame();
    }
}

impl super::editor::Editor {
    /// Ganze Seite mittig zeigen (Präsentation).
    pub fn present_fit(&mut self, i: usize) {
        let i = i.min(self.nb.pages.len() - 1);
        let fit = self.view.fit;
        let (pw, ph) = (self.page_w(i), self.page_h(i));
        let z = ((self.vh - 40.0) / (ph * fit)).min((self.vw - 40.0) / (pw * fit)).max(0.1);
        self.view.zoom = z;
        let s = self.view.s();
        self.view.sy = self.page_top(i) - ((self.vh / s) - ph) * 0.5;
        self.bmps.clear();
        self.clamp();
        self.view.sy = self.page_top(i) - ((self.vh / s) - ph) * 0.5;
        self.view.sx = self.page_left(i) - ((self.vw / s) - pw) * 0.5;
        self.cur_page = i;
        self.dirty = true;
    }
}
