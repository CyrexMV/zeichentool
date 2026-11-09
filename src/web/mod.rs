//! Browser-Anbindung: Bibliothek, Editor, S-Pen/Touch-Eingabe, Rendering, Speicherung.

mod colorpick;
mod docs;
mod editor;
mod input;
mod insert;
mod library;
mod links;
mod marks;
mod mind;
mod lock;
mod pensets;
mod render;
mod ruler;
mod scrubber;
mod stats;
mod study;
mod storage;
mod svg;
mod tabs;
mod tools;
mod transform;
mod zoomwin;
mod ui;

use crate::engine::format::NoteMeta;
use crate::engine::{Color, PenKind, Template, TemplateKind};
use std::cell::RefCell;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{AddEventListenerOptions, Event, EventTarget, HtmlElement, HtmlImageElement, HtmlInputElement};

pub use editor::Editor;
pub use input::{Gesture, RulerGrab, TouchPt};

// ---------------------------------------------------------------------------
// Globale App und DOM-Helfer
// ---------------------------------------------------------------------------

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    static RAF: Closure<dyn FnMut(f64)> = Closure::new(|t: f64| {
        with_app(|a| a.frame(t));
    });
}

/// Führt `f` mit der App aus (`None`, falls sie gerade beschäftigt ist).
pub fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|cell| match cell.try_borrow_mut() {
        Ok(mut b) => b.as_mut().map(f),
        Err(_) => None,
    })
}

pub fn window() -> web_sys::Window {
    web_sys::window().expect("kein window")
}

pub fn dom() -> web_sys::Document {
    window().document().expect("kein document")
}

pub fn el(id: &str) -> HtmlElement {
    dom().get_element_by_id(id).unwrap_or_else(|| panic!("Element #{} fehlt", id)).unchecked_into()
}

pub fn input_el(id: &str) -> HtmlInputElement {
    el(id).unchecked_into()
}

pub fn now() -> f64 {
    window().performance().map(|p| p.now()).unwrap_or(0.0)
}

pub fn wall_clock() -> f64 {
    js_sys::Date::now()
}

pub fn on<T: JsCast + 'static>(target: &EventTarget, ev: &str, mut f: impl FnMut(T) + 'static) {
    let c = Closure::<dyn FnMut(Event)>::new(move |e: Event| f(e.unchecked_into::<T>()));
    let opts = AddEventListenerOptions::new();
    opts.set_passive(false);
    let _ = target.add_event_listener_with_callback_and_add_event_listener_options(ev, c.as_ref().unchecked_ref(), &opts);
    c.forget();
}

pub fn set_timeout(ms: i32, f: impl FnOnce() + 'static) {
    let c = Closure::once_into_js(f);
    let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(c.unchecked_ref(), ms);
}

pub fn set_class(e: &web_sys::Element, class: &str, on: bool) {
    let _ = e.class_list().toggle_with_force(class, on);
}

pub fn show(id: &str, visible: bool) {
    set_class(&el(id), "hidden", !visible);
}

pub fn style(e: &HtmlElement, prop: &str, value: &str) {
    let _ = e.style().set_property(prop, value);
}

pub fn set_text(id: &str, t: &str) {
    if let Some(e) = dom().get_element_by_id(id) {
        e.set_text_content(Some(t));
    }
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn hex4(c: [u8; 4]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

pub fn rgba_css(c: [u8; 4]) -> String {
    format!("rgba({},{},{},{})", c[0], c[1], c[2], c[3] as f32 / 255.0)
}

pub fn color4(c: Color, a: u8) -> [u8; 4] {
    let [r, g, b] = c.to_u8();
    [r, g, b, a]
}

pub fn vibrate(ms: u32) {
    let _ = window().navigator().vibrate_with_duration(ms);
}

// ---------------------------------------------------------------------------
// Werkzeuge und Einstellungen
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Pen,
    Highlighter,
    Eraser,
    Lasso,
    Shape,
    Text,
    Laser,
    Mind,
}

impl Tool {
    pub const ALL: [Tool; 8] = [Tool::Pen, Tool::Highlighter, Tool::Eraser, Tool::Lasso, Tool::Shape, Tool::Text, Tool::Laser, Tool::Mind];

    pub fn code(&self) -> &'static str {
        match self {
            Tool::Pen => "pen",
            Tool::Highlighter => "highlighter",
            Tool::Eraser => "eraser",
            Tool::Lasso => "lasso",
            Tool::Shape => "shape",
            Tool::Text => "text",
            Tool::Laser => "laser",
            Tool::Mind => "mind",
        }
    }

    pub fn from_code(s: &str) -> Option<Tool> {
        Tool::ALL.iter().copied().find(|t| t.code() == s)
    }

    pub fn name_de(&self) -> &'static str {
        match self {
            Tool::Pen => "Stift",
            Tool::Highlighter => "Textmarker",
            Tool::Eraser => "Radierer",
            Tool::Lasso => "Lasso",
            Tool::Shape => "Formen",
            Tool::Text => "Text",
            Tool::Laser => "Laserpointer",
            Tool::Mind => "Mindmap",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PenButton {
    /// Drücken = Radierer wählen, nochmal = zurück
    Select,
    /// Beim Schreiben halten = radieren
    Hold,
    /// Drücken = Rückgängig
    Undo,
}

impl PenButton {
    pub fn code(&self) -> &'static str {
        match self {
            PenButton::Select => "select",
            PenButton::Hold => "hold",
            PenButton::Undo => "undo",
        }
    }
    pub fn from_code(s: &str) -> PenButton {
        match s {
            "hold" => PenButton::Hold,
            "undo" => PenButton::Undo,
            _ => PenButton::Select,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    System,
    Light,
    Dark,
}

impl Theme {
    pub fn code(&self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
    pub fn from_code(s: &str) -> Theme {
        match s {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => Theme::System,
        }
    }
}

pub struct Settings {
    pub theme: Theme,
    pub pressure: bool,
    pub smoothing: f32,
    pub quickshape: bool,
    pub pen_button: PenButton,
    pub palm_lock: f32,
    pub one_finger_scroll: bool,
    pub finger_draw: bool,
    pub pen_seen: bool,
    pub migrated: bool,
    /// Wiederholungsplan für Lernkarten (fällige Karten) – freiwillig
    pub due_mode: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: Theme::System,
            pressure: true,
            smoothing: 0.35,
            quickshape: true,
            pen_button: PenButton::Select,
            palm_lock: 2.0,
            one_finger_scroll: true,
            finger_draw: false,
            pen_seen: false,
            migrated: false,
            due_mode: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EraserMode {
    Precise,
    Stroke,
}

/// Werkzeug-Einstellungen mit je drei Favoriten-Farben und -Stärken (wie in GoodNotes).
pub struct Prefs {
    pub pen_kind: PenKind,
    pub pen_colors: [[u8; 4]; 3],
    pub pen_sizes: [f32; 3],
    pub pen_ci: usize,
    pub pen_si: usize,
    pub hl_colors: [[u8; 4]; 3],
    pub hl_sizes: [f32; 3],
    pub hl_ci: usize,
    pub hl_si: usize,
    pub hl_straight: bool,
    pub er_sizes: [f32; 3],
    pub er_si: usize,
    pub er_mode: EraserMode,
    pub er_hl_only: bool,
    pub er_auto_back: bool,
    pub text_colors: [[u8; 4]; 3],
    pub text_ci: usize,
    pub text_size: f32,
    pub text_bold: bool,
    pub lasso_ink: bool,
    pub lasso_text: bool,
    pub lasso_img: bool,
    pub laser_color: [u8; 4],
    /// Formen füllen: 0 = aus, 1 = hell, 2 = voll
    pub shape_fill: u8,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            pen_kind: PenKind::Fountain,
            pen_colors: [[24, 24, 32, 255], [30, 90, 230, 255], [225, 45, 60, 255]],
            pen_sizes: [1.2, 2.0, 3.4],
            pen_ci: 0,
            pen_si: 1,
            hl_colors: [[255, 221, 0, 90], [120, 230, 120, 90], [255, 120, 200, 90]],
            hl_sizes: [8.0, 13.0, 20.0],
            hl_ci: 0,
            hl_si: 1,
            hl_straight: false,
            er_sizes: [4.0, 10.0, 24.0],
            er_si: 1,
            er_mode: EraserMode::Precise,
            er_hl_only: false,
            er_auto_back: false,
            text_colors: [[24, 24, 32, 255], [30, 90, 230, 255], [225, 45, 60, 255]],
            text_ci: 0,
            text_size: 16.0,
            text_bold: false,
            lasso_ink: true,
            lasso_text: true,
            lasso_img: true,
            laser_color: [255, 50, 60, 255],
            shape_fill: 0,
        }
    }
}

/// Auswahl im Dialog „Neues Notizbuch“.
pub struct NewNb {
    pub cover: u8,
    pub template: Template,
    pub paper: Color,
    pub format: u8,
    /// Deckblatt mit dem Cover als erste Seite
    pub cover_page: bool,
}

impl Default for NewNb {
    fn default() -> Self {
        NewNb { cover: 0, template: Template::new(TemplateKind::Lined, 1), paper: Color::WHITE, format: 0, cover_page: true }
    }
}

/// Zoom-Rahmen ziehen: (Zeiger, Start x, Start y, (Rahmen x, y, Seite)).
pub type ZwDrag = (i32, f64, f64, (f32, f32, u32));

/// Aktion, die auf die Bestätigung im Dialog wartet.
#[derive(Clone, Debug, PartialEq)]
pub enum Pending {
    DeleteNotebook(String),
    RenameNotebook(String),
    DeletePage(usize),
    NewFolder,
    RenameFolder(String),
    DeleteFolder(String),
    PageTitle(usize),
    /// Notizbuch (ID) oder – bei `None` – das geöffnete Notizbuch als Vorlage speichern
    SaveTemplate(Option<String>),
    DeleteTemplate(String),
    MoveToFolder(String),
    PageTo(usize),
    NewDeck,
    /// Komplett-Backup einspielen (Daten in `App::pending_backup`)
    RestoreBackup,
    /// Link setzen: Seite + Rechteck (x0, y0, x1, y1)
    Link(u32, f32, f32, f32, f32),
    /// Text einer Mindmap-Blase (Seite, ID)
    NodeText(u32, u32),
    Unlock(String, lock::Then),
    SetPin(String),
    ConfirmPin(String, String),
    RemovePin(String),
    DeleteForever(String),
    EmptyTrash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Library,
    Editor,
}

#[derive(Clone, Copy, Debug)]
pub struct Momentum {
    pub vx: f64,
    pub vy: f64,
    pub t: f64,
}

pub struct App {
    pub screen: Screen,
    pub library: Vec<NoteMeta>,
    pub lib_query: String,
    pub lib_sort: String,
    pub lib_fav: bool,
    pub db: Option<web_sys::IdbDatabase>,
    pub settings: Settings,
    pub prefs: Prefs,
    pub tool: Tool,
    pub prev_tool: Tool,
    pub hold_eraser: bool,
    pub ed: Option<Editor>,
    pub images: HashMap<u64, HtmlImageElement>,
    pub new_nb: NewNb,
    pub ctx_target: Option<String>,
    pub open_pop: Option<&'static str>,
    pub tpl_choice: Template,
    pub tpl_paper: Color,

    // Eingabe
    pub touches: Vec<TouchPt>,
    pub gesture: Option<Gesture>,
    pub momentum: Option<Momentum>,
    pub pen_in_range: bool,
    pub last_pen_time: f64,
    /// Letzter Ein-Finger-Tipp (Zeit, x, y) für Doppeltipp zum Einpassen.
    pub last_tap: Option<(f64, f64, f64)>,
    pub pen_barrel: bool,
    pub last_barrel_press: f64,
    pub badge_on: bool,
    pub native_near: bool,
    pub native_button: bool,
    pub mouse_pan: Option<(i32, f64, f64)>,
    pub space_down: bool,

    raf_pending: bool,
    pub toast_seq: u32,
    pub unsaved: bool,
    pub last_change: f64,
    pub dark: bool,
    pub last_ui_rects: String,
    pub pending: Option<Pending>,
    pub ruler_grab: Option<RulerGrab>,
    /// Lernkarten-Sitzung
    pub study: Option<study::Session>,
    /// Ordner, der in der Bibliothek gerade geöffnet ist
    pub folder: Option<String>,
    /// Alle Ordnernamen
    pub folders: Vec<String>,
    /// Ausgewählte Notizbuchvorlage im Neu-Dialog
    pub new_from: Option<String>,
    /// Modus im Auswahldialog: Seite kopieren statt verschieben
    pub pick_copy: bool,
    /// Pipette aktiv: nächster Tipp auf die Seite übernimmt die Farbe
    pub pipette: bool,
    /// Eigenes Papier für alle Seiten (sonst nur aktuelle Seite)
    pub paper_all: bool,
    /// In dieser Sitzung entsperrte Notizbücher
    pub unlocked: Vec<String>,
    /// Bibliothek zeigt den Papierkorb
    pub show_trash: bool,
    /// Lernstatistik je Tag
    pub stats: Vec<stats::Day>,
    /// Zoom-Rahmen wird mit dem Finger gezogen: (Zeiger, Start x, Start y, (Rahmen x, y, Seite))
    pub zw_drag: Option<ZwDrag>,
    /// Farbwähler: Farbton, Sättigung, Helligkeit + Ausgangsfarbe
    pub cp: (f32, f32, f32),
    pub cp_old: Color,
    /// zuletzt gewählte eigene Farben
    pub recent_colors: Vec<String>,
    /// Tabellen-Dialog
    pub table_opts: insert::TableOpts,
    /// ausgewählte Mindmap-Blase (Seite, ID)
    pub mind_sel: Option<(u32, u32)>,
    /// geöffnete Notizbücher als Tabs
    pub tabs: Vec<tabs::Tab>,
    /// Präsentationsmodus: (vorheriges Werkzeug, vorherige Ansicht)
    pub present: Option<(Tool, (f64, f64, f64))>,
    /// gewähltes Backup, wartet auf Bestätigung
    pub pending_backup: Option<Vec<u8>>,
    /// gespeicherte Stift-Sets
    pub pen_sets: Vec<pensets::PenSet>,
    /// Seiten-Schieberegler wird gezogen
    pub scrub_active: bool,
    /// Lernmodus wurde vom Startbildschirm aus geöffnet (Schließen → zurück zur Bibliothek)
    pub study_from_lib: bool,
}

impl App {
    fn new() -> App {
        App {
            screen: Screen::Library,
            library: Vec::new(),
            lib_query: String::new(),
            lib_sort: "modified".into(),
            lib_fav: false,
            db: None,
            settings: Settings::default(),
            prefs: Prefs::default(),
            tool: Tool::Pen,
            prev_tool: Tool::Pen,
            hold_eraser: false,
            ed: None,
            images: HashMap::new(),
            new_nb: NewNb::default(),
            ctx_target: None,
            open_pop: None,
            tpl_choice: Template::default(),
            tpl_paper: Color::WHITE,
            touches: Vec::new(),
            gesture: None,
            momentum: None,
            pen_in_range: false,
            last_pen_time: -1e9,
            last_tap: None,
            pen_barrel: false,
            last_barrel_press: -1e9,
            badge_on: false,
            native_near: false,
            native_button: false,
            mouse_pan: None,
            space_down: false,
            raf_pending: false,
            toast_seq: 0,
            unsaved: false,
            last_change: 0.0,
            dark: false,
            last_ui_rects: String::new(),
            pending: None,
            ruler_grab: None,
            study: None,
            folder: None,
            folders: Vec::new(),
            new_from: None,
            pick_copy: true,
            pipette: false,
            paper_all: false,
            unlocked: Vec::new(),
            show_trash: false,
            stats: Vec::new(),
            zw_drag: None,
            cp: (0.0, 0.0, 0.0),
            cp_old: Color::WHITE,
            recent_colors: Vec::new(),
            table_opts: insert::TableOpts::default(),
            mind_sel: None,
            tabs: Vec::new(),
            present: None,
            pending_backup: None,
            pen_sets: Vec::new(),
            scrub_active: false,
            study_from_lib: false,
        }
    }

    pub fn request_frame(&mut self) {
        if !self.raf_pending {
            self.raf_pending = true;
            RAF.with(|cb| {
                let _ = window().request_animation_frame(cb.as_ref().unchecked_ref());
            });
        }
    }

    /// Editor neu zeichnen lassen.
    pub fn redraw(&mut self) {
        if let Some(ed) = &mut self.ed {
            ed.dirty = true;
        }
        self.request_frame();
    }

    fn frame(&mut self, _t: f64) {
        self.raf_pending = false;
        let t = now();
        let mut again = self.step_momentum(t);
        self.check_quickshape();
        let dark = self.dark;
        let lib_needed = self.screen == Screen::Editor;
        let mut rendered = false;
        let mind_hl = if self.tool == Tool::Mind { self.mind_sel } else { None };
        if let (Some(ed), true) = (&mut self.ed, lib_needed) {
            if ed.mind_hl != mind_hl {
                ed.mind_hl = mind_hl;
                ed.dirty = true;
            }
            if ed.dirty {
                ed.render(&mut self.images, dark);
                rendered = true;
            }
            if !ed.laser.is_empty() {
                ed.draw_laser(t, self.prefs.laser_color);
                again = true;
            }
            if ed.op.is_some() {
                again = true;
            }
            if ed.zoomwin.is_some() && (rendered || ed.op.is_some()) {
                ed.render_zw(&mut self.images, dark);
            }
            if ed.nav_open && ed.thumbs_dirty && ed.op.is_none() {
                ed.render_thumbs(&mut self.images, dark);
            }
        }
        if rendered {
            self.position_overlays();
            self.update_pill();
        }
        self.sync_ui_rects();
        if again {
            self.request_frame();
        }
    }

    pub fn toast(&mut self, msg: &str) {
        let t = el("toast");
        t.set_text_content(Some(msg));
        set_class(&t, "show", true);
        self.toast_seq += 1;
        let seq = self.toast_seq;
        set_timeout(1700, move || {
            with_app(|a| {
                if a.toast_seq == seq {
                    set_class(&el("toast"), "show", false);
                }
            });
        });
    }

    /// Farbschema anwenden (Hell/Dunkel/System).
    pub fn apply_theme(&mut self) {
        let dark = match self.settings.theme {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => window().match_media("(prefers-color-scheme: dark)").ok().flatten().map(|m| m.matches()).unwrap_or(false),
        };
        self.dark = dark;
        if let Some(root) = dom().document_element() {
            let _ = root.set_attribute("data-theme", if dark { "dark" } else { "light" });
        }
        let meta = dom().query_selector("meta[name=theme-color]").ok().flatten();
        if let Some(m) = meta {
            let _ = m.set_attribute("content", if dark { "#121318" } else { "#f3f4f8" });
        }
        self.redraw();
    }
}

// ---------------------------------------------------------------------------
// Start
// ---------------------------------------------------------------------------

#[wasm_bindgen(start)]
pub fn start() {
    let mut app = App::new();
    app.load_settings();
    APP.with(|cell| *cell.borrow_mut() = Some(app));
    ui::bind_ui();
    input::bind_input();
    colorpick::bind_color_picker();
    scrubber::bind_scrubber();
    with_app(|a| {
        a.apply_theme();
        a.refresh_toolbar();
        a.sync_native_settings();
    });
    storage::init_storage();
    storage::register_service_worker();
    // Systemfarbschema beobachten
    if let Ok(Some(mq)) = window().match_media("(prefers-color-scheme: dark)") {
        on(&mq, "change", |_: Event| {
            with_app(|a| a.apply_theme());
        });
    }
}
