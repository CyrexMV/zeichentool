//! Browser-Anbindung: Canvas-Darstellung, S-Pen/Touch-Eingabe, Oberfläche, Speicherung.

mod input;
mod storage;
mod tools;
mod ui;

use crate::engine::project::PackCache;
use crate::engine::{presets, BrushPreset, Color, Document as Doc, History, IRect, Mask, Noise, Overlay, Symmetry, Template};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{Clamped, JsCast};
use web_sys::{AddEventListenerOptions, CanvasRenderingContext2d, Event, EventTarget, HtmlCanvasElement, HtmlElement, HtmlInputElement, ImageData};

pub use input::{Gesture, TouchPt};
pub use tools::{Stroke, StrokeKind};

// ---------------------------------------------------------------------------
// Globale App-Instanz und DOM-Helfer
// ---------------------------------------------------------------------------

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    static RAF: Closure<dyn FnMut(f64)> = Closure::new(|t: f64| {
        with_app(|a| a.frame(t));
    });
}

/// Führt `f` mit der App aus. Gibt `None` zurück, falls die App gerade beschäftigt ist.
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
    dom()
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("Element #{} fehlt", id))
        .unchecked_into()
}

pub fn input_el(id: &str) -> HtmlInputElement {
    el(id).unchecked_into()
}

pub fn now() -> f64 {
    window().performance().map(|p| p.now()).unwrap_or(0.0)
}

pub fn on<T: JsCast + 'static>(target: &EventTarget, ev: &str, mut f: impl FnMut(T) + 'static) {
    let c = Closure::<dyn FnMut(Event)>::new(move |e: Event| f(e.unchecked_into::<T>()));
    let opts = AddEventListenerOptions::new();
    opts.set_passive(false);
    let _ = target.add_event_listener_with_callback_and_add_event_listener_options(ev, c.as_ref().unchecked_ref(), &opts);
    c.forget();
}

/// Klick-Handler, der direkt die App bekommt.
pub fn on_click(id: &str, f: impl Fn(&mut App) + 'static) {
    on(&el(id), "click", move |_: Event| {
        with_app(|a| f(a));
    });
}

pub fn set_timeout(ms: i32, f: impl FnOnce() + 'static) {
    let c = Closure::once_into_js(f);
    let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(c.unchecked_ref(), ms);
}

pub fn set_class(e: &web_sys::Element, class: &str, on: bool) {
    let _ = e.class_list().toggle_with_force(class, on);
}

pub fn style(e: &HtmlElement, prop: &str, value: &str) {
    let _ = e.style().set_property(prop, value);
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// Ansicht (Zoom, Drehung, Verschiebung)
// ---------------------------------------------------------------------------

/// Affine Abbildung Dokument → Bildschirm (CSS-Pixel).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Affine {
    pub const IDENTITY: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    pub fn translate(x: f64, y: f64) -> Affine {
        Affine { e: x, f: y, ..Affine::IDENTITY }
    }

    pub fn scale(s: f64) -> Affine {
        Affine { a: s, d: s, ..Affine::IDENTITY }
    }

    pub fn rotate(t: f64) -> Affine {
        let (s, c) = t.sin_cos();
        Affine { a: c, b: s, c: -s, d: c, e: 0.0, f: 0.0 }
    }

    /// `self ∘ o` – zuerst `o`, dann `self`.
    pub fn mul(&self, o: &Affine) -> Affine {
        Affine {
            a: self.a * o.a + self.c * o.b,
            b: self.b * o.a + self.d * o.b,
            c: self.a * o.c + self.c * o.d,
            d: self.b * o.c + self.d * o.d,
            e: self.a * o.e + self.c * o.f + self.e,
            f: self.b * o.e + self.d * o.f + self.f,
        }
    }

    pub fn det(&self) -> f64 {
        self.a * self.d - self.b * self.c
    }

    pub fn invert(&self) -> Affine {
        let det = self.det();
        let id = if det.abs() < 1e-12 { 0.0 } else { 1.0 / det };
        Affine {
            a: self.d * id,
            b: -self.b * id,
            c: -self.c * id,
            d: self.a * id,
            e: (self.c * self.f - self.d * self.e) * id,
            f: (self.b * self.e - self.a * self.f) * id,
        }
    }

    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }

    pub fn apply_vec(&self, x: f64, y: f64) -> (f64, f64) {
        (self.a * x + self.c * y, self.b * x + self.d * y)
    }

    pub fn zoom(&self) -> f64 {
        self.det().abs().sqrt()
    }

    pub fn angle(&self) -> f64 {
        self.b.atan2(self.a)
    }

    pub fn css(&self) -> String {
        format!("matrix({},{},{},{},{},{})", self.a, self.b, self.c, self.d, self.e, self.f)
    }
}

// ---------------------------------------------------------------------------
// Werkzeuge und Einstellungen
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Brush,
    Smudge,
    Eraser,
    Fill,
    Picker,
    Shape,
    Move,
}

impl Tool {
    pub fn code(&self) -> &'static str {
        match self {
            Tool::Brush => "brush",
            Tool::Smudge => "smudge",
            Tool::Eraser => "eraser",
            Tool::Fill => "fill",
            Tool::Picker => "picker",
            Tool::Shape => "shape",
            Tool::Move => "move",
        }
    }

    pub fn from_code(s: &str) -> Option<Tool> {
        Some(match s {
            "brush" => Tool::Brush,
            "smudge" => Tool::Smudge,
            "eraser" => Tool::Eraser,
            "fill" => Tool::Fill,
            "picker" => Tool::Picker,
            "shape" => Tool::Shape,
            "move" => Tool::Move,
            _ => return None,
        })
    }

    pub fn name_de(&self) -> &'static str {
        match self {
            Tool::Brush => "Pinsel",
            Tool::Smudge => "Wischfinger",
            Tool::Eraser => "Radierer",
            Tool::Fill => "Füllen",
            Tool::Picker => "Pipette",
            Tool::Shape => "Formen",
            Tool::Move => "Verschieben",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PenButton {
    /// Taste drücken = Radierer auswählen, nochmal drücken = zurück
    Select,
    /// Taste beim Zeichnen gedrückt halten = radieren
    Hold,
    Picker,
    Undo,
}

impl PenButton {
    pub fn code(&self) -> &'static str {
        match self {
            PenButton::Select => "select",
            PenButton::Hold => "hold",
            PenButton::Picker => "picker",
            PenButton::Undo => "undo",
        }
    }

    pub fn from_code(s: &str) -> PenButton {
        match s {
            "hold" => PenButton::Hold,
            "picker" => PenButton::Picker,
            "undo" => PenButton::Undo,
            // ältere Einstellungen („eraser“, „toggle“) → neues Standardverhalten
            _ => PenButton::Select,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Line,
    Rect,
    Ellipse,
    Lasso,
}

impl ShapeKind {
    pub fn code(&self) -> &'static str {
        match self {
            ShapeKind::Line => "line",
            ShapeKind::Rect => "rect",
            ShapeKind::Ellipse => "ellipse",
            ShapeKind::Lasso => "lasso",
        }
    }

    pub fn from_code(s: &str) -> ShapeKind {
        match s {
            "rect" => ShapeKind::Rect,
            "ellipse" => ShapeKind::Ellipse,
            "lasso" => ShapeKind::Lasso,
            _ => ShapeKind::Line,
        }
    }
}

pub struct Settings {
    pub smoothing: f32,
    pub gamma: f32,
    pub pen_only: bool,
    pub pen_button: PenButton,
    /// Sekunden, die Finger nach dem Stift gesperrt bleiben (Handballen)
    pub palm_lock: f32,
    pub one_finger_pan: bool,
    pub ignore_large: bool,
    pub quickshape: bool,
    pub hover_cursor: bool,
    pub side_right: bool,
    pub symmetry: Symmetry,
    pub radial_n: u8,
    pub fill_tol: f32,
    pub fill_all: bool,
    pub shape: ShapeKind,
    pub shape_fill: bool,
    /// Zuletzt gewählte Vorlage (Vorgabe für neue Leinwände)
    pub last_template: Template,
    pub last_paper: Option<Color>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            smoothing: 0.25,
            gamma: 1.0,
            pen_only: true,
            pen_button: PenButton::Select,
            palm_lock: 2.0,
            one_finger_pan: false,
            ignore_large: true,
            quickshape: true,
            hover_cursor: true,
            side_right: false,
            symmetry: Symmetry::Off,
            radial_n: 6,
            fill_tol: 0.12,
            fill_all: true,
            shape: ShapeKind::Line,
            shape_fill: false,
            last_template: Template::default(),
            last_paper: Some(Color::WHITE),
        }
    }
}

/// Pinselwahl + Größe + Deckkraft eines Werkzeugs.
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub preset: usize,
    pub size: f32,
    pub opacity: f32,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct App {
    pub doc: Doc,
    pub history: History,
    pub mask: Mask,
    pub noise: Noise,
    pub presets: Vec<BrushPreset>,
    pub base_presets: Vec<BrushPreset>,
    /// Gemerkte Größe/Deckkraft je Pinsel (für das Pinselwerkzeug)
    pub preset_mem: Vec<(f32, f32)>,
    pub tool: Tool,
    pub brush: Slot,
    pub eraser: Slot,
    pub smudge: Slot,
    pub color: Color,
    pub hsv: (f32, f32, f32),
    pub prev_color: Color,
    pub recent: Vec<Color>,
    pub settings: Settings,
    pub view: Affine,
    pub view_fitted: bool,

    pub stroke: Option<Stroke>,
    pub dabs: Vec<crate::engine::Dab>,
    pub smudge_snapshot: Vec<u8>,
    pub touches: Vec<TouchPt>,
    pub gesture: Option<Gesture>,
    pub pen_seen: bool,
    pub last_pen_time: f64,
    pub pen_in_range: bool,
    pub pen_barrel: bool,
    pub last_barrel_press: f64,
    pub prev_tool: Tool,
    pub badge_on: bool,
    pub space_down: bool,
    pub mouse_pan: Option<(i32, f64, f64, Affine)>,

    pub canvas: HtmlCanvasElement,
    pub ctx: CanvasRenderingContext2d,
    pub stage: HtmlElement,
    pub doc_wrap: HtmlElement,
    pub cursor: HtmlElement,
    pub dirty: IRect,
    pub full_redraw: bool,
    raf_pending: bool,
    comp: Vec<u8>,

    pub unsaved: bool,
    pub last_change: f64,
    pub pack_cache: PackCache,
    pub db: Option<web_sys::IdbDatabase>,
    pub open_panel: Option<&'static str>,
    pub ui_layers_dirty: bool,
    pub last_ui_refresh: f64,
    pub toast_seq: u32,
    pub props_before: Option<crate::engine::LayerProps>,
    /// Auswahl im Dialog „Neue Leinwand“
    pub new_tpl: Template,
    pub new_paper: Option<Color>,
}

impl App {
    fn new() -> App {
        let win = window();
        let dpr = win.device_pixel_ratio().max(1.0);
        let iw = win.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1280.0);
        let ih = win.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(800.0);
        let (w, h) = (((iw * dpr).round() as usize).clamp(256, 4096), ((ih * dpr).round() as usize).clamp(256, 4096));

        let canvas: HtmlCanvasElement = el("doc").unchecked_into();
        let opts = js_sys::Object::new();
        // Niedrige Latenz für den S-Pen
        let _ = js_sys::Reflect::set(&opts, &"desynchronized".into(), &true.into());
        let _ = js_sys::Reflect::set(&opts, &"alpha".into(), &true.into());
        let ctx: CanvasRenderingContext2d = canvas
            .get_context_with_context_options("2d", &opts)
            .ok()
            .flatten()
            .expect("Canvas 2D nicht verfügbar")
            .unchecked_into();

        let p = presets();
        let mem = p.iter().map(|b| (b.default_size, b.default_opacity)).collect();
        let idx = |id: &str| p.iter().position(|b| b.id == id).unwrap_or(0);
        let brush = Slot { preset: idx("tusche"), size: p[idx("tusche")].default_size, opacity: 1.0 };
        let eraser = Slot { preset: idx("weich"), size: 60.0, opacity: 1.0 };
        let smudge = Slot { preset: idx("weich"), size: 50.0, opacity: 0.6 };
        let color = Color::from_hex("#1d1d24").unwrap();

        App {
            doc: Doc::new(w, h),
            history: History::default(),
            mask: Mask::new(w, h),
            noise: Noise::new(),
            base_presets: p.clone(),
            presets: p,
            preset_mem: mem,
            tool: Tool::Brush,
            brush,
            eraser,
            smudge,
            color,
            hsv: color.to_hsv(),
            prev_color: color,
            recent: Vec::new(),
            settings: Settings::default(),
            view: Affine::IDENTITY,
            view_fitted: true,
            stroke: None,
            dabs: Vec::new(),
            smudge_snapshot: Vec::new(),
            touches: Vec::new(),
            gesture: None,
            pen_seen: false,
            last_pen_time: -1e9,
            pen_in_range: false,
            pen_barrel: false,
            last_barrel_press: -1e9,
            prev_tool: Tool::Brush,
            badge_on: false,
            space_down: false,
            mouse_pan: None,
            canvas,
            ctx,
            stage: el("stage"),
            doc_wrap: el("doc-wrap"),
            cursor: el("cursor"),
            dirty: IRect::EMPTY,
            full_redraw: true,
            raf_pending: false,
            comp: Vec::new(),
            unsaved: false,
            last_change: 0.0,
            pack_cache: PackCache::default(),
            db: None,
            open_panel: None,
            ui_layers_dirty: true,
            last_ui_refresh: 0.0,
            toast_seq: 0,
            props_before: None,
            new_tpl: Template::default(),
            new_paper: Some(Color::WHITE),
        }
    }

    // ----- Darstellung -----

    pub fn request_frame(&mut self) {
        if !self.raf_pending {
            self.raf_pending = true;
            RAF.with(|cb| {
                let _ = window().request_animation_frame(cb.as_ref().unchecked_ref());
            });
        }
    }

    pub fn invalidate(&mut self, r: IRect) {
        if !r.is_empty() {
            self.dirty = self.dirty.union(&r);
            self.request_frame();
        }
    }

    pub fn redraw_all(&mut self) {
        self.full_redraw = true;
        self.request_frame();
    }

    fn frame(&mut self, _t: f64) {
        self.raf_pending = false;
        self.check_quickshape();
        self.render();
        if self.stroke.is_some() {
            self.request_frame();
        }
        if self.ui_layers_dirty && self.open_panel == Some("panel-layers") && self.stroke.is_none() && now() - self.last_ui_refresh > 120.0 {
            self.render_layers();
        }
    }

    fn overlay(&self) -> Overlay<'_> {
        match &self.stroke {
            Some(s) => match s.kind {
                StrokeKind::Paint | StrokeKind::Shape | StrokeKind::Lasso => Overlay::Paint { layer: s.layer, mask: &self.mask, color: s.color, opacity: s.opacity },
                StrokeKind::Erase => Overlay::Erase { layer: s.layer, mask: &self.mask, opacity: s.opacity },
                StrokeKind::Move => {
                    let (dx, dy) = s.move_offset();
                    Overlay::Offset { layer: s.layer, dx, dy }
                }
                _ => Overlay::None,
            },
            None => Overlay::None,
        }
    }

    fn render(&mut self) {
        let (w, h) = (self.doc.w, self.doc.h);
        let rects: Vec<IRect> = if self.full_redraw {
            (0..h).step_by(128).map(|y| IRect::new(0, y as i32, w as i32, (y + 128).min(h) as i32)).collect()
        } else if self.dirty.is_empty() {
            Vec::new()
        } else {
            vec![self.dirty.clamp(w, h)]
        };
        self.full_redraw = false;
        self.dirty = IRect::EMPTY;
        let mut buf = std::mem::take(&mut self.comp);
        for r in rects {
            if r.is_empty() {
                continue;
            }
            {
                let ov = self.overlay();
                self.doc.composite(r, &ov, true, &mut buf);
            }
            if let Ok(img) = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&buf[..]), r.width() as u32, r.height() as u32) {
                let _ = self.ctx.put_image_data(&img, r.x0 as f64, r.y0 as f64);
            }
        }
        // großen Puffer nicht dauerhaft behalten
        if buf.capacity() > 8 << 20 {
            buf = Vec::new();
        }
        self.comp = buf;
    }

    /// Leinwand-Element an die Dokumentgröße anpassen.
    pub fn setup_canvas(&mut self) {
        let (w, h) = (self.doc.w, self.doc.h);
        self.canvas.set_width(w as u32);
        self.canvas.set_height(h as u32);
        style(&self.doc_wrap, "width", &format!("{}px", w));
        style(&self.doc_wrap, "height", &format!("{}px", h));
        let _ = el("guides").set_attribute("viewBox", &format!("0 0 {} {}", w, h));
        self.update_guides();
        self.redraw_all();
    }

    pub fn apply_view(&mut self) {
        style(&self.doc_wrap, "transform", &self.view.css());
        let dpr = window().device_pixel_ratio().max(1.0);
        let z = self.view.zoom() * dpr;
        el("btn-zoom").set_text_content(Some(&format!("{:.0}%", z * 100.0)));
        set_class(&self.canvas, "pixelated", z > 2.5);
    }

    pub fn fit_view(&mut self) {
        let win = window();
        let iw = win.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1280.0);
        let ih = win.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(800.0);
        let (top, side) = (70.0, 80.0);
        let aw = (iw - side * 2.0).max(100.0);
        let ah = (ih - top - 24.0).max(100.0);
        let s = (aw / self.doc.w as f64).min(ah / self.doc.h as f64);
        let tx = (iw - self.doc.w as f64 * s) * 0.5;
        let ty = top + (ah - self.doc.h as f64 * s) * 0.5;
        self.view = Affine::translate(tx, ty).mul(&Affine::scale(s));
        self.view_fitted = true;
        self.apply_view();
    }

    /// Spiegelt die Ansicht horizontal (zur Kontrolle von Proportionen).
    pub fn flip_view(&mut self) {
        let iw = window().inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1280.0);
        let m = Affine { a: -1.0, b: 0.0, c: 0.0, d: 1.0, e: iw, f: 0.0 };
        self.view = m.mul(&self.view);
        self.apply_view();
        self.toast("Ansicht gespiegelt");
    }

    pub fn screen_to_doc(&self, x: f64, y: f64) -> (f32, f32) {
        let (dx, dy) = self.view.invert().apply(x, y);
        (dx as f32, dy as f32)
    }

    /// Ersetzt das Dokument (Neu, Öffnen, Autoladen).
    pub fn replace_document(&mut self, doc: Doc) {
        self.stroke = None;
        self.mask.resize(doc.w, doc.h);
        self.doc = doc;
        self.history.clear();
        self.pack_cache = PackCache::default();
        self.smudge_snapshot = Vec::new();
        self.setup_canvas();
        self.fit_view();
        self.ui_layers_dirty = true;
        self.refresh_all_ui();
    }

    pub fn toast(&mut self, msg: &str) {
        let t = el("toast");
        t.set_text_content(Some(msg));
        set_class(&t, "show", true);
        self.toast_seq += 1;
        let seq = self.toast_seq;
        set_timeout(1600, move || {
            with_app(|a| {
                if a.toast_seq == seq {
                    set_class(&el("toast"), "show", false);
                }
            });
        });
    }
}

// ---------------------------------------------------------------------------
// Start
// ---------------------------------------------------------------------------

#[wasm_bindgen(start)]
pub fn start() {
    let mut app = App::new();
    app.load_settings();
    // Leere Startleinwand mit der zuletzt gewählten Vorlage (wird durch Autospeichern ggf. ersetzt)
    app.doc.template = app.settings.last_template;
    app.doc.paper = app.settings.last_paper;
    APP.with(|cell| *cell.borrow_mut() = Some(app));

    ui::bind_all();
    input::bind_input();
    with_app(|a| {
        a.setup_canvas();
        a.fit_view();
        a.refresh_all_ui();
    });
    storage::init_storage();
    storage::register_service_worker();
}
