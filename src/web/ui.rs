//! Oberfläche: zentrale Aktionsverteilung (`data-act`), Werkzeugleiste mit Favoriten,
//! Werkzeug-Optionen, Seitenübersicht, Dialoge und Einstellungen.

use super::*;
use crate::engine::{Format, Notebook, COVERS};
use std::cell::RefCell;
use web_sys::{HtmlSelectElement, PointerEvent};

thread_local! {
    static PILL: RefCell<String> = const { RefCell::new(String::new()) };
}

type FavSpec<'a> = (Option<&'a [[u8; 4]; 3]>, usize, Option<&'a [f32; 3]>, usize, f32);

const POPS: [&str; 11] = ["pop-import", "pop-tool", "pop-more", "pop-card", "pop-ruler", "pop-folder", "pop-trash", "pop-insert", "pop-share", "pop-sets", "pop-align"];
thread_local! {
    /// zuletzt in der Seitenleiste markierte Seite (Seite, Seitenzahl, Notizbuch)
    static NAV_CUR: std::cell::RefCell<Option<(usize, usize, String)>> = const { std::cell::RefCell::new(None) };
}

const MODALS: [&str; 16] = ["modal-frac", "modal-exam", "modal-grades", "modal-symbols", "modal-stickers", "modal-new", "modal-tpl", "modal-settings", "modal-cover", "modal-confirm", "modal-pick", "modal-stats", "modal-color", "modal-table", "modal-plot", "modal-marks"];

const PEN_COLORS: [&str; 20] = [
    "#18181f", "#4a4b57", "#8e909c", "#ffffff", "#1e3a8a", "#1e5ae6", "#2fb5ff", "#0f9d8f", "#1f9d55", "#7cc242",
    "#f5c400", "#ff8a00", "#e12d3c", "#b3123b", "#ff5fa2", "#a142f4", "#6d5cff", "#8d5524", "#c9a26b", "#2c2c54",
];
const HL_COLORS: [&str; 10] = ["#ffdd00", "#ffb000", "#ff8ad8", "#ff6b6b", "#78e678", "#3fe0c5", "#7cc8ff", "#b69cff", "#c9c9c9", "#ffffff"];
const PAPERS: [(&str, &str); 7] = [("#ffffff", "Weiß"), ("#fbf6e9", "Creme"), ("#fff8c4", "Notizgelb"), ("#e9eef5", "Hellblau"), ("#2a2b31", "Dunkel"), ("#111214", "Schwarz"), ("#1f3b2d", "Tafelgrün")];

fn closest(e: &Event, attr: &str) -> Option<web_sys::Element> {
    let t: web_sys::Element = e.target()?.dyn_into().ok()?;
    t.closest(&format!("[{}]", attr)).ok().flatten()
}

pub fn bind_ui() {
    // Alle Klicks mit data-act
    on(&dom(), "click", |e: Event| {
        if let Some(el) = closest(&e, "data-act") {
            let act = el.get_attribute("data-act").unwrap_or_default();
            let arg = el.get_attribute("data-arg").unwrap_or_default();
            e.stop_propagation();
            with_app(|a| a.action(&act, &arg, &el));
        } else {
            // Klick ins Leere schließt Menüs
            let in_pop = e.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()).and_then(|t| t.closest(".popover, .modal-card").ok().flatten()).is_some();
            if !in_pop {
                with_app(|a| a.close_popovers());
            }
        }
    });
    // Modale Hintergründe schließen
    for m in MODALS {
        on(&el(m), "pointerdown", move |e: PointerEvent| {
            let on_bg = e.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()).map(|t| t.id() == m).unwrap_or(false);
            if on_bg {
                with_app(|a| a.close_modals());
            }
        });
    }
    let input_handler = |e: Event| {
        if let Some(el) = closest(&e, "data-input") {
            let name = el.get_attribute("data-input").unwrap_or_default();
            let (value, checked) = match el.dyn_ref::<HtmlInputElement>() {
                Some(i) => (i.value(), i.checked()),
                None => match el.dyn_ref::<HtmlSelectElement>() {
                    Some(s) => (s.value(), false),
                    None => (String::new(), false),
                },
            };
            let kind = e.type_();
            with_app(|a| a.input(&name, &value, checked, &kind));
        }
    };
    on(&dom(), "input", input_handler);
    on(&dom(), "change", input_handler);
    on(&el("dlg-input"), "keydown", |e: web_sys::KeyboardEvent| {
        if e.key() == "Enter" {
            e.prevent_default();
            with_app(|a| a.dialog_ok());
        }
    });
    // Textfeld: Höhe mitwachsen lassen
    on(&el("text-editor"), "input", |_: Event| super::tools::autosize(&el("text-editor")));
    // S-Pen-Testfeld
    let show_pen = |e: PointerEvent| {
        e.prevent_default();
        let p = e.pressure();
        let kind = match e.pointer_type().as_str() {
            "pen" => "S-Pen",
            "touch" => "Finger",
            _ => "Maus",
        };
        let b = e.buttons();
        style(&el("pen-meter-fill"), "width", &format!("{}%", (p * 100.0).round()));
        set_text(
            "pen-info",
            &format!("{} {} · Druck {:.2} · Neigung {}°/{}°{}", kind, if b & 1 != 0 { "berührt" } else { "schwebt" }, p, e.tilt_x(), e.tilt_y(), if b & 2 != 0 { " · Taste" } else { "" }),
        );
    };
    on(&el("pen-test"), "pointermove", show_pen);
    on(&el("pen-test"), "pointerdown", show_pen);
    let sel_colors: String = ["#18181f", "#1e5ae6", "#e12d3c", "#1f9d55", "#f5c400", "#a142f4"]
        .iter()
        .map(|c| format!("<button data-act=\"sel-color\" data-arg=\"{c}\" title=\"Farbe ändern\" style=\"background:{c}\"></button>", c = c))
        .collect();
    el("sel-colors").set_inner_html(&sel_colors);
    super::storage::bind_files();
}

fn pct(v: f32) -> String {
    format!("{}%", (v * 100.0).round() as i32)
}

impl App {
    /// Zentrale Aktionsverteilung.
    pub fn action(&mut self, act: &str, arg: &str, src: &web_sys::Element) {
        self.action_inner(act, arg, src);
        self.sync_ui_rects();
    }

    fn action_inner(&mut self, act: &str, arg: &str, src: &web_sys::Element) {
        let n = arg.parse::<usize>().unwrap_or(0);
        match act {
            // ----- Bibliothek -----
            "lib-sort" => {
                self.lib_sort = arg.to_string();
                self.render_library();
                self.save_settings();
            }
            "lib-fav" => {
                self.lib_fav = !self.lib_fav;
                self.render_library();
            }
            "new-nb" => self.open_new_dialog(),
            "import-menu" => self.open_popover("pop-import", src),
            "import" => {
                self.close_popovers();
                set_timeout(0, || input_el("file-import").click());
            }
            "import-pdf" | "import-image" | "import-ink" => {
                self.close_popovers();
                let id = match act {
                    "import-pdf" => "file-import-pdf",
                    "import-ink" => "file-import-ink",
                    _ => "file-import-image",
                };
                set_timeout(0, move || input_el(id).click());
            }
            "settings" => self.open_settings(),
            "card" => self.open_guarded(arg.to_string()),
            "trash-open" => {
                // zweites Antippen schließt den Papierkorb wieder
                self.show_trash = !self.show_trash;
                self.folder = None;
                self.render_library();
            }
            "trash-empty" => self.empty_trash_dialog(),
            "trash-menu" => {
                self.ctx_target = Some(arg.to_string());
                self.open_popover("pop-trash", src);
            }
            "trash-restore" | "trash-delete" => {
                if let Some(id) = self.ctx_target.clone() {
                    self.trash_action(act, id);
                }
            }
            "dlg-bio" => self.start_bio(),
            "card-menu" => {
                self.ctx_target = Some(arg.to_string());
                let fav = self.library.iter().find(|m| m.id == arg).map(|m| m.favorite).unwrap_or(false);
                set_text("card-fav-label", if fav { "Aus Favoriten entfernen" } else { "Zu Favoriten" });
                let locked = self.library.iter().find(|m| m.id == arg).map(|m| !m.lock.is_empty()).unwrap_or(false);
                let deck = self.library.iter().find(|m| m.id == arg).map(|m| m.is_deck).unwrap_or(false);
                // Ordner und Notizbuchvorlagen gibt es nur für Notizbücher
                show_el_sel("#pop-card [data-act=card-folder]", !deck);
                show_el_sel("#pop-card [data-act=card-template]", !deck);
                set_text("card-delete-label", if deck { "Lernkarten-Set löschen" } else { "Löschen" });
                set_text("card-lock-label", if locked { "PIN-Sperre entfernen …" } else { "Mit PIN sperren …" });
                self.open_popover("pop-card", src);
            }
            a if a.starts_with("card-") || a == "cover-pick" => self.card_action(a, n),

            // ----- Editor-Leiste -----
            "back" => self.close_editor(),
            "nav" => self.toggle_nav(),
            "nav-filter" => {
                if let Some(ed) = &mut self.ed {
                    ed.nav_bookmarks = arg == "bm";
                    ed.nav_toc = arg == "toc";
                }
                self.render_nav();
            }
            a if a.starts_with("folder-") => self.folder_action(a, arg, src),
            "pick" => self.pick(arg),
            "pick-mode" => {
                self.pick_copy = arg == "copy";
                App::mark_pick_mode(self.pick_copy);
            }
            "ruler-menu" => self.open_popover("pop-ruler", src),
            "ruler" => {
                self.close_popovers();
                if let Some(ed) = &mut self.ed {
                    ed.ruler = match arg {
                        "ruler" => Some(super::ruler::Ruler::new(super::ruler::RulerKind::Ruler, ed.vw, ed.vh)),
                        "triangle" => Some(super::ruler::Ruler::new(super::ruler::RulerKind::Triangle, ed.vw, ed.vh)),
                        "compass" => Some(super::ruler::Ruler::new(super::ruler::RulerKind::Compass, ed.vw, ed.vh)),
                        _ => None,
                    };
                    ed.dirty = true;
                    let on = ed.ruler.is_some();
                    set_class(&el("btn-ruler"), "active", on);
                }
                self.request_frame();
            }
            "study" => self.open_study(),
            "deck-study" => self.study_deck(arg.to_string()),
            "deck-new" => {
                let visual = "<div class=\"dlg-icon accent\"><svg><use href=\"#i-cards\"/></svg></div>";
                self.open_dialog(
                    visual,
                    "Neues Lernkarten-Set",
                    "Jede Seite ist eine Karte: <b>oben die Frage</b>, <b>unten die Antwort</b>. Weitere Karten fügst du oben mit „Seite hinzufügen“ hinzu. Name des Sets:",
                    "Erstellen",
                    false,
                    Some(""),
                    Pending::NewDeck,
                );
            }
            "study-close" => self.close_study(),
            "study-home" => self.render_study_home(),
            "study-start" => self.study_start(arg),
            "stats" => self.open_stats(),
            "study-flip" => self.study_flip(),
            "study-answer" => self.study_answer(arg == "right"),
            "fc-del" => {
                if let Ok(id) = arg.parse::<u32>() {
                    self.delete_card(id);
                }
            }
            "sel-card" => self.selection_to_card(),
            "sel-align" => self.open_popover("pop-align", src),
            a if a.starts_with("align-") => {
                self.close_popovers();
                let mode = a.trim_start_matches("align-").to_string();
                let n = self.ed.as_mut().map(|e| e.align_selection(&mode)).unwrap_or(0);
                if n == 0 {
                    self.toast("Wähle mit dem Lasso mindestens zwei getrennte Objekte aus");
                }
                self.after_edit();
            }
            "pen-sets" => self.open_pen_sets(src),
            "set-save" | "set-del" | "set-apply" => self.pen_set_action(act, arg),
            "backup-save" => self.create_backup(false),
            "backup-share" => self.create_backup(true),
            "backup-restore" => {
                self.close_modals();
                set_timeout(0, || input_el("file-import").click());
            }
            "share-menu" => self.open_popover("pop-share", src),
            "share-png" | "share-page-pdf" | "share-pdf" | "share-znote" => {
                self.close_popovers();
                super::storage::share_next();
                match act {
                    "share-png" => self.export_png(),
                    "share-page-pdf" => self.export_pdf(true),
                    "share-pdf" => self.export_pdf(false),
                    _ => self.export_znote(None),
                }
            }

            "marks" => self.open_marks_dialog(),
            "mark-set" | "tag-add" | "tag-del" | "tag-new" => self.marks_action(act, arg),
            "nav-mark" => self.nav_mark_filter(arg),
            "tab" => self.tab_switch(arg.to_string()),
            "tab-close" => self.tab_close(arg),
            "present" => self.present_start(),
            "present-exit" => self.present_exit(),
            "present-prev" => self.present_step(-1),
            "present-next" => self.present_step(1),
            "mind-child" => self.mind_add_child(),
            "mind-text" => {
                if let Some((p, id)) = self.mind_sel {
                    self.mind_edit_text(p, id, false);
                }
            }
            "mind-color" => self.mind_set_color(n),
            "mind-del" => self.mind_delete(),
            "color-custom" => self.open_color_picker(),
            "cp-ok" => {
                self.cp_ok();
                self.save_settings();
            }
            "cp-recent" => self.cp_pick(arg),
            "cp-reset" => {
                let c = self.cp_old;
                self.cp = c.to_hsv();
                self.cp_refresh(true);
            }
            "ed-cover" => {
                self.close_popovers();
                if let Some(ed) = &self.ed {
                    self.ctx_target = Some(ed.nb.id.clone());
                    let html = super::library::cover_dots("cover-pick", true, Some(ed.nb.cover));
                    el("cover-pick").set_inner_html(&html);
                    self.open_modal("modal-cover");
                }
            }
            "cover-page" => {
                self.close_popovers();
                let Some(ed) = &mut self.ed else { return };
                if ed.nb.has_cover_page() {
                    if ed.delete_page(0) {
                        self.toast("Deckblatt entfernt – Rückgängig ist möglich");
                    }
                } else {
                    let p = ed.nb.make_cover_page();
                    ed.nb.pages.insert(0, p.clone());
                    ed.push(crate::engine::Action::AddPage { index: 0, page: p });
                    ed.goto_page(0);
                    self.toast("Deckblatt hinzugefügt – der Titel lässt sich wie Text bearbeiten");
                }
                self.after_pages_change();
            }
            "zoomwin" => self.toggle_zoomwin(),
            a if a.starts_with("zw-") => self.zw_action(a),
            "sel-link" => self.open_link_dialog(),
            "paper-img" => {
                self.paper_all = arg == "all";
                set_timeout(0, || input_el("file-paper").click());
            }
            "paper-clear" => self.clear_paper(),
            "pdf-insert" => {
                self.close_popovers();
                set_timeout(0, || input_el("file-pdf").click());
            }
            "lasso-mode" => {
                self.prefs.lasso_mode = (n as u8).min(2);
                self.refresh_tool_popover();
                self.save_settings();
            }
            "shape-fill" => {
                self.prefs.shape_fill = (n as u8).min(2);
                self.refresh_tool_popover();
                self.save_settings();
            }
            "pipette" => {
                self.close_popovers();
                self.pipette = true;
                set_class(&dom().body().unwrap(), "picking", true);
                self.toast("Pipette: tippe auf die Farbe, die du übernehmen möchtest");
            }
            "page-title" => {
                if let Some(ed) = &self.ed {
                    let i = ed.cur_page;
                    let cur = ed.nb.pages[i].title.clone();
                    let visual = "<div class=\"dlg-icon accent\"><svg><use href=\"#i-toc\"/></svg></div>";
                    let text = format!("Titel für <b>Seite {}</b> – erscheint im Inhaltsverzeichnis (Seitenübersicht → Inhalt). Leer lassen zum Entfernen.", i + 1);
                    self.open_dialog(visual, "Seitentitel", &text, "Speichern", false, Some(&cur), Pending::PageTitle(i));
                }
            }
            "page-to" => self.open_page_to(),
            "save-template" => {
                let cur = self.ed.as_ref().map(|e| e.nb.title.clone()).unwrap_or_default();
                let visual = "<div class=\"dlg-icon accent\"><svg><use href=\"#i-template\"/></svg></div>";
                self.open_dialog(visual, "Als Vorlage speichern", "Neue Notizbücher können dann mit diesen Seiten, Vorlagen und Lernkarten starten. Name der Vorlage:", "Speichern", false, Some(&cur), Pending::SaveTemplate(None));
            }
            "new-from" => {
                self.new_from = if arg.is_empty() { None } else { Some(arg.to_string()) };
                self.refresh_new_dialog();
            }
            "from-del" => {
                let name = self.library.iter().find(|m| m.id == arg).map(|m| m.title.clone()).unwrap_or_default();
                let visual = "<div class=\"dlg-icon danger\"><svg><use href=\"#i-template\"/></svg></div>";
                let text = format!("Die Vorlage <b>„{}“</b> wird gelöscht. Bereits erstellte Notizbücher bleiben erhalten.", esc(&name));
                self.open_dialog(visual, "Vorlage löschen?", &text, "Löschen", true, None, Pending::DeleteTemplate(arg.to_string()));
            }
            "nav-page" => {
                if let Some(ed) = &mut self.ed {
                    if let Some(sel) = &mut ed.nav_sel {
                        // Auswahlmodus: Seite an-/abwählen
                        if let Some(id) = ed.nb.pages.get(n).map(|p| p.id) {
                            match sel.iter().position(|x| *x == id) {
                                Some(k) => {
                                    sel.remove(k);
                                }
                                None => sel.push(id),
                            }
                        }
                        self.render_nav();
                        return;
                    }
                    ed.goto_page(n);
                    // die angetippte Seite ist die aktuelle (auch wenn sie nicht bis zur Mitte reicht)
                    if n < ed.nb.pages.len() {
                        ed.cur_page = n;
                    }
                }
                self.after_view_change();
                self.sync_nav_current();
            }
            "nav-sel" | "nav-sel-all" | "nav-sel-pdf" | "nav-sel-dup" | "nav-sel-rot" | "nav-sel-del" => self.page_sel_action(act),
            "undo" => self.undo(),
            "redo" => self.redo(),
            "tool" => {
                if let Some(t) = Tool::from_code(arg) {
                    if self.tool == t && self.open_pop != Some("pop-tool") && t != Tool::Laser {
                        self.open_tool_popover(src);
                    } else {
                        self.close_popovers();
                        self.set_tool(t);
                    }
                }
            }
            "insert-menu" => self.open_popover("pop-insert", src),
            "insert-table" => self.open_table_dialog(),
            "insert-plot" => self.open_plot_dialog(),
            "sticker-menu" => self.open_sticker_dialog(),
            "symbol-menu" => self.open_symbol_dialog(),
            "frac-menu" => self.open_fraction_dialog(),
            "replay" | "replay-close" | "replay-video" | "replay-share" | "replay-play" | "replay-restart" | "replay-speed" => self.replay_action(act),
            "frac-add" | "frac-insert" => self.fraction_action(act, arg),
            "exam-new" | "exam-edit" | "exam-save" | "exam-delete" => self.exam_action(act, arg),
            "grades-open" | "grade-weight" | "subject-add" | "grade-add" | "grade-del" | "subject-del" => self.grades_action(act, arg),
            "sym-add" | "sym-back" | "sym-space" | "sym-insert" => self.symbol_action(act, arg),
            "sticker" => self.insert_sticker(arg),
            "insert-date" => {
                self.close_popovers();
                self.insert_date(arg == "short");
            }
            "plot-insert" => self.insert_plot(),
            a if a.starts_with("tbl-") => self.table_action(a, arg),
            "image" => {
                self.close_popovers();
                set_timeout(0, || input_el("file-image").click());
            }
            "fav-color" | "fav-size" => self.fav(act, n, src),
            "add-page" => {
                if let Some(ed) = &mut self.ed {
                    let i = ed.add_page_after(ed.cur_page);
                    ed.goto_page(i);
                }
                self.toast("Neue Seite");
                self.after_pages_change();
            }
            "more" => {
                let bm = self.ed.as_ref().map(|e| e.nb.pages[e.cur_page].bookmark).unwrap_or(false);
                set_text("bm-label", if bm { "Lesezeichen entfernen" } else { "Lesezeichen setzen" });
                let cp = self.ed.as_ref().map(|e| e.nb.has_cover_page()).unwrap_or(false);
                set_text("cover-page-label", if cp { "Deckblatt entfernen" } else { "Deckblatt hinzufügen" });
                self.open_popover("pop-more", src);
            }
            "page-prev" => self.goto_relative(-1),
            "page-next" => self.goto_relative(1),
            "fit" => {
                if let Some(ed) = &mut self.ed {
                    let i = ed.cur_page;
                    ed.fit_page(i);
                }
                self.after_view_change();
            }
            "sel-copy" | "sel-cut" | "sel-dup" | "sel-delete" | "sel-color" | "sel-fill" | "paste" | "sel-copy-img" => self.selection_action(act, arg),

            // ----- Mehr-Menü -----
            "export-pdf" => {
                self.close_popovers();
                self.export_pdf(false);
            }
            "export-page-pdf" => {
                self.close_popovers();
                self.export_pdf(true);
            }
            "export-png" => {
                self.close_popovers();
                self.export_png();
            }
            "export-znote" => {
                self.close_popovers();
                self.export_znote(None);
            }
            "template" => self.open_template_dialog(),
            "bookmark" => {
                self.close_popovers();
                if let Some(ed) = &mut self.ed {
                    let i = ed.cur_page;
                    ed.set_page_meta(i, |m| m.bookmark = !m.bookmark);
                    let on = ed.nb.pages[i].bookmark;
                    self.toast(if on { "Lesezeichen gesetzt" } else { "Lesezeichen entfernt" });
                }
                self.after_pages_change();
            }
            "rotate-page" => {
                self.close_popovers();
                self.rotate_page();
            }
            "sel-pin" => self.toggle_pin(),
            a if a.starts_with("xf-") => {
                let kind = a.trim_start_matches("xf-").to_string();
                self.transform_selection_kind(&kind);
            }
            "split" => self.toggle_split(),
            "split-close" => self.close_split(),
            "dup-page" => {
                self.close_popovers();
                if let Some(ed) = &mut self.ed {
                    let i = ed.duplicate_page(ed.cur_page);
                    ed.goto_page(i);
                }
                self.toast("Seite dupliziert");
                self.after_pages_change();
            }
            "del-page" => {
                self.close_popovers();
                if let Some(ed) = &self.ed {
                    let (i, n) = (ed.cur_page, ed.nb.pages.len());
                    if n <= 1 {
                        self.toast("Die letzte Seite kann nicht gelöscht werden");
                    } else {
                        let visual = "<div class=\"dlg-icon danger\"><svg><use href=\"#i-trash\"/></svg></div>";
                        let text = format!("<b>Seite {} von {}</b> wird gelöscht. Mit „Rückgängig“ kannst du sie wiederherstellen.", i + 1, n);
                        self.open_dialog(visual, "Seite löschen?", &text, "Löschen", true, None, Pending::DeletePage(i));
                    }
                }
            }
            "dlg-ok" => self.dialog_ok(),
            "page-up" | "page-down" => {
                if let Some(ed) = &mut self.ed {
                    let i = ed.cur_page;
                    let j = if act == "page-up" { i.wrapping_sub(1) } else { i + 1 };
                    if ed.move_page(i, j) {
                        ed.goto_page(j);
                    }
                }
                self.after_pages_change();
            }

            // ----- Dialoge -----
            "modal-close" => self.close_modals(),
            "new-format" => {
                self.new_nb.format = n as u8;
                self.refresh_new_dialog();
            }
            "new-sp" => {
                self.new_nb.template.spacing = n as u8;
                self.refresh_new_dialog();
            }
            "new-cover" => {
                self.new_nb.cover = n as u8;
                self.refresh_new_dialog();
            }
            "new-tpl" => {
                self.new_nb.template.kind = TemplateKind::from_code(arg);
                self.refresh_new_dialog();
            }
            "new-paper" => {
                if let Some(c) = Color::from_hex(arg) {
                    self.new_nb.paper = c;
                }
                self.refresh_new_dialog();
            }
            "new-create" => self.create_from_dialog(),
            "tpl-tpl" => {
                self.tpl_choice.kind = TemplateKind::from_code(arg);
                self.refresh_tpl_dialog();
            }
            "tpl-sp" => {
                self.tpl_choice.spacing = n as u8;
                self.refresh_tpl_dialog();
            }
            "tpl-paper" => {
                if let Some(c) = Color::from_hex(arg) {
                    self.tpl_paper = c;
                }
                self.refresh_tpl_dialog();
            }
            "tpl-apply" => {
                let (t, p) = (self.tpl_choice, self.tpl_paper);
                if let Some(ed) = &mut self.ed {
                    if arg == "all" {
                        ed.set_all_pages_meta(t, p);
                    } else {
                        let i = ed.cur_page;
                        ed.set_page_meta(i, |m| {
                            m.template = t;
                            m.paper = p;
                        });
                    }
                }
                self.close_modals();
                self.after_pages_change();
            }
            "set-theme" => {
                self.settings.theme = Theme::from_code(arg);
                self.apply_theme();
                self.refresh_settings();
                self.save_settings();
            }

            // ----- Werkzeug-Optionen -----
            "pen-kind" => {
                self.prefs.pen_kind = PenKind::from_code(arg);
                self.refresh_tool_popover();
                self.save_settings();
            }
            "pop-color" => {
                if let Some(c) = Color::from_hex(arg) {
                    self.set_active_color(c);
                }
            }
            "er-mode" => {
                self.prefs.er_mode = if arg == "stroke" { EraserMode::Stroke } else { EraserMode::Precise };
                self.refresh_tool_popover();
                self.save_settings();
            }
            "laser-color" => {
                if let Some(c) = Color::from_hex(arg) {
                    self.prefs.laser_color = color4(c, 255);
                }
                self.refresh_tool_popover();
                self.save_settings();
            }
            _ => {}
        }
    }

    /// Eingabefelder (Regler, Schalter, Texte).
    pub fn input(&mut self, name: &str, value: &str, checked: bool, kind: &str) {
        let f = value.parse::<f32>().unwrap_or(0.0);
        match name {
            "frac-text" => self.update_fraction_preview(),
            "replay-pos" => self.replay_seek(value.parse::<f64>().unwrap_or(0.0)),
            "lib-search" => {
                self.lib_query = value.to_lowercase();
                self.render_library();
            }
            "title" => {
                if let Some(ed) = &mut self.ed {
                    let t = value.trim();
                    let t: String = if t.is_empty() { "Ohne Titel".into() } else { t.chars().take(80).collect() };
                    // Titel auf dem Deckblatt wandert mit
                    if ed.nb.set_title(&t) {
                        let id = ed.nb.pages[0].id;
                        ed.bmps.remove(&id);
                        ed.thumbs_dirty = true;
                        ed.dirty = true;
                        self.request_frame();
                    }
                    if let Some(ed) = &mut self.ed {
                        ed.nb.modified = wall_clock();
                    }
                }
                if kind == "change" {
                    self.mark_unsaved();
                }
            }
            "new-cover-page" => {
                self.new_nb.cover_page = checked;
                return;
            }
            "new-title" => {
                let t = if value.trim().is_empty() { "Notizbuch" } else { value.trim() };
                set_text("new-cover-title", t);
            }
            "pop-size" => {
                let p = &mut self.prefs;
                match self.tool {
                    Tool::Highlighter => p.hl_sizes[p.hl_si] = f,
                    Tool::Eraser => p.er_sizes[p.er_si] = f,
                    Tool::Text => p.text_size = f,
                    _ => p.pen_sizes[p.pen_si] = f,
                }
                set_text("pop-size-v", &format!("{:.1}", f));
                self.refresh_favs();
                if kind == "change" {
                    self.save_settings();
                }
            }
            "plot-f" | "plot-x0" | "plot-x1" | "plot-y0" | "plot-y1" | "plot-yauto" | "plot-grid" => {
                self.refresh_plot();
                return;
            }
            "cp-hex" => {
                self.cp_hex_input(value);
                return;
            }
            "pop-custom" => {
                if let Some(c) = Color::from_hex(value) {
                    self.set_active_color(c);
                }
            }
            "pop-straight" => self.prefs.hl_straight = checked,
            "pop-er-hl" => self.prefs.er_hl_only = checked,
            "pop-er-back" => self.prefs.er_auto_back = checked,
            "pop-lasso-ink" => self.prefs.lasso_ink = checked,
            "pop-lasso-text" => self.prefs.lasso_text = checked,
            "pop-lasso-img" => self.prefs.lasso_img = checked,
            "pop-bold" => self.prefs.text_bold = checked,
            "set-pressure" => self.settings.pressure = checked,
            "set-smooth" => {
                self.settings.smoothing = f / 100.0;
                set_text("set-smooth-v", &pct(self.settings.smoothing));
            }
            "set-quick" => self.settings.quickshape = checked,
            "set-button" => self.settings.pen_button = PenButton::from_code(value),
            "set-palm" => {
                self.settings.palm_lock = f / 10.0;
                set_text("set-palm-v", &format!("{:.1} s", self.settings.palm_lock));
            }
            "set-scroll" => self.settings.one_finger_scroll = checked,
            "set-finger" => self.settings.finger_draw = checked,
            "study-due" => {
                self.settings.due_mode = checked;
                self.save_settings();
                self.render_study_home();
                return;
            }
            _ => return,
        }
        if name.starts_with("set-") || name.starts_with("pop-") && kind == "change" {
            self.save_settings();
        }
    }

    // -----------------------------------------------------------------------
    // Werkzeuge, Favoriten
    // -----------------------------------------------------------------------

    pub fn set_tool(&mut self, t: Tool) {
        if self.tool != t {
            if self.tool != Tool::Eraser {
                self.prev_tool = self.tool;
            }
            self.tool = t;
        }
        if t != Tool::Lasso {
            if let Some(ed) = &mut self.ed {
                if ed.selection.take().is_some() {
                    ed.dirty = true;
                }
                ed.paste_at = None;
            }
        }
        if t != Tool::Text && self.ed.as_ref().map(|e| e.text_edit.is_some()).unwrap_or(false) {
            self.commit_text();
        }
        self.refresh_toolbar();
        self.position_overlays();
        self.request_frame();
    }

    pub fn refresh_toolbar(&mut self) {
        self.position_mind_menu();
        let active = self.effective_tool();
        let list = dom().query_selector_all(".tb.tool").unwrap();
        for i in 0..list.length() {
            let b: web_sys::Element = list.get(i).unwrap().unchecked_into();
            set_class(&b, "active", b.get_attribute("data-arg").as_deref() == Some(active.code()));
        }
        self.refresh_favs();
        self.refresh_history_buttons();
    }

    pub fn refresh_history_buttons(&mut self) {
        let (u, r) = self.ed.as_ref().map(|e| (e.history.can_undo(), e.history.can_redo())).unwrap_or((false, false));
        set_class(&el("btn-undo"), "disabled", !u);
        set_class(&el("btn-redo"), "disabled", !r);
    }

    /// Favoriten-Punkte (drei Farben, drei Stärken) für das aktive Werkzeug.
    pub fn refresh_favs(&mut self) {
        let p = &self.prefs;
        let tool = self.effective_tool();
        let (colors, ci, sizes, si, smax): FavSpec = match tool {
            Tool::Pen | Tool::Shape => (Some(&p.pen_colors), p.pen_ci, Some(&p.pen_sizes), p.pen_si, 8.0),
            Tool::Highlighter => (Some(&p.hl_colors), p.hl_ci, Some(&p.hl_sizes), p.hl_si, 30.0),
            Tool::Eraser => (None, 0, Some(&p.er_sizes), p.er_si, 60.0),
            Tool::Text => (Some(&p.text_colors), p.text_ci, None, 0, 1.0),
            _ => (None, 0, None, 0, 1.0),
        };
        let mut html = String::new();
        if let Some(c) = colors {
            for (i, col) in c.iter().enumerate() {
                html.push_str(&format!(
                    "<button class=\"fav-color{}\" data-act=\"fav-color\" data-arg=\"{}\" title=\"Farbe {}\"><span style=\"background:{}\"></span></button>",
                    if i == ci { " active" } else { "" },
                    i,
                    i + 1,
                    hex4(*col)
                ));
            }
        }
        if let Some(sz) = sizes {
            for (i, s) in sz.iter().enumerate() {
                let d = 4.0 + (s / smax).sqrt() * 14.0;
                html.push_str(&format!(
                    "<button class=\"fav-size{}\" data-act=\"fav-size\" data-arg=\"{}\" title=\"Stärke {:.1}\"><span style=\"width:{d}px;height:{d}px\"></span></button>",
                    if i == si { " active" } else { "" },
                    i,
                    s,
                    d = d.min(18.0)
                ));
            }
        }
        if html.is_empty() {
            // Platzhalter mit dem Werkzeugnamen – die Leiste behält immer dieselbe Breite
            html = format!("<span class=\"fav-label\">{}</span>", tool.name_de());
        }
        el("favs").set_inner_html(&html);
        show("fav-sep", true);
    }

    fn fav(&mut self, act: &str, i: usize, src: &web_sys::Element) {
        let i = i.min(2);
        let tool = self.effective_tool();
        let p = &mut self.prefs;
        let was_active;
        if act == "fav-color" {
            let slot = match tool {
                Tool::Highlighter => &mut p.hl_ci,
                Tool::Text => &mut p.text_ci,
                _ => &mut p.pen_ci,
            };
            was_active = *slot == i;
            *slot = i;
        } else {
            let slot = match tool {
                Tool::Highlighter => &mut p.hl_si,
                Tool::Eraser => &mut p.er_si,
                _ => &mut p.pen_si,
            };
            was_active = *slot == i;
            *slot = i;
        }
        self.refresh_favs();
        self.save_settings();
        if was_active {
            // aktiven Favoriten erneut antippen → bearbeiten
            let anchor = dom().query_selector(&format!(".tb.tool[data-arg=\"{}\"]", if tool == Tool::Shape { "shape" } else { tool.code() })).ok().flatten().unwrap_or_else(|| src.clone());
            self.open_tool_popover(&anchor);
        }
    }

    pub fn set_active_color_pub(&mut self, c: Color) {
        self.set_active_color(c);
    }

    fn set_active_color(&mut self, c: Color) {
        let p = &mut self.prefs;
        match self.tool {
            Tool::Highlighter => p.hl_colors[p.hl_ci] = color4(c, 90),
            Tool::Text => p.text_colors[p.text_ci] = color4(c, 255),
            _ => p.pen_colors[p.pen_ci] = color4(c, 255),
        }
        self.refresh_favs();
        self.refresh_tool_popover();
        self.save_settings();
    }

    // -----------------------------------------------------------------------
    // Popover
    // -----------------------------------------------------------------------

    pub fn open_popover(&mut self, id: &'static str, anchor: &web_sys::Element) {
        let open = self.open_pop == Some(id);
        self.close_popovers();
        if open {
            return;
        }
        let pop = el(id);
        set_class(&pop, "hidden", false);
        let r = anchor.get_bounding_client_rect();
        let vw = window().inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1000.0);
        let pw = pop.offset_width() as f64;
        let left = (r.left() + r.width() * 0.5 - pw * 0.5).clamp(12.0, (vw - pw - 12.0).max(12.0));
        style(&pop, "left", &format!("{}px", left));
        // unten kein Platz → über dem Knopf öffnen bzw. in den Bildschirm schieben
        let vh = window().inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(800.0);
        let ph = pop.offset_height() as f64;
        let mut top = r.bottom() + 10.0;
        if top + ph > vh - 12.0 {
            top = if r.top() - 10.0 - ph >= 12.0 { r.top() - 10.0 - ph } else { (vh - ph - 12.0).max(12.0) };
        }
        style(&pop, "top", &format!("{}px", top));
        self.open_pop = Some(id);
    }

    pub fn close_popovers(&mut self) {
        for p in POPS {
            set_class(&el(p), "hidden", true);
        }
        self.open_pop = None;
    }

    fn open_tool_popover(&mut self, anchor: &web_sys::Element) {
        self.build_tool_popover();
        self.open_popover("pop-tool", anchor);
    }

    fn refresh_tool_popover(&mut self) {
        if self.open_pop == Some("pop-tool") {
            self.build_tool_popover();
        }
    }

    fn build_tool_popover(&mut self) {
        let p = &self.prefs;
        let tool = self.tool;
        let mut h = format!("<div class=\"pop-title\">{}</div>", tool.name_de());
        let swatches = |list: &[&str], cur: [u8; 4]| -> String {
            let mut s = String::from("<div class=\"swatches\">");
            for c in list {
                let active = c.eq_ignore_ascii_case(&hex4(cur));
                s.push_str(&format!("<button class=\"sw{}\" data-act=\"pop-color\" data-arg=\"{c}\" style=\"background:{c}\"></button>", if active { " active" } else { "" }, c = c));
            }
            s.push_str(&format!("<button class=\"sw custom\" data-act=\"color-custom\" title=\"Eigene Farbe\" data-cur=\"{}\"></button><button class=\"sw pipette\" data-act=\"pipette\" title=\"Pipette – Farbe von der Seite übernehmen\"><svg><use href=\"#i-pipette\"/></svg></button></div>", hex4(cur)));
            s
        };
        let slider = |min: f32, max: f32, step: f32, v: f32, label: &str| -> String {
            format!(
                "<div class=\"pop-row\"><span>{}</span><input type=\"range\" data-input=\"pop-size\" min=\"{}\" max=\"{}\" step=\"{}\" value=\"{}\"><output id=\"pop-size-v\">{:.1}</output></div>",
                label, min, max, step, v, v
            )
        };
        let toggle = |name: &str, label: &str, on: bool| -> String {
            format!("<label class=\"row toggle\"><span>{}</span><span class=\"switch\"><input type=\"checkbox\" data-input=\"{}\"{}><span></span></span></label>", label, name, if on { " checked" } else { "" })
        };
        match tool {
            Tool::Pen | Tool::Shape => {
                h.push_str("<div class=\"seg pen-kinds\">");
                for k in PenKind::PENS {
                    h.push_str(&format!("<button class=\"{}\" data-act=\"pen-kind\" data-arg=\"{}\">{}</button>", if k == p.pen_kind { "active" } else { "" }, k.code(), k.name_de()));
                }
                h.push_str("</div>");
                h.push_str(&slider(0.3, 8.0, 0.1, p.pen_sizes[p.pen_si], "Stärke"));
                h.push_str(&swatches(&PEN_COLORS, p.pen_colors[p.pen_ci]));
                h.push_str(&format!(
                    "<div class=\"pop-row\"><span>Formen füllen</span><div class=\"seg small\"><button class=\"{}\" data-act=\"shape-fill\" data-arg=\"0\">Aus</button><button class=\"{}\" data-act=\"shape-fill\" data-arg=\"1\">Hell</button><button class=\"{}\" data-act=\"shape-fill\" data-arg=\"2\">Voll</button></div></div>",
                    if p.shape_fill == 0 { "active" } else { "" },
                    if p.shape_fill == 1 { "active" } else { "" },
                    if p.shape_fill == 2 { "active" } else { "" }
                ));
                if tool == Tool::Shape {
                    h.push_str("<p class=\"hint\">Zeichne eine Form – sie wird automatisch zu Linie, Kreis, Ellipse, Dreieck, Rechteck oder Vieleck. Geschlossene Formen werden auf Wunsch gefüllt.</p>");
                }
            }
            Tool::Highlighter => {
                h.push_str(&slider(4.0, 30.0, 0.5, p.hl_sizes[p.hl_si], "Breite"));
                h.push_str(&swatches(&HL_COLORS, p.hl_colors[p.hl_ci]));
                h.push_str(&toggle("pop-straight", "Gerade Linien", p.hl_straight));
            }
            Tool::Eraser => {
                h.push_str(&format!(
                    "<div class=\"seg\"><button class=\"{}\" data-act=\"er-mode\" data-arg=\"precise\">Präzise</button><button class=\"{}\" data-act=\"er-mode\" data-arg=\"stroke\">Ganzer Strich</button></div>",
                    if p.er_mode == EraserMode::Precise { "active" } else { "" },
                    if p.er_mode == EraserMode::Stroke { "active" } else { "" }
                ));
                h.push_str(&slider(2.0, 60.0, 1.0, p.er_sizes[p.er_si], "Größe"));
                h.push_str(&toggle("pop-er-hl", "Nur Textmarker radieren", p.er_hl_only));
                h.push_str(&toggle("pop-er-back", "Danach zurück zum Stift", p.er_auto_back));
            }
            Tool::Lasso => {
                let m = p.lasso_mode;
                let a = |k: u8| if m == k { "active" } else { "" };
                h.push_str(&format!(
                    "<div class=\"seg lasso-modes\"><button class=\"{}\" data-act=\"lasso-mode\" data-arg=\"0\">Lasso</button><button class=\"{}\" data-act=\"lasso-mode\" data-arg=\"1\">Linie</button><button class=\"{}\" data-act=\"lasso-mode\" data-arg=\"2\">Rechteck</button></div>",
                    a(0), a(1), a(2)
                ));
                h.push_str(&toggle("pop-lasso-ink", "Handschrift auswählen", p.lasso_ink));
                h.push_str(&toggle("pop-lasso-text", "Text auswählen", p.lasso_text));
                h.push_str(&toggle("pop-lasso-img", "Bilder auswählen", p.lasso_img));
                h.push_str(match m {
                    1 => "<p class=\"hint\">Linie ziehen: alles, was die Linie berührt, wird ausgewählt. Danach ziehen = verschieben, runder Griff = skalieren. Kurz tippen = Einfügen.</p>",
                    2 => "<p class=\"hint\">Rechteck aufziehen: alles darin wird ausgewählt (bei PDFs auch der Ausschnitt). Danach ziehen = verschieben. Kurz tippen = Einfügen.</p>",
                    _ => "<p class=\"hint\">Einkreisen zum Auswählen. Danach ziehen = verschieben, runder Griff = skalieren. Kurz tippen = Einfügen.</p>",
                });
            }
            Tool::Text => {
                h.push_str(&slider(8.0, 48.0, 1.0, p.text_size, "Größe"));
                h.push_str(&toggle("pop-bold", "Fett", p.text_bold));
                h.push_str(&swatches(&PEN_COLORS, p.text_colors[p.text_ci]));
            }
            Tool::Mind => {
                h.push_str("<p class=\"hint\">Auf eine freie Stelle tippen = neue Blase. Blase antippen = auswählen (Ast hinzufügen, Farbe, Text, löschen), nochmal antippen = Text ändern. Ziehen verschiebt – die Verbindungen wandern mit.</p>");
            }
            Tool::Laser => {
                h.push_str("<div class=\"swatches\">");
                for c in ["#ff323c", "#24d17e", "#2fb5ff", "#ffb000"] {
                    h.push_str(&format!("<button class=\"sw{}\" data-act=\"laser-color\" data-arg=\"{c}\" style=\"background:{c}\"></button>", if c.eq_ignore_ascii_case(&hex4(p.laser_color)) { " active" } else { "" }, c = c));
                }
                h.push_str("</div><p class=\"hint\">Der Laserpointer verblasst nach kurzer Zeit – ideal zum Präsentieren.</p>");
            }
        }
        el("pop-tool").set_inner_html(&h);
    }

    // -----------------------------------------------------------------------
    // Seiten
    // -----------------------------------------------------------------------

    pub fn undo(&mut self) {
        let ok = self.ed.as_mut().map(|e| e.undo()).unwrap_or(false);
        if ok {
            self.toast("Rückgängig");
            self.after_pages_change();
        }
    }

    pub fn redo(&mut self) {
        let ok = self.ed.as_mut().map(|e| e.redo()).unwrap_or(false);
        if ok {
            self.toast("Wiederholt");
            self.after_pages_change();
        }
    }

    pub fn goto_relative(&mut self, d: i32) {
        if let Some(ed) = &mut self.ed {
            let i = (ed.cur_page as i32 + d).clamp(0, ed.nb.pages.len() as i32 - 1) as usize;
            ed.goto_page(i);
        }
        self.after_view_change();
    }

    pub fn after_view_change(&mut self) {
        self.position_overlays();
        self.update_pill();
        self.request_frame();
    }

    /// Nach Seiten-Änderungen: Übersicht, Anzeige, Speichern.
    pub fn after_pages_change(&mut self) {
        if let Some(ed) = &mut self.ed {
            ed.clamp();
            ed.dirty = true;
            ed.thumbs_dirty = true;
        }
        if self.ed.as_ref().map(|e| e.nav_open).unwrap_or(false) {
            self.render_nav();
        }
        self.after_edit();
        self.update_pill();
    }

    /// Lila Rahmen in der Seitenleiste auf die aktuelle Seite setzen und sie dort ins Bild holen.
    pub fn sync_nav_current(&self) {
        let Some(ed) = &self.ed else { return };
        if !ed.nav_open {
            return;
        }
        let cur = if ed.nav_sel.is_some() { usize::MAX } else { ed.cur_page };
        let key = (cur, ed.nb.pages.len(), ed.nb.id.clone());
        let changed = NAV_CUR.with(|c| {
            let mut c = c.borrow_mut();
            let ch = c.as_ref() != Some(&key);
            *c = Some(key);
            ch
        });
        if !changed {
            return;
        }
        let Ok(items) = dom().query_selector_all("#nav-list .nav-item, #nav-list .toc-item") else { return };
        for k in 0..items.length() {
            let Some(node) = items.item(k) else { continue };
            let Ok(item) = node.dyn_into::<web_sys::Element>() else { continue };
            let is_cur = item.get_attribute("data-arg").and_then(|a| a.parse::<usize>().ok()) == Some(cur);
            set_class(&item, "current", is_cur);
            if is_cur {
                // sanft ins Bild scrollen, ohne die Liste unnötig zu bewegen
                let opts = js_sys::Object::new();
                let _ = js_sys::Reflect::set(&opts, &"block".into(), &"nearest".into());
                let _ = js_sys::Reflect::set(&opts, &"behavior".into(), &"smooth".into());
                if let Ok(f) = js_sys::Reflect::get(&item, &"scrollIntoView".into()) {
                    if let Ok(f) = f.dyn_into::<js_sys::Function>() {
                        let _ = f.call1(&item, &opts);
                    }
                }
            }
        }
    }

    pub fn update_pill(&mut self) {
        self.update_scrubber();
        self.sync_nav_current();
        let Some(ed) = &self.ed else { return };
        let title = &ed.nb.pages[ed.cur_page].title;
        let label = if title.is_empty() {
            format!("{} / {}", ed.cur_page + 1, ed.nb.pages.len())
        } else {
            let short: String = title.chars().take(24).collect();
            format!("{} / {} · {}", ed.cur_page + 1, ed.nb.pages.len(), short)
        };
        let p = &ed.nb.pages[ed.cur_page];
        let label = if p.tags.is_empty() { label } else { format!("{} · {}", label, p.tags.iter().take(2).map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" ")) };
        let dot = if p.mark > 0 { crate::engine::note::MARK_COLORS[(p.mark - 1) as usize].1 } else { "" };
        style(&el("page-label"), "--mark", if dot.is_empty() { "transparent" } else { dot });
        set_class(&el("page-label"), "marked", !dot.is_empty());
        let zoom = format!("{:.0}%", ed.zoom_percent());
        let key = format!("{}|{}", label, zoom);
        let changed = PILL.with(|p| {
            let mut p = p.borrow_mut();
            if *p != key {
                *p = key;
                true
            } else {
                false
            }
        });
        if changed {
            set_text("page-label", &label);
            set_text("zoom-label", &zoom);
        }
    }

    fn toggle_nav(&mut self) {
        let open = !self.ed.as_ref().is_some_and(|e| e.nav_open);
        self.set_nav_open(open);
    }

    pub fn render_nav(&mut self) {
        let Some(ed) = &mut self.ed else { return };
        set_class(&el("nav-all"), "active", !ed.nav_bookmarks && !ed.nav_toc);
        set_class(&el("nav-bm"), "active", ed.nav_bookmarks);
        set_class(&el("nav-toc"), "active", ed.nav_toc);
        let mut html = String::new();
        if ed.nav_toc {
            for (i, p) in ed.nb.pages.iter().enumerate() {
                if p.title.is_empty() {
                    continue;
                }
                html.push_str(&format!(
                    "<button class=\"toc-item{}\" data-act=\"nav-page\" data-arg=\"{}\"><span class=\"toc-num\">{}</span><span class=\"toc-title\">{}</span></button>",
                    if i == ed.cur_page { " current" } else { "" },
                    i,
                    i + 1,
                    esc(&p.title)
                ));
            }
            if html.is_empty() {
                html = "<p class=\"muted nav-empty\">Noch kein Inhaltsverzeichnis.<br><br>Gib Seiten einen Titel: ⋯ → „Seitentitel“.</p>".into();
            }
            html.push_str("<div class=\"nav-actions\"><button class=\"btn small\" data-act=\"page-title\">Titel für aktuelle Seite …</button></div>");
            el("nav-list").set_inner_html(&html);
            return;
        }
        let filter = ed.nav_filter.clone();
        for (i, p) in ed.nb.pages.iter().enumerate() {
            if ed.nav_bookmarks && !p.bookmark {
                continue;
            }
            if !super::marks::page_matches(p, filter.as_deref()) {
                continue;
            }
            let mark = if p.mark > 0 { format!("<span class=\"nav-mark\" style=\"background:{}\"></span>", crate::engine::note::MARK_COLORS[(p.mark - 1) as usize].1) } else { String::new() };
            let tags: String = p.tags.iter().take(3).map(|t| format!("<span class=\"nav-tag\">#{}</span>", esc(t))).collect();
            let picked = ed.nav_sel.as_ref().map(|s| s.contains(&p.id));
            html.push_str(&format!(
                "<button class=\"nav-item{}{}\" data-act=\"nav-page\" data-arg=\"{}\">{}<canvas data-page=\"{}\"></canvas><span class=\"nav-num\">{}{}{}</span>{}</button>",
                if i == ed.cur_page && picked.is_none() { " current" } else { "" },
                match picked {
                    Some(true) => " selecting picked",
                    Some(false) => " selecting",
                    None => "",
                },
                i,
                mark,
                p.id,
                i + 1,
                if p.bookmark { " <svg class=\"bm\"><use href=\"#i-bookmark\"/></svg>" } else { "" },
                if p.title.is_empty() { String::new() } else { format!(" · {}", esc(&p.title)) },
                if tags.is_empty() { String::new() } else { format!("<span class=\"nav-tags\">{}</span>", tags) }
            ));
        }
        if html.is_empty() {
            html = if filter.is_some() { "<p class=\"muted nav-empty\">Keine Seiten mit dieser Markierung</p>".into() } else { "<p class=\"muted nav-empty\">Keine Seiten mit Lesezeichen</p>".into() };
        }
        let bar = self.nav_filter_bar();
        html.insert_str(0, &bar);
        let selbar = self.nav_selbar();
        let Some(ed) = &mut self.ed else { return };
        set_class(&el("nav-sel-btn"), "active", ed.nav_sel.is_some());
        if ed.nav_sel.is_some() {
            html.push_str(&selbar);
        } else {
            html.push_str("<div class=\"nav-actions\"><button class=\"btn small\" data-act=\"page-up\">Seite nach oben</button><button class=\"btn small\" data-act=\"page-down\">Seite nach unten</button></div>");
        }
        el("nav-list").set_inner_html(&html);
        NAV_CUR.with(|c| *c.borrow_mut() = None);
        ed.thumb_versions.clear();
        ed.thumbs_dirty = true;
        self.request_frame();
    }

    // -----------------------------------------------------------------------
    // Dialoge
    // -----------------------------------------------------------------------

    pub fn open_modal(&mut self, id: &str) {
        self.close_popovers();
        self.close_modals();
        show(id, true);
    }

    pub fn close_modals(&mut self) {
        if !el("modal-confirm").class_list().contains("hidden") || !el("modal-pick").class_list().contains("hidden") {
            self.pending = None;
        }
        for m in MODALS {
            show(m, false);
        }
    }

    fn tpl_tiles(prefix: &str, cur: TemplateKind) -> String {
        TemplateKind::ALL
            .iter()
            .map(|k| {
                let prev = if k.is_colored() { format!("<span class=\"tpl-prev svg\">{}</span>", Self::tpl_svg(*k)) } else { format!("<span class=\"tpl-prev tp-{}\"></span>", k.code()) };
                format!("<button class=\"tpl{}\" data-act=\"{}-tpl\" data-arg=\"{}\">{}{}</button>", if *k == cur { " active" } else { "" }, prefix, k.code(), prev, k.name_de())
            })
            .collect()
    }

    /// Vorschau einer Vorlage als SVG (aus denselben Grundformen wie die Seite).
    fn tpl_svg(k: TemplateKind) -> String {
        let (w, h) = if k == TemplateKind::Flashcard { (420.0, 595.0) } else { (595.0, 842.0) };
        format!(
            "<svg viewBox=\"0 0 {w} {h}\" preserveAspectRatio=\"none\" font-family=\"Inter, system-ui, sans-serif\">{}</svg>",
            super::svg::body(Template::new(k, 1).primitives(w, h))
        )
    }

    fn paper_dots(prefix: &str, cur: Color) -> String {
        PAPERS
            .iter()
            .map(|(c, name)| format!("<button class=\"{}\" data-act=\"{}-paper\" data-arg=\"{c}\" title=\"{}\" style=\"background:{c}\"></button>", if c.eq_ignore_ascii_case(&cur.to_hex()) { "active" } else { "" }, prefix, name, c = c))
            .collect()
    }

    fn mark_seg(sel: &str, val: &str) {
        let list = dom().query_selector_all(&format!("{} button", sel)).unwrap();
        for i in 0..list.length() {
            let b: web_sys::Element = list.get(i).unwrap().unchecked_into();
            set_class(&b, "active", b.get_attribute("data-arg").as_deref() == Some(val));
        }
    }

    pub fn open_new_dialog(&mut self) {
        self.close_popovers();
        input_el("new-title").set_value("");
        set_text("new-cover-title", "Notizbuch");
        let covers = super::library::cover_dots("new-cover", false, None);
        el("new-covers").set_inner_html(&covers);
        self.new_from = None;
        self.refresh_new_dialog();
        self.open_modal("modal-new");
        let _ = input_el("new-title").focus();
    }

    fn refresh_new_dialog(&mut self) {
        // Eigene Notizbuchvorlagen
        let templates: Vec<(String, String, u8, u32)> = self.library.iter().filter(|m| m.is_template).map(|m| (m.id.clone(), m.title.clone(), m.cover, m.pages)).collect();
        show("new-from-wrap", true);
        let year = js_sys::Date::new_0().get_full_year() as i32;
        let planner_year = self.new_from.as_deref().and_then(|f| f.strip_prefix("planner:")).and_then(|y| y.parse::<i32>().ok());
        if planner_year.is_none() && self.new_from.as_ref().map(|id| !templates.iter().any(|t| &t.0 == id)).unwrap_or(false) {
            self.new_from = None;
        }
        let mut from = format!(
            "<button class=\"from-item{}\" data-act=\"new-from\" data-arg=\"\"><span class=\"mini blank\"><svg><use href=\"#i-plus\"/></svg></span><span>Leer</span></button>",
            if self.new_from.is_none() { " active" } else { "" }
        );
        for y in [year, year + 1] {
            from.push_str(&format!(
                "<button class=\"from-item{}\" data-act=\"new-from\" data-arg=\"planner:{y}\" title=\"Jahresübersicht, 12 Monate und alle Wochen mit verlinkten Reitern\"><span class=\"mini plan\"><svg><use href=\"#i-calendar\"/></svg></span><span>Jahresplaner {y}</span></button>",
                if planner_year == Some(y) { " active" } else { "" },
                y = y
            ));
        }
        for (id, title, cover, pages) in &templates {
            from.push_str(&format!(
                "<button class=\"from-item{}\" data-act=\"new-from\" data-arg=\"{id}\" title=\"{title} · {pages} Seiten\"><span class=\"mini\" style=\"{css}\"><svg><use href=\"#i-template\"/></svg></span><span>{title}</span><span class=\"del\" data-act=\"from-del\" data-arg=\"{id}\" title=\"Vorlage löschen\"><svg><use href=\"#i-close\"/></svg></span></button>",
                if self.new_from.as_deref() == Some(id.as_str()) { " active" } else { "" },
                id = esc(id),
                title = esc(title),
                pages = pages,
                css = super::library::cover_css(*cover)
            ));
        }
        el("new-from").set_inner_html(&from);
        set_class(&el("new-paper-section"), "section-off", self.new_from.is_some());
        set_class(&el("new-format"), "section-off", self.new_from.is_some() && planner_year.is_none());
        if let Some(y) = planner_year {
            let t = input_el("new-title");
            if t.value().trim().is_empty() || t.value().starts_with("Planer ") {
                t.set_value(&format!("Planer {}", y));
                set_text("new-cover-title", &format!("Planer {}", y));
            }
        }
        let n = &self.new_nb;
        el("new-tpl").set_inner_html(&Self::tpl_tiles("new", n.template.kind));
        el("new-paper").set_inner_html(&Self::paper_dots("new", n.paper));
        Self::mark_seg("#new-sp", &n.template.spacing.to_string());
        Self::mark_seg("#new-format", &n.format.to_string());
        Self::mark_seg("#new-covers", &n.cover.to_string());
        let list = dom().query_selector_all("#new-covers .cover-dot").unwrap();
        for i in 0..list.length() {
            let b: web_sys::Element = list.get(i).unwrap().unchecked_into();
            set_class(&b, "active", b.get_attribute("data-arg").as_deref() == Some(&n.cover.to_string()));
        }
        let (_, a, b) = COVERS[n.cover as usize % COVERS.len()];
        style(&el("new-cover"), "background", &format!("linear-gradient(135deg,{},{})", a, b));
        let (rw, rh) = match n.format {
            1 => (842.0, 595.0),
            2 => (1.0, 1.0),
            3 => (3.0, 4.0),
            _ => (595.0, 842.0),
        };
        if let Ok(Some(cb)) = dom().query_selector("#new-cover-page") {
            cb.unchecked_into::<web_sys::HtmlInputElement>().set_checked(self.new_nb.cover_page);
        }
        el("new-cover-art").set_inner_html(&super::library::cover_art(n.cover, rw, rh));
        set_class(&el("new-cover"), "patterned", super::svg::patterned(n.cover));
        let ratio = match n.format {
            1 => "842 / 595",
            2 => "1 / 1",
            3 => "3 / 4",
            _ => "595 / 842",
        };
        style(&el("new-cover"), "aspect-ratio", ratio);
    }

    fn create_from_dialog(&mut self) {
        let title = input_el("new-title").value();
        let title = if title.trim().is_empty() { "Notizbuch".to_string() } else { title.trim().chars().take(80).collect() };
        let n = &self.new_nb;
        let format = match n.format {
            1 => Format::A4_LANDSCAPE,
            2 => Format::SQUARE,
            3 => {
                let win = window();
                let w = win.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1280.0);
                let h = win.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(800.0);
                let base = 842.0f64;
                if w >= h {
                    Format { w: base as f32, h: (base * h / w) as f32 }
                } else {
                    Format { w: (base * w / h) as f32, h: base as f32 }
                }
            }
            _ => Format::A4_PORTRAIT,
        };
        let (cover, template, paper) = (n.cover, n.template, n.paper);
        let folder = self.folder.clone().unwrap_or_default();
        self.close_modals();
        self.save_settings();
        if let Some(y) = self.new_from.as_deref().and_then(|f| f.strip_prefix("planner:")).and_then(|y| y.parse::<i32>().ok()) {
            let mut nb = crate::engine::planner::year_planner(super::storage::new_id(), &title, cover, format, y, paper, wall_clock());
            nb.folder = folder;
            self.add_notebook(nb, true);
            self.toast("Jahresplaner erstellt – tippe auf Reiter, Monate und Kalenderwochen, um zu springen");
            return;
        }
        if let Some(tid) = self.new_from.clone() {
            self.with_stored(tid, move |a, t| {
                let mut nb = t.copy_as(super::storage::new_id(), &title, wall_clock());
                nb.cover = cover;
                nb.folder = folder;
                a.add_notebook(nb, true);
            });
            return;
        }
        let mut nb = Notebook::new(super::storage::new_id(), &title, cover, format, template, paper, wall_clock());
        nb.folder = folder;
        if self.new_nb.cover_page {
            let p = nb.make_cover_page();
            nb.pages.insert(0, p);
        }
        self.add_notebook(nb, true);
    }

    fn open_template_dialog(&mut self) {
        if let Some(ed) = &self.ed {
            let p = &ed.nb.pages[ed.cur_page];
            self.tpl_choice = p.template;
            self.tpl_paper = p.paper;
        }
        self.refresh_tpl_dialog();
        self.open_modal("modal-tpl");
    }

    fn refresh_tpl_dialog(&mut self) {
        el("tpl-tpl").set_inner_html(&Self::tpl_tiles("tpl", self.tpl_choice.kind));
        el("tpl-paper").set_inner_html(&Self::paper_dots("tpl", self.tpl_paper));
        Self::mark_seg("#tpl-sp", &self.tpl_choice.spacing.to_string());
    }

    fn open_settings(&mut self) {
        self.refresh_settings();
        self.open_modal("modal-settings");
    }

    pub fn refresh_settings(&mut self) {
        let mut v = option_env!("ZT_BUILD").unwrap_or("Entwicklung").to_string();
        if let Some(b) = super::storage::android_bridge() {
            if let Some(n) = js_sys::Reflect::get(&b, &"version".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()).and_then(|f| f.call0(&b).ok()).and_then(|x| x.as_string()) {
                v = format!("{} · App {}", v, n);
            }
        }
        set_text("set-version", &v);
        let s = &self.settings;
        Self::mark_seg("#set-theme", s.theme.code());
        input_el("set-pressure").set_checked(s.pressure);
        input_el("set-quick").set_checked(s.quickshape);
        input_el("set-scroll").set_checked(s.one_finger_scroll);
        input_el("set-finger").set_checked(s.finger_draw);
        input_el("set-smooth").set_value(&format!("{}", (s.smoothing * 100.0).round()));
        set_text("set-smooth-v", &pct(s.smoothing));
        input_el("set-palm").set_value(&format!("{}", (s.palm_lock * 10.0).round()));
        set_text("set-palm-v", &format!("{:.1} s", s.palm_lock));
        let sel: HtmlSelectElement = el("set-button").unchecked_into();
        sel.set_value(s.pen_button.code());
    }

    pub fn refresh_lib_controls(&mut self) {
        Self::mark_seg("#lib-sort", &self.lib_sort.clone());
        set_class(&el("lib-fav"), "active", self.lib_fav);
    }
}

/// Element per CSS-Selektor ein-/ausblenden.
fn show_el_sel(sel: &str, on: bool) {
    if let Ok(Some(e)) = dom().query_selector(sel) {
        set_class(&e, "hidden", !on);
    }
}
