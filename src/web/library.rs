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
        let searching = !q.is_empty();
        let folder = self.folder.clone();
        let mut list: Vec<&NoteMeta> = self
            .library
            .iter()
            .filter(|m| m.is_notebook())
            .filter(|m| !self.lib_fav || m.favorite)
            .filter(|m| if searching { m.title.to_lowercase().contains(&q) } else { m.folder == folder.clone().unwrap_or_default() })
            .collect();
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
            let in_folder = if searching && !m.folder.is_empty() {
                format!("<span class=\"in-folder\"><svg><use href=\"#i-folder\"/></svg>{}</span>", esc(&m.folder))
            } else {
                String::new()
            };
            html.push_str(&format!(
                "<div class=\"card\" data-act=\"card\" data-arg=\"{id}\">\
                   <div class=\"cover\" style=\"{css};aspect-ratio:{ratio}\">\
                     <div class=\"cover-band\"></div>\
                     <div class=\"cover-label\"><b>{title}</b><small>{pages} {pw}</small></div>\
                     {star}{lock}\
                     <button class=\"card-more\" data-act=\"card-menu\" data-arg=\"{id}\" title=\"Optionen\"><svg><use href=\"#i-more\"/></svg></button>\
                   </div>\
                   <div class=\"card-meta\"><b>{title}</b><small>{date}</small>{in_folder}</div>\
                 </div>",
                id = esc(&m.id),
                css = cover_css(m.cover),
                ratio = ratio,
                title = esc(&m.title),
                pages = m.pages,
                pw = if m.pages == 1 { "Seite" } else { "Seiten" },
                star = if m.favorite { "<svg class=\"cover-star\"><use href=\"#i-star\"/></svg>" } else { "" },
                lock = if m.locked() { "<span class=\"cover-lock\"><svg><use href=\"#i-lock\"/></svg></span>" } else { "" },
                date = date_label(m.modified),
                in_folder = in_folder,
            ));
        }
        // Ordner (nur auf oberster Ebene und ohne Suche)
        let mut fhtml = String::new();
        if folder.is_none() && !searching {
            for f in &self.folders {
                let n = self.library.iter().filter(|m| m.is_notebook() && &m.folder == f).count();
                fhtml.push_str(&format!(
                    "<div class=\"folder-card\" data-act=\"folder-open\" data-arg=\"{f}\"><div class=\"folder-ico\"><svg><use href=\"#i-folder\"/></svg></div><div><b>{f}</b><small>{n} {nw}</small></div><button class=\"folder-more\" data-act=\"folder-menu\" data-arg=\"{f}\" title=\"Ordner-Optionen\"><svg><use href=\"#i-more\"/></svg></button></div>",
                    f = esc(f),
                    n = n,
                    nw = if n == 1 { "Notizbuch" } else { "Notizbücher" }
                ));
            }
        }
        let trash_n = self.trash_count();
        if folder.is_none() && !searching && trash_n > 0 && !self.show_trash {
            fhtml.push_str(&format!(
                "<div class=\"folder-card trash\" data-act=\"trash-open\"><div class=\"folder-ico\"><svg><use href=\"#i-trash\"/></svg></div><div><b>Papierkorb</b><small>{} {}</small></div></div>",
                trash_n,
                if trash_n == 1 { "Notizbuch" } else { "Notizbücher" }
            ));
        }
        if self.show_trash {
            fhtml.clear();
        }
        el("lib-folders").set_inner_html(&fhtml);
        self.render_decks(folder.is_none() && !searching && !self.lib_fav && !self.show_trash);
        show("lib-crumb", (folder.is_some() || self.show_trash) && !searching);
        set_text("lib-crumb-name", if self.show_trash { "Papierkorb" } else { folder.as_deref().unwrap_or("") });
        show("lib-trash-empty", self.show_trash && trash_n > 0);
        if self.show_trash {
            show("lib-decks", false);
            show("lib-empty", false);
            show("lib-grid", true);
            let t = self.render_trash();
            el("lib-grid").set_inner_html(&t);
            return;
        }
        el("lib-grid").set_inner_html(&html);
        let total = self.library.iter().filter(|m| m.is_notebook()).count();
        set_text("lib-count", &format!("{} {}", total, if total == 1 { "Notizbuch" } else { "Notizbücher" }));
        let empty = total == 0 && self.folders.is_empty();
        show("lib-empty", empty);
        show("lib-grid", !empty);
        if !empty && list.is_empty() {
            let msg = if searching {
                "Keine Treffer"
            } else if folder.is_some() {
                "Dieser Ordner ist leer – lege hier ein Notizbuch an oder verschiebe eines über ⋯ → „In Ordner verschieben“."
            } else {
                ""
            };
            el("lib-grid").set_inner_html(&format!("<p class=\"muted lib-none\">{}</p>", msg));
        }
    }

    /// Lernkarten-Bereich auf dem Startbildschirm.
    fn render_decks(&self, visible: bool) {
        show("lib-decks", visible);
        if !visible {
            return;
        }
        let mut decks: Vec<&NoteMeta> = self.library.iter().filter(|m| m.visible() && (m.is_deck || m.cards > 0)).collect();
        decks.sort_by(|a, b| b.modified.partial_cmp(&a.modified).unwrap_or(std::cmp::Ordering::Equal));
        let (total, learned): (u32, u32) = decks.iter().fold((0, 0), |(t, l), m| (t + m.cards, l + m.learned));
        let now = wall_clock();
        let due_total: usize = if self.settings.due_mode { decks.iter().map(|m| m.due_count(now)).sum() } else { 0 };
        let summary = if decks.is_empty() {
            "Lerne mit Karteikarten – Frage vorne, Antwort hinten".to_string()
        } else {
            format!("{} {} · {} gelernt{}", total, if total == 1 { "Karte" } else { "Karten" }, learned, if due_total > 0 { format!(" · <b class=\"due\">{} heute fällig</b>", due_total) } else { String::new() })
        };
        let mut html = format!(
            "<div class=\"decks-head\"><h3><svg><use href=\"#i-cards\"/></svg>Lernkarten</h3><span class=\"muted\">{}</span><span class=\"decks-streak\">{}</span><button class=\"btn small ghost\" data-act=\"stats\"><svg><use href=\"#i-chart\"/></svg>Statistik</button></div><div class=\"decks-row\">\
             <button class=\"deck-new\" data-act=\"deck-new\"><span class=\"deck-plus\"><svg><use href=\"#i-plus\"/></svg></span><b>Neues Lernkarten-Set</b><small>Frage oben, Antwort unten – mit dem S-Pen</small></button>",
            summary,
            self.streak_line()
        );
        for m in decks {
            let due = if self.settings.due_mode { m.due_count(now) } else { 0 };
            let pct = m.progress.min(100) as f64;
            html.push_str(&format!(
                "<div class=\"deck\" data-act=\"card\" data-arg=\"{id}\" title=\"Bearbeiten\">\
                   <div class=\"deck-stack\" style=\"{css}\"><span></span><span></span><div class=\"deck-face\"><b>{title}</b><small>{n} {nw}</small>{due}{lock}</div><button class=\"card-more\" data-act=\"card-menu\" data-arg=\"{id}\" title=\"Optionen\"><svg><use href=\"#i-more\"/></svg></button></div>\
                   <div class=\"deck-prog\"><span style=\"width:{pct:.0}%\"></span></div>\
                   <div class=\"deck-foot\"><small>{l} von {n} gelernt</small><button class=\"btn small primary\" data-act=\"deck-study\" data-arg=\"{id}\"><svg><use href=\"#i-cards\"/></svg>Lernen</button></div>\
                 </div>",
                id = esc(&m.id),
                css = cover_css(m.cover),
                title = esc(&m.title),
                n = m.cards,
                nw = if m.cards == 1 { "Karte" } else { "Karten" },
                l = m.learned,
                pct = pct,
                due = if due > 0 { format!("<span class=\"due-badge\">{} fällig</span>", due) } else { String::new() },
                lock = if m.locked() { "<span class=\"cover-lock\"><svg><use href=\"#i-lock\"/></svg></span>" } else { "" }
            ));
        }
        html.push_str("</div>");
        el("lib-decks").set_inner_html(&html);
    }

    /// Lernkarten-Set vom Startbildschirm aus öffnen und gleich lernen.
    pub fn study_deck(&mut self, id: String) {
        if self.needs_unlock(&id, super::lock::Then::Study) {
            return;
        }
        self.with_stored(id, |a, nb| {
            a.enter_editor(nb);
            a.open_study();
            a.study_from_lib = true;
            let now = wall_clock();
            let (has, due) = a.ed.as_ref().map(|e| (!e.nb.study_cards().is_empty(), e.nb.study_cards().iter().any(|c| c.is_due(now)))).unwrap_or((false, false));
            if has {
                a.study_start(if a.settings.due_mode && due { "due" } else { "all" });
            }
        });
    }

    /// Neues Notizbuch speichern und (optional) öffnen.
    pub fn add_notebook(&mut self, mut nb: Notebook, open: bool) {
        nb.sync_template_cards();
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
        self.unlocked.clear();
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
        if matches!(act, "card-dup" | "card-template" | "card-pdf" | "card-znote") && self.needs_unlock(&id, super::lock::Then::Card(act.to_string())) {
            return;
        }
        match act {
            "card-open" => self.open_guarded(id),
            "card-lock" => self.card_lock(id),
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
            "card-folder" => self.pick_folder(id),
            "card-template" => {
                let cur = self.library.iter().find(|m| m.id == id).map(|m| m.title.clone()).unwrap_or_default();
                let visual = self.cover_visual(&id, "accent", "i-template");
                self.open_dialog(&visual, "Als Vorlage speichern", "Neue Notizbücher können dann mit diesen Seiten, Vorlagen und Lernkarten starten. Name der Vorlage:", "Speichern", false, Some(&cur), Pending::SaveTemplate(Some(id)));
            }
            "card-pdf" => self.with_stored(id, |a, nb| a.export_notebook_pdf(&nb, None)),
            "card-znote" => self.with_stored(id, |a, nb| a.export_znote(Some(&nb))),
            "card-delete" => {
                let meta = self.library.iter().find(|m| m.id == id).cloned();
                let (title, pages) = meta.map(|m| (m.title, m.pages)).unwrap_or_default();
                let visual = self.cover_visual(&id, "danger", "i-trash");
                let text = format!(
                    "<b>„{}“</b> mit {} {} wird in den Papierkorb verschoben. Dort kannst du es 30 Tage lang wiederherstellen.",
                    esc(&title),
                    pages,
                    if pages == 1 { "Seite" } else { "Seiten" }
                );
                self.open_dialog(&visual, "Notizbuch löschen?", &text, "In den Papierkorb", true, None, Pending::DeleteNotebook(id));
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
        let _ = inp.set_attribute("type", "text");
        let _ = inp.set_attribute("inputmode", "text");
        inp.set_placeholder("");
        show("dlg-bio", false);
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
        if let Some(p) = &pending {
            if self.lock_dialog(p, &value) {
                return;
            }
        }
        match pending {
            Some(Pending::DeleteForever(id)) => {
                self.delete_stored(&id);
                self.toast("Endgültig gelöscht");
            }
            Some(Pending::EmptyTrash) => self.empty_trash(),
            Some(Pending::NewDeck) => {
                let t: String = value.trim().chars().take(80).collect();
                let title = if t.is_empty() { "Lernkarten".to_string() } else { t };
                let cover = (js_sys::Math::random() * COVERS.len() as f64) as u8 % COVERS.len() as u8;
                let tpl = crate::engine::Template::new(crate::engine::TemplateKind::Flashcard, 1);
                let mut nb = Notebook::new(super::storage::new_id(), &title, cover, crate::engine::Format { w: 420.0, h: 595.0 }, tpl, crate::engine::Color::WHITE, wall_clock());
                // Lernkarten-Sets stehen nur im Lernkarten-Bereich, nicht bei den Notizbüchern
                nb.is_deck = true;
                self.add_notebook(nb, true);
                self.toast("Oben die Frage, unten die Antwort schreiben – neue Karte über „Seite hinzufügen“ (Blatt mit +)");
            }
            Some(Pending::DeleteNotebook(id)) => self.trash_notebook(id),
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
            Some(Pending::NewFolder) => {
                let name: String = value.trim().chars().take(40).collect();
                if name.is_empty() {
                    return;
                }
                if !self.folders.contains(&name) {
                    self.folders.push(name.clone());
                    self.folders.sort_by_key(|f| f.to_lowercase());
                    self.persist_folders();
                }
                self.folder = Some(name);
                self.render_library();
            }
            Some(Pending::RenameFolder(old)) => {
                let name: String = value.trim().chars().take(40).collect();
                if name.is_empty() || name == old {
                    return;
                }
                for f in self.folders.iter_mut() {
                    if *f == old {
                        *f = name.clone();
                    }
                }
                self.folders.dedup();
                self.persist_folders();
                let ids: Vec<String> = self.library.iter().filter(|m| m.folder == old).map(|m| m.id.clone()).collect();
                for id in ids {
                    let n = name.clone();
                    self.modify_stored(id, move |nb| nb.folder = n);
                }
                if self.folder.as_deref() == Some(&old) {
                    self.folder = Some(name);
                }
                self.render_library();
            }
            Some(Pending::DeleteFolder(name)) => {
                self.folders.retain(|f| *f != name);
                self.persist_folders();
                let ids: Vec<String> = self.library.iter().filter(|m| m.folder == name).map(|m| m.id.clone()).collect();
                for id in ids {
                    self.modify_stored(id, |nb| nb.folder.clear());
                }
                if self.folder.as_deref() == Some(&name) {
                    self.folder = None;
                }
                self.render_library();
                self.toast("Ordner aufgelöst – die Notizbücher liegen jetzt in der Übersicht");
            }
            Some(Pending::PageTitle(i)) => {
                let t: String = value.trim().chars().take(80).collect();
                if let Some(ed) = &mut self.ed {
                    ed.set_page_meta(i, |m| m.title = t);
                }
                self.after_pages_change();
            }
            Some(Pending::SaveTemplate(id)) => {
                let name: String = value.trim().chars().take(80).collect();
                let name = if name.is_empty() { "Vorlage".to_string() } else { name };
                let make = move |a: &mut App, nb: Notebook| {
                    let mut t = nb.copy_as(super::storage::new_id(), &name, wall_clock());
                    t.is_template = true;
                    t.folder.clear();
                    a.add_notebook(t, false);
                    a.toast("Als Notizbuchvorlage gespeichert – zu finden unter „Neues Notizbuch“");
                };
                match id {
                    Some(id) => self.with_stored(id, make),
                    None => {
                        if let Some(nb) = self.ed.as_ref().map(|e| e.nb.clone()) {
                            make(self, nb);
                        }
                    }
                }
            }
            Some(Pending::DeleteTemplate(id)) => {
                self.delete_stored(&id);
                if self.new_from.as_deref() == Some(&id) {
                    self.new_from = None;
                }
                self.toast("Vorlage gelöscht");
            }
            Some(Pending::MoveToFolder(_)) | Some(Pending::PageTo(_)) | Some(Pending::Link(..)) => {}
            Some(Pending::Unlock(..)) | Some(Pending::SetPin(_)) | Some(Pending::ConfirmPin(..)) | Some(Pending::RemovePin(_)) => {}
            None => {}
        }
    }

    // -----------------------------------------------------------------------
    // Ordner
    // -----------------------------------------------------------------------

    pub fn folder_action(&mut self, act: &str, arg: &str, src: &web_sys::Element) {
        match act {
            "folder-new" => {
                let visual = "<div class=\"dlg-icon accent\"><svg><use href=\"#i-folder-plus\"/></svg></div>";
                self.open_dialog(visual, "Neuer Ordner", "Wie soll der Ordner heißen?", "Erstellen", false, Some(""), Pending::NewFolder);
            }
            "folder-open" => {
                self.folder = Some(arg.to_string());
                self.render_library();
            }
            "folder-up" => {
                self.folder = None;
                self.show_trash = false;
                self.render_library();
            }
            "folder-menu" => {
                self.ctx_target = Some(arg.to_string());
                self.open_popover("pop-folder", src);
            }
            "folder-rename" => {
                let Some(name) = self.ctx_target.clone() else { return };
                let visual = "<div class=\"dlg-icon accent\"><svg><use href=\"#i-folder\"/></svg></div>";
                self.open_dialog(visual, "Ordner umbenennen", "Neuer Name für den Ordner:", "Speichern", false, Some(&name), Pending::RenameFolder(name.clone()));
            }
            "folder-delete" => {
                let Some(name) = self.ctx_target.clone() else { return };
                let n = self.library.iter().filter(|m| m.is_notebook() && m.folder == name).count();
                let visual = "<div class=\"dlg-icon danger\"><svg><use href=\"#i-folder\"/></svg></div>";
                let text = format!("Der Ordner <b>„{}“</b> wird aufgelöst. Die {} {} darin bleiben erhalten und liegen danach in der Übersicht.", esc(&name), n, if n == 1 { "Notizbuch" } else { "Notizbücher" });
                self.open_dialog(visual, "Ordner auflösen?", &text, "Auflösen", true, None, Pending::DeleteFolder(name));
            }
            _ => {}
        }
    }

    /// Auswahldialog mit Liste (Ordner bzw. Notizbücher).
    pub fn open_pick(&mut self, icon: &str, title: &str, text: &str, items: &str, with_mode: bool, pending: Pending) {
        self.close_popovers();
        el("pick-icon").set_inner_html(&format!("<svg><use href=\"#{}\"/></svg>", icon));
        set_text("pick-title", title);
        el("pick-text").set_inner_html(text);
        el("pick-list").set_inner_html(items);
        show("pick-mode", with_mode);
        self.pick_copy = true;
        Self::mark_pick_mode(true);
        self.pending = Some(pending);
        self.open_modal("modal-pick");
    }

    pub fn mark_pick_mode(copy: bool) {
        if let Ok(list) = dom().query_selector_all("#pick-mode button") {
            for i in 0..list.length() {
                let b: web_sys::Element = list.get(i).unwrap().unchecked_into();
                let on = (b.get_attribute("data-arg").as_deref() == Some("copy")) == copy;
                set_class(&b, "active", on);
            }
        }
    }

    fn pick_folder(&mut self, id: String) {
        let cur = self.library.iter().find(|m| m.id == id).map(|m| m.folder.clone()).unwrap_or_default();
        let mut items = format!(
            "<button class=\"pick-item{}\" data-act=\"pick\" data-arg=\"\"><span class=\"fico root\"><svg><use href=\"#i-pages\"/></svg></span><span>Übersicht<small>kein Ordner</small></span></button>",
            if cur.is_empty() { " current" } else { "" }
        );
        for f in &self.folders {
            items.push_str(&format!(
                "<button class=\"pick-item{}\" data-act=\"pick\" data-arg=\"{f}\"><span class=\"fico\"><svg><use href=\"#i-folder\"/></svg></span><span>{f}</span></button>",
                if *f == cur { " current" } else { "" },
                f = esc(f)
            ));
        }
        if self.folders.is_empty() {
            items.push_str("<p class=\"muted\" style=\"text-align:center;font-size:13px\">Noch keine Ordner – lege oben in der Bibliothek mit dem Ordner-Symbol einen an.</p>");
        }
        let title = self.library.iter().find(|m| m.id == id).map(|m| m.title.clone()).unwrap_or_default();
        self.open_pick("i-folder", "In Ordner verschieben", &format!("Wohin soll <b>„{}“</b>?", esc(&title)), &items, false, Pending::MoveToFolder(id));
    }

    /// Eintrag im Auswahldialog gewählt.
    pub fn pick(&mut self, value: &str) {
        let pending = self.pending.take();
        self.close_modals();
        match pending {
            Some(Pending::MoveToFolder(id)) => {
                let f = value.to_string();
                let label = if f.is_empty() { "die Übersicht".to_string() } else { format!("„{}“", f) };
                self.modify_stored(id, move |nb| nb.folder = f);
                self.toast(&format!("Verschoben nach {}", label));
            }
            Some(Pending::PageTo(i)) => self.page_to(i, value.to_string()),
            Some(Pending::Link(page, x0, y0, x1, y1)) => self.create_link(page, crate::engine::Rect::new(x0, y0, x1, y1), value),
            _ => {}
        }
    }

    /// Seite `i` des geöffneten Notizbuchs in ein anderes kopieren oder verschieben.
    fn page_to(&mut self, i: usize, target: String) {
        let copy = self.pick_copy;
        let Some(page) = self.ed.as_ref().and_then(|e| e.nb.pages.get(i).cloned()) else { return };
        let title = self.library.iter().find(|m| m.id == target).map(|m| m.title.clone()).unwrap_or_default();
        self.with_stored(target, move |a, mut nb| {
            let p = nb.adopt_page(&page);
            nb.pages.push(p);
            nb.modified = wall_clock();
            a.add_notebook(nb, false);
            if !copy {
                let done = a.ed.as_mut().map(|ed| ed.delete_page(i)).unwrap_or(false);
                if !done {
                    a.toast(&format!("Seite nach „{}“ kopiert (die letzte Seite bleibt hier)", title));
                    return;
                }
                a.after_pages_change();
            }
            a.toast(&format!("Seite nach „{}“ {}", title, if copy { "kopiert" } else { "verschoben" }));
        });
    }

    pub fn open_page_to(&mut self) {
        let Some(ed) = &self.ed else { return };
        let (i, cur_id) = (ed.cur_page, ed.nb.id.clone());
        let mut list: Vec<&NoteMeta> = self.library.iter().filter(|m| m.is_notebook() && m.id != cur_id).collect();
        list.sort_by(|a, b| b.modified.partial_cmp(&a.modified).unwrap_or(std::cmp::Ordering::Equal));
        if list.is_empty() {
            self.toast("Lege zuerst ein weiteres Notizbuch an");
            return;
        }
        let items: String = list
            .iter()
            .map(|m| {
                format!(
                    "<button class=\"pick-item\" data-act=\"pick\" data-arg=\"{}\"><span class=\"mini\" style=\"{}\"></span><span>{}<small>{} {}{}</small></span></button>",
                    esc(&m.id),
                    cover_css(m.cover),
                    esc(&m.title),
                    m.pages,
                    if m.pages == 1 { "Seite" } else { "Seiten" },
                    if m.folder.is_empty() { String::new() } else { format!(" · {}", esc(&m.folder)) }
                )
            })
            .collect();
        self.open_pick("i-move-page", "Seite in anderes Notizbuch", &format!("<b>Seite {}</b> wird ans Ende des gewählten Notizbuchs angehängt.", i + 1), &items, true, Pending::PageTo(i));
    }
}
