//! Eingabe: S-Pen (Druck, Schweben, Seitentaste), Handballen-Sperre, Finger-Gesten mit Schwung, Maus, Tastatur.

use super::*;
use web_sys::{KeyboardEvent, PointerEvent, WheelEvent};

#[derive(Clone, Copy, Debug)]
pub struct TouchPt {
    pub id: i32,
    pub x: f64,
    pub y: f64,
    pub sx: f64,
    pub sy: f64,
}

/// Finger, die gerade das Lineal halten.
pub struct RulerGrab {
    /// Zirkel an der Spitze angefasst (Radius/Richtung ändern statt verschieben)
    pub tip: bool,
    pub base: super::ruler::Ruler,
    pub anchors: Vec<(i32, f64, f64)>,
    pub cur: Vec<(i32, f64, f64)>,
}

pub struct Gesture {
    pub origin: (f64, f64, f64),
    pub base: (f64, f64, f64),
    pub anchors: Vec<(i32, f64, f64)>,
    pub max_fingers: usize,
    pub t0: f64,
    pub moved: f64,
    /// letzte Bewegungen für den Schwung (dx, dy, dt)
    pub vel: Vec<(f64, f64, f64)>,
    pub last: (f64, f64, f64),
}

pub fn client_xy(e: &PointerEvent) -> (f64, f64) {
    let get = |k: &str| js_sys::Reflect::get(e, &k.into()).ok().and_then(|v| v.as_f64());
    let (x, y) = match (get("clientX"), get("clientY")) {
        (Some(x), Some(y)) => (x, y),
        _ => (e.client_x() as f64, e.client_y() as f64),
    };
    // Eingaben im Zoom-Fenster auf die Seite umrechnen (schon Bereichskoordinaten)
    if super::zoomwin::mapped(e.pointer_id()) {
        return super::zoomwin::map_xy(e.pointer_id(), x, y);
    }
    // Fenster → Koordinaten des aktiven Bereichs (Seitenleiste / Split-Ansicht)
    (x - pane_ox(), y)
}

thread_local! {
    static PANE_OX: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
}

/// Linke Kante des aktiven Editorbereichs im Fenster.
pub fn pane_ox() -> f64 {
    PANE_OX.with(|c| c.get())
}

pub fn set_pane_ox(v: f64) {
    PANE_OX.with(|c| c.set(v));
}

fn target_id(e: &PointerEvent) -> String {
    e.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()).map(|el| el.id()).unwrap_or_default()
}

fn samples(e: &PointerEvent) -> Vec<(f64, f64, f32)> {
    let list = e.get_coalesced_events();
    let pen = e.pointer_type() == "pen";
    let conv = |pe: &PointerEvent| {
        let (x, y) = client_xy(pe);
        (x, y, if pen { pe.pressure().clamp(0.0, 1.0) } else { 0.55 })
    };
    if list.length() == 0 {
        vec![conv(e)]
    } else {
        (0..list.length()).map(|i| conv(&list.get(i).unchecked_into::<PointerEvent>())).collect()
    }
}

fn predicted(e: &PointerEvent) -> Vec<(f64, f64, f32)> {
    let Ok(f) = js_sys::Reflect::get(e, &"getPredictedEvents".into()) else { return Vec::new() };
    let Some(f) = f.dyn_ref::<js_sys::Function>() else { return Vec::new() };
    let Ok(arr) = f.call0(e) else { return Vec::new() };
    let arr: js_sys::Array = arr.unchecked_into();
    (0..arr.length().min(3))
        .map(|i| {
            let pe: PointerEvent = arr.get(i).unchecked_into();
            let (x, y) = client_xy(&pe);
            (x, y, pe.pressure().clamp(0.0, 1.0))
        })
        .collect()
}

pub fn bind_input() {
    let stage = el("editor");
    on(&stage, "pointerdown", |e: PointerEvent| {
        if !is_canvas_target(&e) {
            return;
        }
        e.prevent_default();
        with_app(|a| a.pointer_down(&e));
    });
    on(&stage, "pointermove", |e: PointerEvent| {
        // Bewegungen über Leisten/Menüs nur für laufende Aktionen (Zeigerfang) auswerten
        if !is_canvas_target(&e) && !is_captured(&e) {
            return;
        }
        with_app(|a| a.pointer_move(&e));
    });
    on(&stage, "pointerup", |e: PointerEvent| {
        with_app(|a| a.pointer_up(&e, false));
    });
    on(&stage, "pointercancel", |e: PointerEvent| {
        with_app(|a| a.pointer_up(&e, true));
    });
    on(&stage, "pointerleave", |e: PointerEvent| {
        if e.pointer_type() == "pen" {
            with_app(|a| {
                a.pen_left();
                a.hide_cursor();
            });
        }
    });
    on(&stage, "wheel", |e: WheelEvent| {
        e.prevent_default();
        with_app(|a| a.wheel(&e));
    });
    on(&dom(), "contextmenu", |e: Event| {
        let tag = e.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()).map(|el| el.tag_name()).unwrap_or_default();
        if tag != "INPUT" && tag != "TEXTAREA" {
            e.prevent_default();
        }
        let pen = js_sys::Reflect::get(&e, &"pointerType".into()).ok().and_then(|v| v.as_string()).as_deref() == Some("pen");
        if pen {
            with_app(|a| a.native_pen_button(true));
        }
    });
    on(&window(), "keydown", |e: KeyboardEvent| {
        with_app(|a| a.key_down(&e));
    });
    on(&window(), "keyup", |e: KeyboardEvent| {
        if e.code() == "Space" {
            with_app(|a| a.space_down = false);
        }
    });
    on(&window(), "resize", |_: Event| {
        with_app(|a| {
            a.layout_panes();
            if let Some(ed) = &mut a.ed {
                if let Some(i) = ed.presenting {
                    ed.present_fit(i);
                }
            }
            a.position_overlays();
            a.request_frame();
        });
    });
    // Rückrufe der Android-App (MainActivity liest Taste und Stift-Nähe direkt aus Android)
    let w = window();
    let btn = Closure::<dyn FnMut(JsValue)>::new(|v: JsValue| {
        let down = v.as_bool().unwrap_or(false);
        with_app(|a| a.native_pen_button(down));
    });
    let _ = js_sys::Reflect::set(&w, &"zeichentoolPenButton".into(), btn.as_ref());
    btn.forget();
    let state = Closure::<dyn FnMut(JsValue)>::new(|v: JsValue| {
        let near = v.as_bool().unwrap_or(false);
        with_app(|a| a.native_pen_state(near));
    });
    let _ = js_sys::Reflect::set(&w, &"zeichentoolPenState".into(), state.as_ref());
    state.forget();
    let auth = Closure::<dyn FnMut(JsValue)>::new(|v: JsValue| {
        let ok = v.as_bool().unwrap_or(false);
        with_app(|a| a.bio_result(ok));
    });
    let _ = js_sys::Reflect::set(&w, &"zeichentoolAuthResult".into(), auth.as_ref());
    auth.forget();
}

/// Ereignis eines Zeigers, den die Zeichenfläche gerade „gefangen“ hat (laufender Strich)?
fn is_captured(e: &PointerEvent) -> bool {
    e.target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        .map(|el| el.id() == "editor")
        .unwrap_or(false)
}

/// Nur Ereignisse auf der Zeichenfläche (nicht auf Leisten/Menüs) steuern das Zeichnen.
fn is_canvas_target(e: &PointerEvent) -> bool {
    e.target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        .map(|el| matches!(el.id().as_str(), "live" | "view" | "live2" | "view2" | "zw-canvas"))
        .unwrap_or(false)
}

impl App {
    fn pointer_down(&mut self, e: &PointerEvent) {
        if self.screen != Screen::Editor {
            return;
        }
        // Split-Ansicht: Stift oder Finger im anderen Bereich macht ihn aktiv
        if self.is_other_pane(&target_id(e)) {
            let busy = self.ed.as_ref().is_some_and(|ed| ed.op.is_some()) || !self.touches.is_empty();
            let palm = e.pointer_type() == "touch" && self.palm_locked();
            if busy || palm {
                return;
            }
            self.activate_other();
        }
        if target_id(e) == "zw-canvas" {
            // Handballen auf dem Zoom-Fenster ignorieren; Finger schreibt nur, wenn erlaubt
            if e.pointer_type() == "touch" && (!self.finger_draws() || self.palm_locked()) {
                return;
            }
            if self.op_pointer().is_none() {
                self.zw_pointer_down(e.pointer_id());
            }
        }
        match e.pointer_type().as_str() {
            "pen" => self.pen_event(e),
            "touch" => self.touch_down(e),
            _ => self.mouse_down(e),
        }
    }

    fn pointer_move(&mut self, e: &PointerEvent) {
        if self.screen != Screen::Editor {
            return;
        }
        // Schweben über dem inaktiven Bereich der Split-Ansicht ignorieren
        if self.is_other_pane(&target_id(e)) && self.touches.is_empty() && self.ed.as_ref().is_some_and(|ed| ed.op.is_none()) {
            self.hide_cursor();
            return;
        }
        match e.pointer_type().as_str() {
            "pen" => self.pen_event(e),
            "touch" => self.touch_move(e),
            _ => self.mouse_move(e),
        }
    }

    fn pointer_up(&mut self, e: &PointerEvent, cancel: bool) {
        let id = e.pointer_id();
        let _ = el("editor").release_pointer_capture(id);
        if super::zoomwin::mapped(id) {
            self.pointer_up_inner(e, cancel);
            super::zoomwin::clear_map(id);
            if !cancel {
                self.zw_after_stroke();
            }
            return;
        }
        self.pointer_up_inner(e, cancel);
    }

    fn pointer_up_inner(&mut self, e: &PointerEvent, cancel: bool) {
        let id = e.pointer_id();
        match e.pointer_type().as_str() {
            "pen" => {
                self.last_pen_time = now();
                self.pen_barrel = e.buttons() & 2 != 0;
                if self.op_pointer() == Some(id) {
                    let pts = samples(e);
                    self.move_op(id, &pts, &[]);
                    self.end_op(id);
                }
                self.release_hold_eraser();
            }
            "touch" => self.touch_up(e, cancel),
            _ => {
                if self.ruler_up(id) {
                    return;
                }
                if self.mouse_pan.map(|m| m.0) == Some(id) {
                    self.mouse_pan = None;
                } else if self.op_pointer() == Some(id) {
                    self.end_op(id);
                }
            }
        }
    }

    pub fn op_pointer(&self) -> Option<i32> {
        use super::editor::Op;
        self.ed.as_ref().and_then(|ed| match &ed.op {
            Some(Op::Ink { pointer, .. } | Op::Erase { pointer, .. } | Op::Lasso { pointer, .. } | Op::MoveSel { pointer, .. } | Op::Laser { pointer } | Op::Mind { pointer, .. }) => Some(*pointer),
            None => None,
        })
    }

    // ----- S-Pen -----

    fn pen_event(&mut self, e: &PointerEvent) {
        let t = now();
        if !self.settings.pen_seen {
            self.settings.pen_seen = true;
            self.save_settings();
        }
        self.last_pen_time = t;
        self.pen_in_range = true;
        self.momentum = None;
        self.update_pen_badge();
        // Hand lag schon auf: Finger-Aktionen abbrechen, verschobene Ansicht zurücksetzen
        if self.gesture.is_some() || (!self.touches.is_empty()) {
            self.abort_touch(true);
        }
        let id = e.pointer_id();
        let buttons = e.buttons();
        let contact = buttons & 1 != 0;
        let barrel = buttons & 2 != 0 || buttons & 32 != 0;
        let (sx, sy) = client_xy(e);
        if barrel && !self.pen_barrel {
            self.pen_barrel = true;
            self.pen_button_pressed();
        } else if !barrel && !self.native_button {
            self.pen_barrel = false;
            self.release_hold_eraser();
        }
        let mine = self.op_pointer() == Some(id);
        if contact {
            if self.op_pointer().is_none() {
                // Tippen auf Knöpfe/Menüs (Stift zittert leicht) darf keinen Strich beginnen
                if !is_canvas_target(e) {
                    return;
                }
                let _ = el("editor").set_pointer_capture(id);
                self.hide_cursor();
                self.begin_op(id, sx, sy, e.pressure());
            } else if mine {
                let pts = samples(e);
                let pred = predicted(e);
                self.move_op(id, &pts, &pred);
            }
        } else {
            if mine {
                self.end_op(id);
            }
            // eigener Vorschaupunkt statt zusätzlichem System-Zeiger
            set_class(&el("editor"), "pen-hover", true);
            self.show_hover(sx, sy);
        }
    }

    fn pen_button_pressed(&mut self) {
        let t = now();
        if t - self.last_barrel_press < 250.0 {
            return;
        }
        self.last_barrel_press = t;
        match self.settings.pen_button {
            PenButton::Select => {
                if let Some(p) = self.op_pointer() {
                    self.end_op(p);
                }
                if self.tool == Tool::Eraser {
                    let back = if self.prev_tool == Tool::Eraser { Tool::Pen } else { self.prev_tool };
                    self.set_tool(back);
                    self.toast(back.name_de());
                } else {
                    self.set_tool(Tool::Eraser);
                    self.toast("Radierer");
                }
                vibrate(8);
            }
            PenButton::Hold => {
                if let Some(p) = self.op_pointer() {
                    self.end_op(p);
                }
                self.hold_eraser = true;
                self.refresh_toolbar();
            }
            PenButton::Undo => self.undo(),
        }
    }

    fn release_hold_eraser(&mut self) {
        if self.hold_eraser && !self.pen_barrel {
            if let Some(p) = self.op_pointer() {
                self.end_op(p);
            }
            self.hold_eraser = false;
            self.refresh_toolbar();
        }
    }

    pub fn native_pen_button(&mut self, down: bool) {
        self.native_button = down;
        self.native_status();
        self.settings.pen_seen = true;
        self.last_pen_time = now();
        if down {
            if !self.pen_barrel {
                self.pen_barrel = true;
                self.pen_button_pressed();
            }
        } else {
            self.pen_barrel = false;
            self.release_hold_eraser();
        }
    }

    pub fn native_pen_state(&mut self, near: bool) {
        self.native_near = near;
        self.native_status();
        if near {
            self.settings.pen_seen = true;
            self.pen_in_range = true;
            self.last_pen_time = now();
            self.abort_touch(true);
            self.update_pen_badge();
        } else {
            self.pen_left();
        }
    }

    fn native_status(&self) {
        set_text(
            "pen-native",
            &format!("Android: Stift {} · Taste {}", if self.native_near { "in der Nähe" } else { "weg" }, if self.native_button { "GEDRÜCKT" } else { "losgelassen" }),
        );
    }

    pub fn pen_left(&mut self) {
        if self.pen_in_range {
            self.pen_in_range = false;
            self.last_pen_time = now();
            self.update_pen_badge();
            let ms = (self.settings.palm_lock * 1000.0) as i32 + 60;
            set_timeout(ms, || {
                with_app(|a| a.update_pen_badge());
            });
        }
    }

    pub fn palm_locked(&self) -> bool {
        if !self.settings.pen_seen {
            return false;
        }
        let dt = now() - self.last_pen_time;
        (self.pen_in_range && dt < 4_000.0) || dt < self.settings.palm_lock as f64 * 1000.0
    }

    pub fn update_pen_badge(&mut self) {
        let on = self.palm_locked();
        if on != self.badge_on {
            self.badge_on = on;
            set_class(&el("pen-badge"), "on", on);
        }
    }

    /// Schwebender Stift: Größe des Werkzeugs als Kreis anzeigen.
    fn show_hover(&mut self, sx: f64, sy: f64) {
        let Some(ed) = &self.ed else { return };
        let s = ed.view.s();
        let tool = self.effective_tool();
        let d = match tool {
            Tool::Pen | Tool::Shape => self.prefs.pen_sizes[self.prefs.pen_si] as f64 * s,
            Tool::Highlighter => self.prefs.hl_sizes[self.prefs.hl_si] as f64 * s,
            Tool::Eraser => self.prefs.er_sizes[self.prefs.er_si] as f64 * s,
            _ => {
                self.hide_cursor();
                return;
            }
        };
        let c = el("cursor");
        let d = d.max(5.0);
        style(&c, "width", &format!("{}px", d));
        style(&c, "height", &format!("{}px", d));
        // sx/sy sind Koordinaten des Editorbereichs → Fenster (Seitenleiste / Split-Ansicht)
        style(&c, "transform", &format!("translate({}px,{}px) translate(-50%,-50%)", sx + ed.ox, sy));
        set_class(&c, "eraser", tool == Tool::Eraser);
        set_class(&c, "show", true);
    }

    pub fn hide_cursor(&mut self) {
        set_class(&el("cursor"), "show", false);
        set_class(&el("editor"), "pen-hover", false);
    }

    // ----- Finger -----

    fn finger_draws(&self) -> bool {
        self.settings.finger_draw || !self.settings.pen_seen
    }

    /// Finger-Eingaben verwerfen; Ansicht zurücksetzen, falls sie gerade erst verschoben wurde.
    fn abort_touch(&mut self, restore: bool) {
        if let Some(g) = self.gesture.take() {
            if restore && now() - g.t0 < 2500.0 {
                if let Some(ed) = &mut self.ed {
                    ed.view.zoom = g.origin.0;
                    ed.view.sx = g.origin.1;
                    ed.view.sy = g.origin.2;
                    ed.zooming = false;
                    ed.clamp();
                    ed.dirty = true;
                }
            }
        }
        let touch_op = self.touches.iter().any(|t| Some(t.id) == self.op_pointer());
        if touch_op {
            self.cancel_op();
        }
        self.touches.clear();
        self.momentum = None;
        self.request_frame();
    }

    fn view_tuple(&self) -> (f64, f64, f64) {
        self.ed.as_ref().map(|e| (e.view.zoom, e.view.sx, e.view.sy)).unwrap_or((1.0, 0.0, 0.0))
    }

    fn touch_down(&mut self, e: &PointerEvent) {
        let t = now();
        let (x, y) = client_xy(e);
        let id = e.pointer_id();
        // Lineal anfassen geht immer – auch wenn der Stift gerade in der Nähe ist
        if self.ruler_down(id, x, y) {
            let _ = el("editor").set_pointer_capture(id);
            return;
        }
        if self.palm_locked() || e.width() > 42 || e.height() > 42 {
            return;
        }
        // Zoom-Rahmen mit einem Finger verschieben
        if self.touches.is_empty() && self.zw_frame_hit(x, y) {
            if let Some(z) = self.ed.as_ref().and_then(|e| e.zoomwin) {
                let _ = el("editor").set_pointer_capture(id);
                self.zw_drag = Some((id, x, y, (z.x, z.y, z.page)));
                return;
            }
        }
        let _ = el("editor").set_pointer_capture(id);
        self.momentum = None;
        self.touches.retain(|p| p.id != id);
        self.touches.push(TouchPt { id, x, y, sx: x, sy: y });
        self.hide_cursor();
        if self.touches.len() == 1 && self.finger_draws() && self.tool != Tool::Laser {
            self.begin_op(id, x, y, 0.55);
            return;
        }
        // zweiter Finger während eines Finger-Strichs → war eine Geste
        if let Some(op_id) = self.op_pointer() {
            if self.touches.iter().any(|p| p.id == op_id) {
                self.cancel_op();
            }
        }
        let v = self.view_tuple();
        let (origin, t0, maxf, moved) = match &self.gesture {
            Some(g) => (g.origin, g.t0, g.max_fingers, g.moved),
            None => (v, t, 0, 0.0),
        };
        let (cx, cy) = self.centroid();
        self.gesture = Some(Gesture {
            origin,
            base: v,
            anchors: self.touches.iter().map(|p| (p.id, p.x, p.y)).collect(),
            max_fingers: maxf.max(self.touches.len()),
            t0,
            moved,
            vel: Vec::new(),
            last: (cx, cy, t),
        });
        if let Some(ed) = &mut self.ed {
            ed.zooming = self.touches.len() >= 2;
        }
    }

    fn centroid(&self) -> (f64, f64) {
        let n = self.touches.len().max(1) as f64;
        (self.touches.iter().map(|p| p.x).sum::<f64>() / n, self.touches.iter().map(|p| p.y).sum::<f64>() / n)
    }

    fn touch_move(&mut self, e: &PointerEvent) {
        let id = e.pointer_id();
        let (x, y) = client_xy(e);
        if let Some((did, x0, y0, start)) = self.zw_drag {
            if did == id {
                self.zw_drag(start, x - x0, y - y0);
                return;
            }
        }
        if self.ruler_move(id, x, y) {
            return;
        }
        let Some(tp) = self.touches.iter_mut().find(|p| p.id == id) else { return };
        tp.x = x;
        tp.y = y;
        let d = ((x - tp.sx).powi(2) + (y - tp.sy).powi(2)).sqrt();
        if self.op_pointer() == Some(id) {
            let pts = samples(e);
            self.move_op(id, &pts, &[]);
            return;
        }
        // Präsentation: Finger blättert nur (kein Verschieben/Zoomen)
        if self.present.is_some() {
            if let Some(g) = &mut self.gesture {
                g.moved = g.moved.max(d);
                g.last = (x, y, now());
            }
            return;
        }
        let one_finger = self.settings.one_finger_scroll;
        let (cx, cy) = self.centroid();
        let t = now();
        let Some(g) = &mut self.gesture else { return };
        g.moved = g.moved.max(d);
        if g.moved < 8.0 {
            return;
        }
        let dt = (t - g.last.2).max(1.0);
        g.vel.push((cx - g.last.0, cy - g.last.1, dt));
        if g.vel.len() > 5 {
            g.vel.remove(0);
        }
        g.last = (cx, cy, t);
        let cur: Vec<(f64, f64, f64, f64)> = g.anchors.iter().filter_map(|(id, ax, ay)| self.touches.iter().find(|p| p.id == *id).map(|p| (*ax, *ay, p.x, p.y))).collect();
        let (z0, sx0, sy0) = g.base;
        let Some(ed) = &mut self.ed else { return };
        let fit = ed.view.fit;
        match cur.len() {
            0 => {}
            1 => {
                if !one_finger {
                    return;
                }
                let (ax, ay, qx, qy) = cur[0];
                let s = fit * z0;
                ed.view.zoom = z0;
                ed.view.sx = sx0 - (qx - ax) / s;
                ed.view.sy = sy0 - (qy - ay) / s;
            }
            _ => {
                let (p1x, p1y, q1x, q1y) = cur[0];
                let (p2x, p2y, q2x, q2y) = cur[1];
                let lp = ((p2x - p1x).powi(2) + (p2y - p1y).powi(2)).sqrt().max(1.0);
                let lq = ((q2x - q1x).powi(2) + (q2y - q1y).powi(2)).sqrt().max(1.0);
                let zoom = (z0 * lq / lp).clamp(0.35, 8.0);
                let (pmx, pmy) = ((p1x + p2x) * 0.5, (p1y + p2y) * 0.5);
                let (qmx, qmy) = ((q1x + q2x) * 0.5, (q1y + q2y) * 0.5);
                let s0 = fit * z0;
                let (nx, ny) = (pmx / s0 + sx0, pmy / s0 + sy0);
                let s1 = fit * zoom;
                ed.view.zoom = zoom;
                ed.view.sx = nx - qmx / s1;
                ed.view.sy = ny - qmy / s1;
            }
        }
        ed.clamp();
        ed.dirty = true;
        self.position_overlays();
        self.request_frame();
    }

    fn touch_up(&mut self, e: &PointerEvent, cancel: bool) {
        let id = e.pointer_id();
        if self.zw_drag.map(|d| d.0) == Some(id) {
            self.zw_drag = None;
            return;
        }
        if self.ruler_up(id) {
            return;
        }
        let was = self.touches.len();
        self.touches.retain(|p| p.id != id);
        if was == self.touches.len() {
            return;
        }
        if self.op_pointer() == Some(id) {
            if cancel {
                self.cancel_op();
            } else {
                self.end_op(id);
            }
            return;
        }
        if self.touches.is_empty() && self.present.is_some() {
            if let Some(g) = self.gesture.take() {
                let (x, y) = client_xy(e);
                let dx = x - g.anchors.first().map(|a| a.1).unwrap_or(x);
                let dy = y - g.anchors.first().map(|a| a.2).unwrap_or(y);
                let vw = window().inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1000.0);
                if !cancel && g.max_fingers == 1 {
                    if dx.abs() > 60.0 && dx.abs() > dy.abs() {
                        self.present_step(if dx < 0.0 { 1 } else { -1 });
                    } else if dy.abs() > 60.0 {
                        self.present_step(if dy < 0.0 { 1 } else { -1 });
                    } else if g.moved < 14.0 {
                        if x > vw * 0.6 {
                            self.present_step(1);
                        } else if x < vw * 0.4 {
                            self.present_step(-1);
                        }
                    }
                }
            }
            return;
        }
        if self.touches.is_empty() {
            if let Some(g) = self.gesture.take() {
                let dt = now() - g.t0;
                if let Some(ed) = &mut self.ed {
                    if ed.zooming {
                        ed.zooming = false;
                        ed.dirty = true;
                    }
                }
                if !cancel && dt < 300.0 && g.moved < 14.0 {
                    match g.max_fingers {
                        1 => {
                            let (x, y) = client_xy(e);
                            let t = now();
                            let double = self
                                .last_tap
                                .take()
                                .is_some_and(|(t0, x0, y0)| t - t0 < 380.0 && (x - x0).hypot(y - y0) < 48.0);
                            if self.tap_link(x, y) {
                                // Link gefolgt – kein Doppeltipp.
                            } else if double {
                                if let Some(ed) = &mut self.ed {
                                    ed.fit_at(x, y);
                                }
                                self.after_view_change();
                            } else {
                                self.last_tap = Some((t, x, y));
                            }
                        }
                        2 => self.undo(),
                        3 => self.redo(),
                        _ => {}
                    }
                } else if g.max_fingers == 1 && g.moved >= 8.0 && !cancel {
                    // Schwung
                    let (mut vx, mut vy, mut tt) = (0.0, 0.0, 0.0);
                    for (dx, dy, dt) in &g.vel {
                        vx += dx;
                        vy += dy;
                        tt += dt;
                    }
                    if tt > 0.0 && now() - g.last.2 < 80.0 {
                        let (vx, vy) = (vx / tt, vy / tt);
                        if (vx * vx + vy * vy).sqrt() > 0.25 {
                            self.momentum = Some(Momentum { vx, vy, t: now() });
                        }
                    }
                }
                self.request_frame();
            }
        } else if self.gesture.is_some() {
            let v = self.view_tuple();
            let (cx, cy) = self.centroid();
            let anchors = self.touches.iter().map(|p| (p.id, p.x, p.y)).collect();
            if let Some(g) = &mut self.gesture {
                g.base = v;
                g.anchors = anchors;
                g.last = (cx, cy, now());
                g.vel.clear();
            }
        }
    }

    /// Schwung-Scrollen; gibt `true` zurück, solange es weiterläuft.
    pub fn step_momentum(&mut self, t: f64) -> bool {
        let Some(m) = &mut self.momentum else { return false };
        let dt = (t - m.t).clamp(0.0, 50.0);
        m.t = t;
        let decay = 0.995f64.powf(dt);
        m.vx *= decay;
        m.vy *= decay;
        let (dx, dy) = (-m.vx * dt, -m.vy * dt);
        let speed = (m.vx * m.vx + m.vy * m.vy).sqrt();
        if speed < 0.02 {
            self.momentum = None;
            return false;
        }
        if let Some(ed) = &mut self.ed {
            ed.scroll_by(dx, dy);
        }
        self.position_overlays();
        true
    }

    // ----- Lineal / Geodreieck -----

    fn ruler_down(&mut self, id: i32, x: f64, y: f64) -> bool {
        let Some(r) = self.ed.as_ref().and_then(|e| e.ruler) else { return false };
        match &mut self.ruler_grab {
            Some(g) => {
                if g.cur.len() >= 2 || g.tip || r.kind == super::ruler::RulerKind::Compass {
                    return true;
                }
                g.cur.push((id, x, y));
                g.base = r;
                g.anchors = g.cur.clone();
            }
            None => {
                if !r.contains(x, y) {
                    return false;
                }
                self.ruler_grab = Some(RulerGrab { tip: r.grabs_tip(x, y), base: r, anchors: vec![(id, x, y)], cur: vec![(id, x, y)] });
            }
        }
        self.momentum = None;
        true
    }

    fn ruler_move(&mut self, id: i32, x: f64, y: f64) -> bool {
        let Some(g) = &mut self.ruler_grab else { return false };
        let Some(c) = g.cur.iter_mut().find(|c| c.0 == id) else { return false };
        c.1 = x;
        c.2 = y;
        let mut r = g.base;
        if g.tip {
            // Zirkel aufziehen: Radius und Richtung folgen dem Finger
            r.radius = (x - r.cx).hypot(y - r.cy).max(24.0);
            r.angle = (y - r.cy).atan2(x - r.cx);
        } else if g.anchors.len() == 1 || g.cur.len() == 1 {
            let (_, ax, ay) = g.anchors[0];
            let (_, qx, qy) = g.cur[0];
            r.cx += qx - ax;
            r.cy += qy - ay;
        } else {
            let (p1, p2) = (g.anchors[0], g.anchors[1]);
            let (q1, q2) = (g.cur[0], g.cur[1]);
            let d0 = (p2.2 - p1.2).atan2(p2.1 - p1.1);
            let d1 = (q2.2 - q1.2).atan2(q2.1 - q1.1);
            let angle = super::ruler::Ruler::snap_angle(g.base.angle + (d1 - d0));
            let da = angle - g.base.angle;
            let (pm, qm) = (((p1.1 + p2.1) * 0.5, (p1.2 + p2.2) * 0.5), ((q1.1 + q2.1) * 0.5, (q1.2 + q2.2) * 0.5));
            let (s, c) = da.sin_cos();
            let (vx, vy) = (g.base.cx - pm.0, g.base.cy - pm.1);
            r.cx = qm.0 + vx * c - vy * s;
            r.cy = qm.1 + vx * s + vy * c;
            r.angle = angle;
        }
        if let Some(ed) = &mut self.ed {
            ed.ruler = Some(r);
            ed.dirty = true;
        }
        self.request_frame();
        true
    }

    fn ruler_up(&mut self, id: i32) -> bool {
        let Some(g) = &mut self.ruler_grab else { return false };
        if !g.cur.iter().any(|c| c.0 == id) {
            return false;
        }
        g.cur.retain(|c| c.0 != id);
        if g.cur.is_empty() {
            self.ruler_grab = None;
        } else if let Some(r) = self.ed.as_ref().and_then(|e| e.ruler) {
            g.base = r;
            g.anchors = g.cur.clone();
        }
        self.sync_ui_rects();
        true
    }

    // ----- Maus -----

    fn mouse_down(&mut self, e: &PointerEvent) {
        let id = e.pointer_id();
        let (x, y) = client_xy(e);
        let _ = el("editor").set_pointer_capture(id);
        if e.button() == 0 && self.ruler_down(id, x, y) {
            return;
        }
        if e.button() == 1 || (e.button() == 0 && self.space_down) {
            self.mouse_pan = Some((id, x, y));
            return;
        }
        if e.button() == 0 {
            self.begin_op(id, x, y, 0.55);
        }
    }

    fn mouse_move(&mut self, e: &PointerEvent) {
        let (x, y) = client_xy(e);
        if self.ruler_move(e.pointer_id(), x, y) {
            return;
        }
        if let Some((id, lx, ly)) = self.mouse_pan {
            if id == e.pointer_id() {
                if let Some(ed) = &mut self.ed {
                    ed.scroll_by(lx - x, ly - y);
                }
                self.mouse_pan = Some((id, x, y));
                self.position_overlays();
                self.request_frame();
                return;
            }
        }
        if self.op_pointer() == Some(e.pointer_id()) {
            let pts = samples(e);
            self.move_op(e.pointer_id(), &pts, &[]);
        } else {
            self.show_hover(x, y);
        }
    }

    fn wheel(&mut self, e: &WheelEvent) {
        let tid = e.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()).map(|el| el.id()).unwrap_or_default();
        if self.is_other_pane(&tid) {
            self.activate_other();
        }
        let Some(ed) = &mut self.ed else { return };
        let mut dy = e.delta_y();
        let mut dx = e.delta_x();
        if e.delta_mode() == 1 {
            dy *= 32.0;
            dx *= 32.0;
        }
        if e.ctrl_key() || e.meta_key() {
            let f = (-dy * 0.0025).exp();
            let z = ed.view.zoom * f;
            ed.zoom_about(z, e.client_x() as f64 - pane_ox(), e.client_y() as f64);
            ed.bmps.clear();
        } else {
            ed.scroll_by(dx, dy);
        }
        self.position_overlays();
        self.request_frame();
    }

    // ----- Tastatur -----

    fn key_down(&mut self, e: &KeyboardEvent) {
        if let Some(t) = e.target() {
            if let Some(el) = t.dyn_ref::<web_sys::Element>() {
                let tag = el.tag_name();
                if tag == "INPUT" && el.id() == "marks-input" && e.key() == "Enter" {
                    e.prevent_default();
                    self.marks_action("tag-new", "");
                    return;
                }
                if tag == "INPUT" && e.key() == "Enter" {
                    let id = el.id();
                    let target = match id.as_str() {
                        "exam-name" | "exam-date" => Some(("exam-save", String::new())),
                        "frac-text" => {
                            e.prevent_default();
                            self.fraction_action("frac-insert", "");
                            return;
                        }
                        "subject-name" => Some(("subject-add", String::new())),
                        _ => id.strip_prefix("grade-in-").map(|i| ("grade-add", i.to_string())),
                    };
                    if let Some((act, arg)) = target {
                        e.prevent_default();
                        if act == "exam-save" {
                            self.exam_action(act, &arg);
                        } else {
                            self.grades_action(act, &arg);
                        }
                        return;
                    }
                }
                if tag == "INPUT" && el.id() == "sym-text" && e.key() == "Enter" {
                    e.prevent_default();
                    self.symbol_action("sym-insert", "");
                    return;
                }
                if tag == "INPUT" || tag == "TEXTAREA" || tag == "SELECT" {
                    if e.key() == "Escape" && tag == "TEXTAREA" {
                        self.commit_text();
                    }
                    return;
                }
            }
        }
        if self.present.is_some() {
            match e.key().as_str() {
                "ArrowRight" | "ArrowDown" | "PageDown" | " " | "Enter" => self.present_step(1),
                "ArrowLeft" | "ArrowUp" | "PageUp" | "Backspace" => self.present_step(-1),
                "Escape" => self.present_exit(),
                _ => return,
            }
            e.prevent_default();
            return;
        }
        if e.key() == "Escape" {
            self.close_popovers();
            self.close_modals();
        }
        if self.screen != Screen::Editor {
            return;
        }
        let ctrl = e.ctrl_key() || e.meta_key();
        let key = e.key().to_lowercase();
        if ctrl {
            match key.as_str() {
                "z" if e.shift_key() => self.redo(),
                "z" => self.undo(),
                "y" => self.redo(),
                "c" => self.selection_action("sel-copy", ""),
                "x" => self.selection_action("sel-cut", ""),
                "d" => self.selection_action("sel-dup", ""),
                _ => return,
            }
            e.prevent_default();
            return;
        }
        match key.as_str() {
            "p" => self.set_tool(Tool::Pen),
            "h" => self.set_tool(Tool::Highlighter),
            "e" => self.set_tool(Tool::Eraser),
            "l" => self.set_tool(Tool::Lasso),
            "s" => self.set_tool(Tool::Shape),
            "m" => self.set_tool(Tool::Mind),
            "t" => self.set_tool(Tool::Text),
            "delete" | "backspace" => self.selection_action("sel-delete", ""),
            "escape" => {
                self.close_popovers();
                if let Some(ed) = &mut self.ed {
                    ed.selection = None;
                    ed.dirty = true;
                }
                self.position_overlays();
                self.request_frame();
            }
            "pagedown" | "arrowright" => self.goto_relative(1),
            "pageup" | "arrowleft" => self.goto_relative(-1),
            " " => self.space_down = true,
            _ => return,
        }
        e.prevent_default();
    }
}
