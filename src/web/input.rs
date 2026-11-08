//! Eingabe: S-Pen (Druck, Neigung, Schweben, Seitentaste), Finger-Gesten, Maus, Tastatur.

use super::*;
use crate::engine::InputPoint;
use web_sys::{KeyboardEvent, PointerEvent, WheelEvent};

#[derive(Clone, Copy, Debug)]
pub struct TouchPt {
    pub id: i32,
    pub x: f64,
    pub y: f64,
    pub sx: f64,
    pub sy: f64,
}

/// Laufende Mehrfinger-Geste.
pub struct Gesture {
    pub start_view: Affine,
    pub anchors: Vec<(i32, f64, f64)>,
    pub max_fingers: usize,
    pub t0: f64,
    pub moved: f64,
    pub picker: bool,
}

/// Fließkomma-Koordinaten (web-sys liefert sonst nur ganze CSS-Pixel).
pub fn client_xy(e: &PointerEvent) -> (f64, f64) {
    let get = |k: &str| js_sys::Reflect::get(e, &k.into()).ok().and_then(|v| v.as_f64());
    match (get("clientX"), get("clientY")) {
        (Some(x), Some(y)) => (x, y),
        _ => (e.client_x() as f64, e.client_y() as f64),
    }
}

pub fn bind_input() {
    let stage = el("stage");
    on(&stage, "pointerdown", |e: PointerEvent| {
        e.prevent_default();
        with_app(|a| a.pointer_down(&e));
    });
    on(&stage, "pointermove", |e: PointerEvent| {
        e.prevent_default();
        with_app(|a| a.pointer_move(&e));
    });
    on(&stage, "pointerup", |e: PointerEvent| {
        with_app(|a| a.pointer_up(&e, false));
    });
    on(&stage, "pointercancel", |e: PointerEvent| {
        with_app(|a| a.pointer_up(&e, true));
    });
    on(&stage, "pointerleave", |e: PointerEvent| {
        with_app(|a| {
            if a.stroke.as_ref().map(|s| s.pointer) != Some(e.pointer_id()) {
                a.hide_cursor();
            }
        });
    });
    on(&stage, "wheel", |e: WheelEvent| {
        e.prevent_default();
        with_app(|a| a.wheel(&e));
    });
    // S-Pen-Taste und langes Drücken öffnen sonst ein Kontextmenü
    on(&dom(), "contextmenu", |e: Event| e.prevent_default());
    on(&dom(), "gesturestart", |e: Event| e.prevent_default());
    on(&window(), "keydown", |e: KeyboardEvent| {
        with_app(|a| a.key_down(&e));
    });
    on(&window(), "keyup", |e: KeyboardEvent| {
        if e.code() == "Space" {
            with_app(|a| {
                a.space_down = false;
                set_class(&a.stage, "panning", false);
            });
        }
    });
    on(&window(), "resize", |_: Event| {
        with_app(|a| {
            if a.view_fitted {
                a.fit_view();
            }
        });
    });
}

impl App {
    /// Zeichnen Finger? (Nein, sobald ein S-Pen erkannt wurde und „Nur Stift“ aktiv ist.)
    pub fn touch_draws(&self) -> bool {
        !(self.settings.pen_only && self.pen_seen)
    }

    pub fn input_points(&self, e: &PointerEvent) -> Vec<InputPoint> {
        let list = e.get_coalesced_events();
        let n = list.length();
        if n == 0 {
            vec![self.to_input(e)]
        } else {
            (0..n).map(|i| self.to_input(&list.get(i).unchecked_into::<PointerEvent>())).collect()
        }
    }

    pub fn to_input(&self, e: &PointerEvent) -> InputPoint {
        let (sx, sy) = client_xy(e);
        let inv = self.view.invert();
        let (x, y) = inv.apply(sx, sy);
        let pen = e.pointer_type() == "pen";
        let pressure = if pen { e.pressure().clamp(0.0, 1.0) } else { 1.0 };
        let (mut tx, mut ty) = (0.0f32, 0.0f32);
        if pen {
            let (rx, ry) = (e.tilt_x() as f64, e.tilt_y() as f64);
            let mag = (rx * rx + ry * ry).sqrt();
            if mag > 0.5 {
                // Neigungsrichtung in Dokumentkoordinaten drehen
                let (vx, vy) = inv.apply_vec(rx, ry);
                let vm = (vx * vx + vy * vy).sqrt().max(1e-9);
                tx = (vx / vm * mag) as f32;
                ty = (vy / vm * mag) as f32;
            }
        }
        InputPoint { x: x as f32, y: y as f32, pressure, tilt_x: tx, tilt_y: ty }
    }

    fn pointer_down(&mut self, e: &PointerEvent) {
        if self.open_panel.is_some() {
            self.close_panels();
            return;
        }
        match e.pointer_type().as_str() {
            "pen" => self.pen_event(e, false),
            "touch" => self.touch_down(e),
            _ => self.mouse_down(e),
        }
    }

    fn pointer_move(&mut self, e: &PointerEvent) {
        match e.pointer_type().as_str() {
            "pen" => self.pen_event(e, false),
            "touch" => self.touch_move(e),
            _ => self.mouse_move(e),
        }
    }

    fn pointer_up(&mut self, e: &PointerEvent, cancel: bool) {
        let id = e.pointer_id();
        let _ = self.stage.release_pointer_capture(id);
        match e.pointer_type().as_str() {
            "pen" => {
                self.last_pen_time = now();
                if self.stroke.as_ref().map(|s| s.pointer) == Some(id) {
                    let pts = self.input_points(e);
                    self.stroke_move(&pts);
                    self.end_stroke();
                }
            }
            "touch" => self.touch_up(e, cancel),
            _ => {
                if self.mouse_pan.as_ref().map(|m| m.0) == Some(id) {
                    self.mouse_pan = None;
                }
                if self.stroke.as_ref().map(|s| s.pointer) == Some(id) {
                    self.end_stroke();
                }
            }
        }
    }

    // ----- S-Pen -----

    fn pen_event(&mut self, e: &PointerEvent, _up: bool) {
        let t = now();
        if !self.pen_seen {
            self.pen_seen = true;
            self.save_settings();
            if self.settings.pen_only {
                self.toast("S-Pen erkannt – Finger steuern jetzt nur die Ansicht");
            }
        }
        self.last_pen_time = t;
        // Handballen: laufende Finger-Aktionen abbrechen
        if self.gesture.is_some() || self.stroke.as_ref().map(|s| s.touch).unwrap_or(false) {
            if self.stroke.as_ref().map(|s| s.touch).unwrap_or(false) {
                self.cancel_stroke();
            }
            self.gesture = None;
            self.touches.clear();
        }
        let id = e.pointer_id();
        let buttons = e.buttons();
        let contact = buttons & 1 != 0;
        let barrel = buttons & 2 != 0;
        let eraser_tip = buttons & 32 != 0;
        let is_down = e.type_() == "pointerdown";
        let (sx, sy) = client_xy(e);

        // Taste gedrückt, ohne den Bildschirm zu berühren → Klick-Aktion
        if is_down && !contact && barrel {
            self.pen_button_click();
            return;
        }
        let mine = self.stroke.as_ref().map(|s| s.pointer) == Some(id);
        if contact {
            if self.stroke.is_none() {
                let ov = if eraser_tip {
                    Some(Tool::Eraser)
                } else if barrel {
                    match self.settings.pen_button {
                        PenButton::Eraser => Some(Tool::Eraser),
                        PenButton::Picker => Some(Tool::Picker),
                        _ => None,
                    }
                } else {
                    None
                };
                let _ = self.stage.set_pointer_capture(id);
                let p = self.to_input(e);
                self.begin_stroke(id, p, ov, false);
                if !matches!(self.tool_for(ov), Tool::Eraser | Tool::Smudge) {
                    self.hide_cursor();
                } else {
                    self.show_cursor(sx, sy, ov);
                }
            } else if mine {
                let pts = self.input_points(e);
                self.stroke_move(&pts);
                if self.stroke.as_ref().map(|s| matches!(s.kind, StrokeKind::Erase | StrokeKind::Smudge)).unwrap_or(false) {
                    self.show_cursor(sx, sy, None);
                }
            }
        } else {
            if mine {
                self.end_stroke();
            }
            // Schweben: Pinselvorschau anzeigen
            if self.settings.hover_cursor {
                let ov = if eraser_tip || (barrel && self.settings.pen_button == PenButton::Eraser) { Some(Tool::Eraser) } else { None };
                self.show_cursor(sx, sy, ov);
            }
        }
    }

    fn pen_button_click(&mut self) {
        match self.settings.pen_button {
            PenButton::Undo => self.undo(),
            PenButton::Toggle => {
                let t = if self.tool == Tool::Eraser { Tool::Brush } else { Tool::Eraser };
                self.set_tool(t);
                self.toast(t.name_de());
            }
            _ => {}
        }
    }

    pub fn tool_for(&self, ov: Option<Tool>) -> Tool {
        ov.unwrap_or(self.tool)
    }

    // ----- Pinselvorschau-Kreis -----

    pub fn show_cursor(&mut self, sx: f64, sy: f64, ov: Option<Tool>) {
        let tool = self.tool_for(ov);
        let size = match tool {
            Tool::Brush | Tool::Shape => self.brush.size,
            Tool::Eraser => self.eraser.size,
            Tool::Smudge => self.smudge.size,
            _ => {
                self.hide_cursor();
                return;
            }
        };
        let d = (size as f64 * self.view.zoom()).max(4.0);
        let c = &self.cursor;
        style(c, "width", &format!("{}px", d));
        style(c, "height", &format!("{}px", d));
        style(c, "left", &format!("{}px", sx));
        style(c, "top", &format!("{}px", sy));
        style(c, "background", "transparent");
        style(c, "border-color", "rgba(255,255,255,.95)");
        set_class(c, "picker", false);
        set_class(c, "show", true);
    }

    pub fn show_picker_cursor(&mut self, sx: f64, sy: f64) {
        let c = &self.cursor;
        style(c, "width", "64px");
        style(c, "height", "64px");
        style(c, "left", &format!("{}px", sx));
        style(c, "top", &format!("{}px", sy - 70.0));
        style(c, "background", &self.color.to_hex());
        style(c, "border-color", &self.color.to_hex());
        set_class(c, "picker", true);
        set_class(c, "show", true);
    }

    pub fn hide_cursor(&mut self) {
        set_class(&self.cursor, "show", false);
    }

    // ----- Finger -----

    fn touch_down(&mut self, e: &PointerEvent) {
        let t = now();
        // Handballen-Erkennung: während der Stift aktiv ist (oder gerade war), Finger ignorieren
        if self.stroke.as_ref().map(|s| !s.touch).unwrap_or(false) || (self.pen_seen && t - self.last_pen_time < 150.0) {
            return;
        }
        let (x, y) = client_xy(e);
        let id = e.pointer_id();
        let _ = self.stage.set_pointer_capture(id);
        self.touches.retain(|p| p.id != id);
        self.touches.push(TouchPt { id, x, y, sx: x, sy: y });
        self.hide_cursor();

        if self.touches.len() == 1 && self.touch_draws() {
            let p = self.to_input(e);
            self.begin_stroke(id, p, None, true);
            return;
        }
        // zweiter Finger während eines Finger-Strichs → Strich war vermutlich eine Geste
        if let Some(s) = &self.stroke {
            if s.touch {
                if t - s.start_t < 350.0 {
                    self.cancel_stroke();
                } else {
                    self.end_stroke();
                }
            }
        }
        let start_t = self.gesture.as_ref().map(|g| g.t0).unwrap_or(t);
        let max_f = self.gesture.as_ref().map(|g| g.max_fingers).unwrap_or(0).max(self.touches.len());
        let moved = self.gesture.as_ref().map(|g| g.moved).unwrap_or(0.0);
        self.gesture = Some(Gesture {
            start_view: self.view,
            anchors: self.touches.iter().map(|p| (p.id, p.x, p.y)).collect(),
            max_fingers: max_f,
            t0: start_t,
            moved,
            picker: false,
        });
        if self.touches.len() == 1 {
            // Langes Halten → Pipette
            set_timeout(450, || {
                with_app(|a| a.check_long_press());
            });
        }
    }

    fn check_long_press(&mut self) {
        let ok = matches!(&self.gesture, Some(g) if g.max_fingers == 1 && g.moved < 10.0 && !g.picker) && self.touches.len() == 1;
        if ok {
            if let Some(g) = &mut self.gesture {
                g.picker = true;
            }
            let p = self.touches[0];
            self.pick_screen(p.x, p.y);
            let _ = window().navigator().vibrate_with_duration(12);
        }
    }

    fn pick_screen(&mut self, sx: f64, sy: f64) {
        let (x, y) = self.screen_to_doc(sx, sy);
        if let Some(c) = self.doc.sample(x.floor() as i32, y.floor() as i32) {
            self.set_color(c, false);
        }
        self.show_picker_cursor(sx, sy);
    }

    fn touch_move(&mut self, e: &PointerEvent) {
        let id = e.pointer_id();
        let (x, y) = client_xy(e);
        let Some(tp) = self.touches.iter_mut().find(|p| p.id == id) else { return };
        tp.x = x;
        tp.y = y;
        let d = ((x - tp.sx).powi(2) + (y - tp.sy).powi(2)).sqrt();
        if self.stroke.as_ref().map(|s| s.pointer) == Some(id) {
            let pts = self.input_points(e);
            self.stroke_move(&pts);
            return;
        }
        if let Some(g) = &mut self.gesture {
            g.moved = g.moved.max(d);
            if g.picker {
                self.pick_screen(x, y);
                return;
            }
        }
        self.update_gesture();
    }

    fn update_gesture(&mut self) {
        let Some(g) = &self.gesture else { return };
        let cur: Vec<(f64, f64, f64, f64)> = g
            .anchors
            .iter()
            .filter_map(|(id, ax, ay)| self.touches.iter().find(|p| p.id == *id).map(|p| (*ax, *ay, p.x, p.y)))
            .collect();
        let start = g.start_view;
        let new_view = match cur.len() {
            0 => return,
            1 => {
                let (ax, ay, x, y) = cur[0];
                Affine::translate(x - ax, y - ay).mul(&start)
            }
            _ => {
                let (p1x, p1y, q1x, q1y) = cur[0];
                let (p2x, p2y, q2x, q2y) = cur[1];
                let (dpx, dpy) = (p2x - p1x, p2y - p1y);
                let (dqx, dqy) = (q2x - q1x, q2y - q1y);
                let lp = (dpx * dpx + dpy * dpy).sqrt().max(1.0);
                let lq = (dqx * dqx + dqy * dqy).sqrt().max(1.0);
                let z0 = start.zoom();
                let s = (lq / lp).clamp(0.02 / z0, 64.0 / z0);
                let mut th = dqy.atan2(dqx) - dpy.atan2(dpx);
                // Einrasten bei 0°/90°/180°/270°
                let total = start.angle() + th;
                let q = std::f64::consts::FRAC_PI_2;
                let snapped = (total / q).round() * q;
                if (total - snapped).abs() < 0.06 {
                    th += snapped - total;
                }
                let (pmx, pmy) = ((p1x + p2x) * 0.5, (p1y + p2y) * 0.5);
                let (qmx, qmy) = ((q1x + q2x) * 0.5, (q1y + q2y) * 0.5);
                let gm = Affine::translate(qmx, qmy).mul(&Affine::rotate(th)).mul(&Affine::scale(s)).mul(&Affine::translate(-pmx, -pmy));
                gm.mul(&start)
            }
        };
        self.view = new_view;
        self.view_fitted = false;
        self.apply_view();
    }

    fn touch_up(&mut self, e: &PointerEvent, cancel: bool) {
        let id = e.pointer_id();
        let was = self.touches.len();
        self.touches.retain(|p| p.id != id);
        if was == self.touches.len() {
            return;
        }
        if self.stroke.as_ref().map(|s| s.pointer) == Some(id) {
            if cancel {
                self.cancel_stroke();
            } else {
                self.end_stroke();
            }
            return;
        }
        if self.touches.is_empty() {
            if let Some(g) = self.gesture.take() {
                let dt = now() - g.t0;
                if g.picker {
                    self.hide_cursor();
                    self.push_recent(self.color);
                } else if !cancel && dt < 300.0 && g.moved < 14.0 {
                    match g.max_fingers {
                        2 => self.undo(),
                        3 => self.redo(),
                        _ => {}
                    }
                }
            }
        } else if let Some(g) = &mut self.gesture {
            // Neue Ausgangslage mit den verbleibenden Fingern
            g.start_view = self.view;
            g.anchors = self.touches.iter().map(|p| (p.id, p.x, p.y)).collect();
        }
    }

    // ----- Maus -----

    fn mouse_down(&mut self, e: &PointerEvent) {
        let id = e.pointer_id();
        let (x, y) = client_xy(e);
        let _ = self.stage.set_pointer_capture(id);
        if e.button() == 1 || (e.button() == 0 && self.space_down) {
            self.mouse_pan = Some((id, x, y, self.view));
            return;
        }
        let ov = if e.button() == 2 { Some(Tool::Picker) } else { None };
        if e.button() == 0 || e.button() == 2 {
            let p = self.to_input(e);
            self.begin_stroke(id, p, ov, false);
        }
    }

    fn mouse_move(&mut self, e: &PointerEvent) {
        let (x, y) = client_xy(e);
        if let Some((id, sx, sy, v)) = self.mouse_pan {
            if id == e.pointer_id() {
                self.view = Affine::translate(x - sx, y - sy).mul(&v);
                self.view_fitted = false;
                self.apply_view();
                return;
            }
        }
        if self.stroke.as_ref().map(|s| s.pointer) == Some(e.pointer_id()) {
            let pts = self.input_points(e);
            self.stroke_move(&pts);
        }
        if self.space_down {
            self.hide_cursor();
        } else {
            self.show_cursor(x, y, None);
        }
    }

    fn wheel(&mut self, e: &WheelEvent) {
        let (x, y) = (e.client_x() as f64, e.client_y() as f64);
        let mut dy = e.delta_y();
        if e.delta_mode() == 1 {
            dy *= 32.0;
        }
        let z0 = self.view.zoom();
        let f = (-dy * 0.0015).exp().clamp(0.02 / z0, 64.0 / z0);
        self.view = Affine::translate(x, y).mul(&Affine::scale(f)).mul(&Affine::translate(-x, -y)).mul(&self.view);
        self.view_fitted = false;
        self.apply_view();
    }

    // ----- Tastatur -----

    fn key_down(&mut self, e: &KeyboardEvent) {
        if let Some(t) = e.target() {
            if let Some(el) = t.dyn_ref::<web_sys::Element>() {
                let tag = el.tag_name();
                if tag == "INPUT" || tag == "SELECT" || tag == "TEXTAREA" {
                    return;
                }
            }
        }
        let ctrl = e.ctrl_key() || e.meta_key();
        let key = e.key().to_lowercase();
        if ctrl {
            match key.as_str() {
                "z" if e.shift_key() => self.redo(),
                "z" => self.undo(),
                "y" => self.redo(),
                "s" => self.download_project(),
                _ => return,
            }
            e.prevent_default();
            return;
        }
        match key.as_str() {
            "b" => self.set_tool(Tool::Brush),
            "e" => self.set_tool(Tool::Eraser),
            "s" => self.set_tool(Tool::Smudge),
            "g" => self.set_tool(Tool::Fill),
            "i" => self.set_tool(Tool::Picker),
            "u" => self.set_tool(Tool::Shape),
            "v" => self.set_tool(Tool::Move),
            "l" => self.toggle_panel("panel-layers"),
            "c" => self.toggle_panel("panel-color"),
            "0" => self.fit_view(),
            "[" => self.nudge_size(1.0 / 1.15),
            "]" => self.nudge_size(1.15),
            "escape" => self.close_panels(),
            " " => {
                if !e.repeat() {
                    self.space_down = true;
                    set_class(&self.stage, "panning", true);
                }
            }
            _ => return,
        }
        e.prevent_default();
    }

    fn nudge_size(&mut self, f: f32) {
        let s = self.slot_mut();
        s.size = (s.size * f).clamp(1.0, 1000.0);
        self.refresh_side();
        self.save_settings();
    }
}
