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

const POPS: [&str; 3] = ["pop-tool", "pop-more", "pop-card"];
const MODALS: [&str; 4] = ["modal-new", "modal-tpl", "modal-settings", "modal-cover"];

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
            "import" => set_timeout(0, || input_el("file-import").click()),
            "settings" => self.open_settings(),
            "card" => self.open_notebook(arg.to_string()),
            "card-menu" => {
                self.ctx_target = Some(arg.to_string());
                let fav = self.library.iter().find(|m| m.id == arg).map(|m| m.favorite).unwrap_or(false);
                set_text("card-fav-label", if fav { "Aus Favoriten entfernen" } else { "Zu Favoriten" });
                self.open_popover("pop-card", src);
            }
            a if a.starts_with("card-") || a == "cover-pick" => self.card_action(a, n),

            // ----- Editor-Leiste -----
            "back" => self.close_editor(),
            "nav" => self.toggle_nav(),
            "nav-filter" => {
                if let Some(ed) = &mut self.ed {
                    ed.nav_bookmarks = arg == "bm";
                }
                self.render_nav();
            }
            "nav-page" => {
                if let Some(ed) = &mut self.ed {
                    ed.goto_page(n);
                }
                self.after_view_change();
            }
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
            "sel-copy" | "sel-cut" | "sel-dup" | "sel-delete" | "sel-color" | "paste" => self.selection_action(act, arg),

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
                let ok = window().confirm_with_message("Diese Seite löschen? (Rückgängig ist möglich)").unwrap_or(false);
                if ok {
                    let done = self.ed.as_mut().map(|ed| ed.delete_page(ed.cur_page)).unwrap_or(false);
                    if !done {
                        self.toast("Die letzte Seite kann nicht gelöscht werden");
                    }
                    self.after_pages_change();
                }
            }
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
            "lib-search" => {
                self.lib_query = value.to_lowercase();
                self.render_library();
            }
            "title" => {
                if let Some(ed) = &mut self.ed {
                    let t = value.trim();
                    ed.nb.title = if t.is_empty() { "Ohne Titel".into() } else { t.chars().take(80).collect() };
                    ed.nb.modified = wall_clock();
                }
                if kind == "change" {
                    self.mark_unsaved();
                }
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
        el("favs").set_inner_html(&html);
        show("fav-sep", !html.is_empty());
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
        style(&pop, "top", &format!("{}px", r.bottom() + 10.0));
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
            s.push_str(&format!("<label class=\"sw custom\" title=\"Eigene Farbe\"><input type=\"color\" data-input=\"pop-custom\" value=\"{}\"></label></div>", hex4(cur)));
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
                if tool == Tool::Shape {
                    h.push_str("<p class=\"hint\">Zeichne eine Form – sie wird automatisch zu Linie, Kreis, Ellipse, Dreieck, Rechteck oder Vieleck.</p>");
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
                h.push_str(&toggle("pop-lasso-ink", "Handschrift auswählen", p.lasso_ink));
                h.push_str(&toggle("pop-lasso-text", "Text auswählen", p.lasso_text));
                h.push_str(&toggle("pop-lasso-img", "Bilder auswählen", p.lasso_img));
                h.push_str("<p class=\"hint\">Einkreisen zum Auswählen. Danach ziehen = verschieben, runder Griff = skalieren. Kurz tippen = Einfügen.</p>");
            }
            Tool::Text => {
                h.push_str(&slider(8.0, 48.0, 1.0, p.text_size, "Größe"));
                h.push_str(&toggle("pop-bold", "Fett", p.text_bold));
                h.push_str(&swatches(&PEN_COLORS, p.text_colors[p.text_ci]));
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

    pub fn update_pill(&mut self) {
        let Some(ed) = &self.ed else { return };
        let label = format!("{} / {}", ed.cur_page + 1, ed.nb.pages.len());
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
        let Some(ed) = &mut self.ed else { return };
        ed.nav_open = !ed.nav_open;
        let open = ed.nav_open;
        set_class(&el("navigator"), "open", open);
        set_class(&el("btn-nav"), "active", open);
        if open {
            self.render_nav();
        }
    }

    pub fn render_nav(&mut self) {
        let Some(ed) = &mut self.ed else { return };
        set_class(&el("nav-all"), "active", !ed.nav_bookmarks);
        set_class(&el("nav-bm"), "active", ed.nav_bookmarks);
        let mut html = String::new();
        for (i, p) in ed.nb.pages.iter().enumerate() {
            if ed.nav_bookmarks && !p.bookmark {
                continue;
            }
            html.push_str(&format!(
                "<button class=\"nav-item{}\" data-act=\"nav-page\" data-arg=\"{}\"><canvas data-page=\"{}\"></canvas><span class=\"nav-num\">{}{}</span></button>",
                if i == ed.cur_page { " current" } else { "" },
                i,
                p.id,
                i + 1,
                if p.bookmark { " <svg class=\"bm\"><use href=\"#i-bookmark\"/></svg>" } else { "" }
            ));
        }
        if html.is_empty() {
            html = "<p class=\"muted nav-empty\">Keine Seiten mit Lesezeichen</p>".into();
        }
        html.push_str("<div class=\"nav-actions\"><button class=\"btn small\" data-act=\"page-up\">Seite nach oben</button><button class=\"btn small\" data-act=\"page-down\">Seite nach unten</button></div>");
        el("nav-list").set_inner_html(&html);
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
        for m in MODALS {
            show(m, false);
        }
    }

    fn tpl_tiles(prefix: &str, cur: TemplateKind) -> String {
        TemplateKind::ALL
            .iter()
            .map(|k| {
                format!(
                    "<button class=\"tpl{}\" data-act=\"{}-tpl\" data-arg=\"{}\"><span class=\"tpl-prev tp-{}\"></span>{}</button>",
                    if *k == cur { " active" } else { "" },
                    prefix,
                    k.code(),
                    k.code(),
                    k.name_de()
                )
            })
            .collect()
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
        let covers: String = COVERS
            .iter()
            .enumerate()
            .map(|(i, (name, a, b))| format!("<button class=\"cover-dot\" data-act=\"new-cover\" data-arg=\"{}\" title=\"{}\" style=\"background:linear-gradient(135deg,{},{})\"></button>", i, name, a, b))
            .collect();
        el("new-covers").set_inner_html(&covers);
        self.refresh_new_dialog();
        self.open_modal("modal-new");
        let _ = input_el("new-title").focus();
    }

    fn refresh_new_dialog(&mut self) {
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
        let nb = Notebook::new(super::storage::new_id(), &title, n.cover, format, n.template, n.paper, wall_clock());
        self.close_modals();
        self.save_settings();
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
