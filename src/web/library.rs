//! Bibliothek: Notizbuch-Übersicht mit Covern, Suche, Sortierung, Favoriten.

use super::*;
use crate::engine::format::NoteMeta;
use crate::engine::{Notebook, COVERS};

fn date_label(ms: f64) -> String {
    let d = js_sys::Date::new(&JsValue::from_f64(ms));
    let today = js_sys::Date::new_0();
    let same = |a: &js_sys::Date, b: &js_sys::Date| a.get_full_year() == b.get_full_year() && a.get_month() == b.get_month() && a.get_date() == b.get_date();
    let yesterday = js_sys::Date::new(&JsValue::from_f64(today.get_time() - 86_400_000.0));
    if same(&d, &today) {
        format!("Heute, {:02}:{:02}", d.get_hours(), d.get_minutes())
    } else if same(&d, &yesterday) {
        format!("Gestern, {:02}:{:02}", d.get_hours(), d.get_minutes())
    } else {
        format!("{:02}.{:02}.{}", d.get_date(), d.get_month() + 1, d.get_full_year())
    }
}

pub fn cover_css(i: u8) -> String {
    let (_, a, b) = COVERS[i as usize % COVERS.len()];
    format!("--c1:{};--c2:{}", a, b)
}

impl App {
    pub fn render_library(&mut self) {
        self.refresh_lib_controls();
        let q = self.lib_query.clone();
        let mut list: Vec<&NoteMeta> = self.library.iter().filter(|m| (!self.lib_fav || m.favorite) && (q.is_empty() || m.title.to_lowercase().contains(&q))).collect();
        match self.lib_sort.as_str() {
            "title" => list.sort_by_key(|m| m.title.to_lowercase()),
            "created" => list.sort_by(|a, b| b.created.partial_cmp(&a.created).unwrap_or(std::cmp::Ordering::Equal)),
            _ => list.sort_by(|a, b| b.modified.partial_cmp(&a.modified).unwrap_or(std::cmp::Ordering::Equal)),
        }
        // Favoriten zuerst
        list.sort_by_key(|m| !m.favorite);
        let mut html = String::new();
        for m in &list {
            let ratio = format!("{} / {}", m.w, m.h);
            html.push_str(&format!(
                "<div class=\"card\" data-act=\"card\" data-arg=\"{id}\">\
                   <div class=\"cover\" style=\"{css};aspect-ratio:{ratio}\">\
                     <div class=\"cover-band\"></div>\
                     <div class=\"cover-label\"><b>{title}</b><small>{pages} {pw}</small></div>\
                     {star}\
                     <button class=\"card-more\" data-act=\"card-menu\" data-arg=\"{id}\" title=\"Optionen\"><svg><use href=\"#i-more\"/></svg></button>\
                   </div>\
                   <div class=\"card-meta\"><b>{title}</b><small>{date}</small></div>\
                 </div>",
                id = esc(&m.id),
                css = cover_css(m.cover),
                ratio = ratio,
                title = esc(&m.title),
                pages = m.pages,
                pw = if m.pages == 1 { "Seite" } else { "Seiten" },
                star = if m.favorite { "<svg class=\"cover-star\"><use href=\"#i-star\"/></svg>" } else { "" },
                date = date_label(m.modified),
            ));
        }
        el("lib-grid").set_inner_html(&html);
        let total = self.library.len();
        set_text("lib-count", &format!("{} {}", total, if total == 1 { "Notizbuch" } else { "Notizbücher" }));
        show("lib-empty", total == 0);
        show("lib-grid", total > 0);
        if total > 0 && list.is_empty() {
            el("lib-grid").set_inner_html("<p class=\"muted lib-none\">Keine Treffer</p>");
        }
    }

    /// Neues Notizbuch speichern und (optional) öffnen.
    pub fn add_notebook(&mut self, nb: Notebook, open: bool) {
        self.library.retain(|m| m.id != nb.id);
        self.library.push(NoteMeta::of(&nb));
        self.persist_notebook(&nb);
        self.persist_library();
        if open {
            self.enter_editor(nb);
        } else {
            self.render_library();
        }
    }

    pub fn enter_editor(&mut self, nb: Notebook) {
        self.close_popovers();
        input_el("ed-title").set_value(&nb.title);
        self.screen = Screen::Editor;
        show("library", false);
        show("editor", true);
        self.ed = Some(Editor::new(nb));
        if let Some(ed) = &mut self.ed {
            ed.resize();
            let i = 0;
            ed.fit_page(i);
            ed.nav_open = false;
        }
        set_class(&el("navigator"), "open", false);
        set_class(&el("btn-nav"), "active", false);
        self.hold_eraser = false;
        self.refresh_toolbar();
        self.update_pill();
        self.position_overlays();
        self.request_frame();
    }

    pub fn close_editor(&mut self) {
        if self.ed.as_ref().map(|e| e.text_edit.is_some()).unwrap_or(false) {
            self.commit_text();
        }
        self.save_now();
        self.ed = None;
        self.images.clear();
        self.screen = Screen::Library;
        self.close_popovers();
        show("editor", false);
        show("library", true);
        show("sel-menu", false);
        show("text-editor", false);
        self.render_library();
    }

    pub fn card_action(&mut self, act: &str, n: usize) {
        self.close_popovers();
        let Some(id) = self.ctx_target.clone() else { return };
        match act {
            "card-open" => self.open_notebook(id),
            "card-rename" => {
                let cur = self.library.iter().find(|m| m.id == id).map(|m| m.title.clone()).unwrap_or_default();
                let visual = self.cover_visual(&id, "accent", "i-text");
                self.open_dialog(&visual, "Notizbuch umbenennen", "Gib dem Notizbuch einen neuen Namen.", "Speichern", false, Some(&cur), Pending::RenameNotebook(id));
            }
            "card-fav" => self.modify_stored(id, |nb| nb.favorite = !nb.favorite),
            "card-cover" => {
                let html: String = COVERS
                    .iter()
                    .enumerate()
                    .map(|(i, (name, a, b))| format!("<button class=\"cover-dot big\" data-act=\"cover-pick\" data-arg=\"{}\" title=\"{}\" style=\"background:linear-gradient(135deg,{},{})\"></button>", i, name, a, b))
                    .collect();
                el("cover-pick").set_inner_html(&html);
                self.open_modal("modal-cover");
            }
            "cover-pick" => {
                self.close_modals();
                self.modify_stored(id, move |nb| nb.cover = n as u8);
            }
            "card-dup" => self.duplicate_stored(id),
            "card-pdf" => self.with_stored(id, |a, nb| a.export_notebook_pdf(&nb, None)),
            "card-znote" => self.with_stored(id, |a, nb| a.export_znote(Some(&nb))),
            "card-delete" => {
                let meta = self.library.iter().find(|m| m.id == id).cloned();
                let (title, pages) = meta.map(|m| (m.title, m.pages)).unwrap_or_default();
                let visual = self.cover_visual(&id, "danger", "i-trash");
                let text = format!(
                    "<b>„{}“</b> mit {} {} wird endgültig gelöscht. Das kann nicht rückgängig gemacht werden.",
                    esc(&title),
                    pages,
                    if pages == 1 { "Seite" } else { "Seiten" }
                );
                self.open_dialog(&visual, "Notizbuch löschen?", &text, "Löschen", true, None, Pending::DeleteNotebook(id));
            }
            _ => {}
        }
    }

    /// Mini-Cover mit Symbol-Plakette für Dialoge.
    pub fn cover_visual(&self, id: &str, kind: &str, icon: &str) -> String {
        let m = self.library.iter().find(|m| m.id == id);
        let (title, cover) = m.map(|m| (m.title.clone(), m.cover)).unwrap_or_default();
        format!(
            "<div class=\"dlg-cover-wrap\"><div class=\"dlg-cover\" style=\"{}\"><span>{}</span></div><div class=\"badge {}\"><svg><use href=\"#{}\"/></svg></div></div>",
            cover_css(cover),
            esc(&title),
            kind,
            icon
        )
    }

    /// Eigener Dialog statt Browser-Fenster. `input` = Textfeld mit Vorgabewert.
    #[allow(clippy::too_many_arguments)]
    pub fn open_dialog(&mut self, visual: &str, title: &str, text_html: &str, ok: &str, danger: bool, input: Option<&str>, pending: Pending) {
        self.close_popovers();
        el("dlg-visual").set_inner_html(visual);
        set_text("dlg-title", title);
        el("dlg-text").set_inner_html(text_html);
        let okb = el("dlg-ok");
        okb.set_text_content(Some(ok));
        set_class(&okb, "danger", danger);
        set_class(&okb, "primary", !danger);
        let inp = input_el("dlg-input");
        set_class(&inp, "hidden", input.is_none());
        inp.set_value(input.unwrap_or(""));
        self.pending = Some(pending);
        self.open_modal("modal-confirm");
        if input.is_some() {
            let _ = inp.focus();
            inp.select();
        }
    }

    /// Bestätigt den Dialog.
    pub fn dialog_ok(&mut self) {
        let value = input_el("dlg-input").value();
        let pending = self.pending.take();
        self.close_modals();
        match pending {
            Some(Pending::DeleteNotebook(id)) => {
                self.delete_stored(&id);
                self.toast("Notizbuch gelöscht");
            }
            Some(Pending::RenameNotebook(id)) => {
                let t: String = value.trim().chars().take(80).collect();
                if !t.is_empty() {
                    self.modify_stored(id, move |nb| nb.title = t);
                }
            }
            Some(Pending::DeletePage(i)) => {
                let done = self.ed.as_mut().map(|ed| ed.delete_page(i)).unwrap_or(false);
                if done {
                    self.toast("Seite gelöscht – Rückgängig ist möglich");
                } else {
                    self.toast("Die letzte Seite kann nicht gelöscht werden");
                }
                self.after_pages_change();
            }
            None => {}
        }
    }
}
