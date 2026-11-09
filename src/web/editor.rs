//! Editor: Notizbuch, Ansicht (Zoom/Scrollen), Seitenverwaltung.

use super::*;
use crate::engine::geom::Rect;
use crate::engine::history::{Action, History, PageMeta};
use crate::engine::shapes::Shape;
use crate::engine::{Item, Notebook, Pt, Smoother, Stroke, Xf};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, Path2d};

/// Abstand zwischen Seiten (pt).
pub const GAP: f64 = 28.0;

#[derive(Clone, Copy, Debug)]
pub struct View {
    pub fit: f64,
    pub zoom: f64,
    /// Notizbuch-Koordinate am linken/oberen Bildschirmrand
    pub sx: f64,
    pub sy: f64,
}

impl View {
    pub fn s(&self) -> f64 {
        self.fit * self.zoom
    }
}

/// Laufende Bedienung (Strich, Radierer, Lasso …).
pub enum Op {
    Ink {
        page: u32,
        pointer: i32,
        stroke: Stroke,
        smoother: Smoother,
        raw: Vec<Pt>,
        hold_t: f64,
        hold_pos: (f32, f32),
        quick: Option<Shape>,
        shape_tool: bool,
        straight: bool,
        predicted: Vec<Pt>,
        /// Linealkante, an der der Strich entlangläuft (Bildschirmkoordinaten)
        rule: Option<super::ruler::Guide>,
    },
    Erase {
        page: u32,
        pointer: i32,
        last: (f32, f32),
        before: Vec<Item>,
        precise: bool,
        hl_only: bool,
        radius: f32,
    },
    Lasso {
        page: u32,
        pointer: i32,
        pts: Vec<(f32, f32)>,
    },
    MoveSel {
        pointer: i32,
        start: (f32, f32),
        scale: bool,
        xf: Xf,
    },
    Laser {
        pointer: i32,
    },
    /// Mindmap: Blase antippen/verschieben oder auf leere Stelle tippen
    Mind {
        pointer: i32,
        page: u32,
        id: Option<u32>,
        start: (f64, f64),
        at: (f32, f32),
        orig: Option<Item>,
        moved: bool,
    },
}

pub struct Selection {
    pub page: u32,
    pub ids: Vec<u32>,
    pub bbox: Rect,
}

pub struct TextEdit {
    pub page: u32,
    pub item: Option<u32>,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub size: f32,
    pub color: [u8; 4],
    pub bold: bool,
}

pub struct PageBmp {
    pub canvas: HtmlCanvasElement,
    pub ctx: CanvasRenderingContext2d,
    pub scale: f64,
    pub version: u64,
    pub dark: bool,
}

pub struct Editor {
    pub nb: Notebook,
    pub history: History,
    pub view: View,
    pub vw: f64,
    pub vh: f64,
    pub dpr: f64,
    pub canvas: HtmlCanvasElement,
    pub ctx: CanvasRenderingContext2d,
    pub live: HtmlCanvasElement,
    pub lctx: CanvasRenderingContext2d,
    pub dirty: bool,
    pub paths: HashMap<u32, (u64, Path2d)>,
    pub bmps: HashMap<u32, PageBmp>,
    pub tpl_cache: super::render::TplCache,
    pub wraps: HashMap<u32, (u64, Vec<String>)>,
    pub op: Option<Op>,
    pub selection: Option<Selection>,
    pub clipboard: Vec<Item>,
    pub text_edit: Option<TextEdit>,
    pub laser: Vec<(f64, f64, f64)>,
    pub zooming: bool,
    pub nav_open: bool,
    pub nav_bookmarks: bool,
    /// Seitenübersicht zeigt das Inhaltsverzeichnis
    pub nav_toc: bool,
    /// Filter der Seitenübersicht: "c:<Farbe>" oder "t:<Schlagwort>"
    pub nav_filter: Option<String>,
    pub thumbs_dirty: bool,
    pub thumb_versions: HashMap<u32, u64>,
    pub live_page: Option<u32>,
    pub cur_page: usize,
    pub paste_at: Option<(u32, f32, f32)>,
    pub ruler: Option<super::ruler::Ruler>,
    /// Zoom-Fenster (Schreibstreifen unten)
    pub zoomwin: Option<super::zoomwin::ZoomWin>,
    /// hervorgehobene Mindmap-Blase (Seite, ID)
    pub mind_hl: Option<(u32, u32)>,
    /// Präsentationsmodus: gezeigte Seite
    pub presenting: Option<usize>,
    /// Gemerkte Vorderseite einer neuen Lernkarte
    pub card_front: Option<crate::engine::Region>,
    /// Platz für die Werkzeugleiste oben (CSS-Pixel)
    pub top_pad: f64,
}

fn ctx2d(c: &HtmlCanvasElement, desync: bool) -> CanvasRenderingContext2d {
    let opts = js_sys::Object::new();
    if desync {
        let _ = js_sys::Reflect::set(&opts, &"desynchronized".into(), &true.into());
    }
    c.get_context_with_context_options("2d", &opts).ok().flatten().expect("Canvas 2D").unchecked_into()
}

pub fn new_canvas() -> (HtmlCanvasElement, CanvasRenderingContext2d) {
    let c: HtmlCanvasElement = dom().create_element("canvas").unwrap().unchecked_into();
    let ctx = ctx2d(&c, false);
    (c, ctx)
}

impl Editor {
    pub fn new(nb: Notebook) -> Editor {
        let canvas: HtmlCanvasElement = el("view").unchecked_into();
        let live: HtmlCanvasElement = el("live").unchecked_into();
        let ctx = ctx2d(&canvas, false);
        let lctx = ctx2d(&live, true);
        let mut ed = Editor {
            nb,
            history: History::default(),
            view: View { fit: 1.0, zoom: 1.0, sx: 0.0, sy: 0.0 },
            vw: 1.0,
            vh: 1.0,
            dpr: 1.0,
            canvas,
            ctx,
            live,
            lctx,
            dirty: true,
            paths: HashMap::new(),
            bmps: HashMap::new(),
            tpl_cache: HashMap::new(),
            wraps: HashMap::new(),
            op: None,
            selection: None,
            clipboard: Vec::new(),
            text_edit: None,
            laser: Vec::new(),
            zooming: false,
            nav_open: false,
            nav_bookmarks: false,
            nav_toc: false,
            nav_filter: None,
            thumbs_dirty: true,
            thumb_versions: HashMap::new(),
            live_page: None,
            cur_page: 0,
            paste_at: None,
            ruler: None,
            zoomwin: None,
            mind_hl: None,
            presenting: None,
            card_front: None,
            top_pad: 84.0,
        };
        ed.resize();
        ed.fit_page(0);
        ed
    }

    pub fn pw(&self) -> f64 {
        self.nb.format.w as f64
    }

    pub fn ph(&self) -> f64 {
        self.nb.format.h as f64
    }

    pub fn page_top(&self, i: usize) -> f64 {
        i as f64 * (self.ph() + GAP)
    }

    pub fn content_h(&self) -> f64 {
        self.nb.pages.len() as f64 * (self.ph() + GAP) - GAP
    }

    /// Größe der Zeichenflächen an das Fenster anpassen.
    pub fn resize(&mut self) {
        let win = window();
        self.dpr = win.device_pixel_ratio().max(1.0);
        self.vw = win.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1280.0);
        self.vh = win.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(800.0);
        self.top_pad = if self.presenting.is_some() { 16.0 } else { dom().get_element_by_id("ed-top").map(|e| e.get_bounding_client_rect().bottom()).filter(|b| *b > 0.0).unwrap_or(72.0) + 12.0 };
        for c in [&self.canvas, &self.live] {
            c.set_width((self.vw * self.dpr).round() as u32);
            c.set_height((self.vh * self.dpr).round() as u32);
        }
        let old_fit = self.view.fit;
        self.view.fit = ((self.vw - 48.0) / self.pw()).min(1.6 * (self.vh - self.top_pad) / self.ph()).max(0.1);
        if (old_fit - self.view.fit).abs() > 1e-9 {
            self.bmps.clear();
        }
        self.clamp();
        self.dirty = true;
    }

    pub fn clamp(&mut self) {
        let s = self.view.s();
        let (vis_w, vis_h) = (self.vw / s, self.vh / s);
        let w = self.pw();
        if w <= vis_w {
            self.view.sx = (w - vis_w) * 0.5;
        } else {
            let m = 24.0 / s;
            self.view.sx = self.view.sx.clamp(-m, w - vis_w + m);
        }
        let min_y = -self.top_pad / s;
        let max_y = (self.content_h() - vis_h + 80.0 / s).max(min_y);
        self.view.sy = self.view.sy.clamp(min_y, max_y);
        // aktuelle Seite = Seite in der Bildschirmmitte
        let mid = self.view.sy + vis_h * 0.5;
        let i = (mid / (self.ph() + GAP)).floor().max(0.0) as usize;
        self.cur_page = i.min(self.nb.pages.len() - 1);
    }

    /// Scrollt so, dass Seite `i` oben sichtbar ist (Zoom = Seitenbreite).
    pub fn fit_page(&mut self, i: usize) {
        self.view.zoom = 1.0;
        self.bmps.clear();
        let s = self.view.s();
        self.view.sy = self.page_top(i.min(self.nb.pages.len() - 1)) - self.top_pad / s;
        self.clamp();
        self.dirty = true;
    }

    /// Doppeltipp: Ist hineingezoomt, wird auf Seitenbreite eingepasst (der getippte Punkt
    /// bleibt an seiner Stelle). Bei Seitenbreite wird die getippte Seite ganz eingepasst.
    pub fn fit_at(&mut self, px: f64, py: f64) {
        if (self.view.zoom - 1.0).abs() > 0.02 {
            self.bmps.clear();
            self.zoom_about(1.0, px, py);
        } else {
            let i = self.hit_page(px, py, GAP).map(|h| h.0).unwrap_or(self.cur_page);
            self.fit_page(i);
        }
    }

    pub fn goto_page(&mut self, i: usize) {
        let s = self.view.s();
        self.view.sy = self.page_top(i.min(self.nb.pages.len() - 1)) - self.top_pad / s;
        self.clamp();
        self.dirty = true;
    }

    pub fn scroll_by(&mut self, dx: f64, dy: f64) {
        let s = self.view.s();
        self.view.sx += dx / s;
        self.view.sy += dy / s;
        self.clamp();
        self.dirty = true;
    }

    pub fn zoom_about(&mut self, zoom: f64, cx: f64, cy: f64) {
        let (x, y) = self.to_nb(cx, cy);
        self.view.zoom = zoom.clamp(0.35, 8.0);
        let s = self.view.s();
        self.view.sx = x - cx / s;
        self.view.sy = y - cy / s;
        self.clamp();
        self.dirty = true;
    }

    pub fn to_nb(&self, px: f64, py: f64) -> (f64, f64) {
        let s = self.view.s();
        (px / s + self.view.sx, py / s + self.view.sy)
    }

    /// Bildschirmpunkt → (Seitenindex, Seitenkoordinaten). `margin` erlaubt Punkte knapp außerhalb.
    pub fn hit_page(&self, px: f64, py: f64, margin: f64) -> Option<(usize, f32, f32)> {
        let (x, y) = self.to_nb(px, py);
        let span = self.ph() + GAP;
        let i = ((y + GAP * 0.5) / span).floor();
        if i < 0.0 {
            return None;
        }
        let i = (i as usize).min(self.nb.pages.len() - 1);
        let ly = y - self.page_top(i);
        if x < -margin || x > self.pw() + margin || ly < -margin || ly > self.ph() + margin {
            return None;
        }
        Some((i, x as f32, ly as f32))
    }

    /// Seitenkoordinaten → Bildschirm.
    pub fn page_to_screen(&self, i: usize, x: f32, y: f32) -> (f64, f64) {
        let s = self.view.s();
        ((x as f64 - self.view.sx) * s, (self.page_top(i) + y as f64 - self.view.sy) * s)
    }

    pub fn page_index(&self, id: u32) -> Option<usize> {
        self.nb.page_index(id)
    }

    pub fn zoom_percent(&self) -> f64 {
        self.view.zoom * 100.0
    }

    pub fn push(&mut self, a: Action) {
        self.history.push(a);
        self.nb.modified = wall_clock();
        self.thumbs_dirty = true;
        self.dirty = true;
    }

    // ----- Seiten -----

    pub fn add_page_after(&mut self, i: usize) -> usize {
        let p = self.nb.new_page_like(i);
        let idx = (i + 1).min(self.nb.pages.len());
        self.nb.pages.insert(idx, p.clone());
        self.push(Action::AddPage { index: idx, page: p });
        idx
    }

    pub fn duplicate_page(&mut self, i: usize) -> usize {
        let p = self.nb.duplicate_page(i);
        self.nb.pages.insert(i + 1, p.clone());
        self.push(Action::AddPage { index: i + 1, page: p });
        i + 1
    }

    pub fn delete_page(&mut self, i: usize) -> bool {
        if self.nb.pages.len() <= 1 {
            return false;
        }
        let p = self.nb.pages.remove(i);
        self.bmps.remove(&p.id);
        self.selection = None;
        self.push(Action::RemovePage { index: i, page: p });
        self.clamp();
        true
    }

    pub fn move_page(&mut self, from: usize, to: usize) -> bool {
        if from >= self.nb.pages.len() || to >= self.nb.pages.len() || from == to {
            return false;
        }
        let p = self.nb.pages.remove(from);
        self.nb.pages.insert(to, p);
        self.push(Action::MovePage { from, to });
        true
    }

    pub fn set_page_meta(&mut self, i: usize, f: impl FnOnce(&mut PageMeta)) {
        let Some(p) = self.nb.pages.get_mut(i) else { return };
        let before = PageMeta::of(p);
        let mut after = before.clone();
        f(&mut after);
        if after == before {
            return;
        }
        after.apply(p);
        let id = p.id;
        self.push(Action::PageMeta { page: id, before, after });
    }

    /// Vorlage/Papier für alle Seiten (eine Aktion).
    pub fn set_all_pages_meta(&mut self, template: crate::engine::Template, paper: Color) {
        let mut group = Vec::new();
        for p in self.nb.pages.iter_mut() {
            let before = PageMeta::of(p);
            let mut after = before.clone();
            after.template = template;
            after.paper = paper;
            if before != after {
                after.apply(p);
                group.push(Action::PageMeta { page: p.id, before, after });
            }
        }
        if !group.is_empty() {
            self.push(Action::Group(group));
        }
    }

    pub fn undo(&mut self) -> bool {
        self.selection = None;
        match self.history.undo(&mut self.nb) {
            Some(focus) => {
                self.after_history(focus);
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        self.selection = None;
        match self.history.redo(&mut self.nb) {
            Some(focus) => {
                self.after_history(focus);
                true
            }
            None => false,
        }
    }

    fn after_history(&mut self, focus: Option<usize>) {
        self.nb.modified = wall_clock();
        self.thumbs_dirty = true;
        self.dirty = true;
        // Seite sichtbar machen, falls sie außerhalb liegt
        if let Some(i) = focus {
            let s = self.view.s();
            let top = self.page_top(i);
            let (v0, v1) = (self.view.sy, self.view.sy + self.vh / s);
            if top + self.ph() < v0 || top > v1 {
                self.goto_page(i);
            }
        }
        self.clamp();
    }
}
