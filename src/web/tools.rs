//! Werkzeuge: Malen, Radieren, Wischen, Formen, Lasso, Füllen, Pipette, Verschieben, QuickShape.

use super::*;
use crate::engine::brush::render_dab;
use crate::engine::fill::{fill_polygon, flood_fill};
use crate::engine::smudge::Smudger;
use crate::engine::stroke::{detect_quickshape, ellipse_points, QuickShape};
use crate::engine::{Action, InputPoint, StrokeEngine};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeKind {
    Paint,
    Erase,
    Smudge,
    Shape,
    Lasso,
    Move,
    Picker,
}

pub struct Stroke {
    pub pointer: i32,
    pub kind: StrokeKind,
    pub layer: usize,
    pub engine: StrokeEngine,
    pub color: Color,
    pub opacity: f32,
    pub start: (f32, f32),
    pub cur: (f32, f32),
    pub start_t: f64,
    pub hold_t: f64,
    pub hold_pos: (f32, f32),
    pub quick: Option<QuickShape>,
    pub quick_pressure: f32,
    pub lasso: Vec<(f32, f32)>,
    pub smudger: Option<Smudger>,
    pub touch: bool,
    /// Inhaltsbereich der Ebene beim Verschieben
    pub move_bounds: IRect,
}

impl Stroke {
    pub fn move_offset(&self) -> (i32, i32) {
        ((self.cur.0 - self.start.0).round() as i32, (self.cur.1 - self.start.1).round() as i32)
    }
}

impl App {
    pub fn slot(&self) -> &Slot {
        match self.tool {
            Tool::Eraser => &self.eraser,
            Tool::Smudge => &self.smudge,
            _ => &self.brush,
        }
    }

    pub fn slot_mut(&mut self) -> &mut Slot {
        match self.tool {
            Tool::Eraser => &mut self.eraser,
            Tool::Smudge => &mut self.smudge,
            _ => &mut self.brush,
        }
    }

    fn slot_for(&self, t: Tool) -> Slot {
        match t {
            Tool::Eraser => self.eraser,
            Tool::Smudge => self.smudge,
            _ => self.brush,
        }
    }

    fn make_engine(&self, slot: &Slot, smoothing: bool) -> StrokeEngine {
        StrokeEngine::new(
            self.presets[slot.preset].clone(),
            slot.size,
            self.settings.gamma,
            if smoothing { self.settings.smoothing } else { 0.0 },
            self.settings.symmetry,
            self.doc.w as f32 * 0.5,
            self.doc.h as f32 * 0.5,
        )
    }

    /// Prüft, ob auf der aktiven Ebene gemalt werden darf.
    fn layer_editable(&mut self) -> bool {
        let l = &self.doc.active_layer().props;
        if l.locked {
            self.toast("Ebene ist gesperrt");
            false
        } else if !l.visible {
            self.toast("Ebene ist ausgeblendet");
            false
        } else {
            true
        }
    }

    pub fn begin_stroke(&mut self, pointer: i32, p: InputPoint, ov: Option<Tool>, touch: bool) {
        if self.stroke.is_some() {
            return;
        }
        let tool = self.tool_for(ov);
        let t = now();
        let layer = self.doc.active;
        let kind = match tool {
            Tool::Picker => StrokeKind::Picker,
            Tool::Fill => {
                self.fill_at(p.x, p.y);
                return;
            }
            Tool::Move => StrokeKind::Move,
            Tool::Brush => StrokeKind::Paint,
            Tool::Eraser => StrokeKind::Erase,
            Tool::Smudge => StrokeKind::Smudge,
            Tool::Shape => {
                if self.settings.shape == ShapeKind::Lasso {
                    StrokeKind::Lasso
                } else {
                    StrokeKind::Shape
                }
            }
        };
        if kind != StrokeKind::Picker && !self.layer_editable() {
            return;
        }
        let slot = self.slot_for(tool);
        let engine = self.make_engine(&slot, matches!(kind, StrokeKind::Paint | StrokeKind::Erase | StrokeKind::Smudge));
        let copies = engine.symmetry_count();
        self.mask.clear();
        let mut s = Stroke {
            pointer,
            kind,
            layer,
            engine,
            color: self.color,
            opacity: slot.opacity,
            start: (p.x, p.y),
            cur: (p.x, p.y),
            start_t: t,
            hold_t: t,
            hold_pos: (p.x, p.y),
            quick: None,
            quick_pressure: 1.0,
            lasso: vec![(p.x, p.y)],
            smudger: None,
            touch,
            move_bounds: IRect::EMPTY,
        };
        if kind == StrokeKind::Move {
            s.move_bounds = self.doc.content_bounds(layer);
        }
        if kind == StrokeKind::Smudge {
            let data = &self.doc.layers[layer].data;
            self.smudge_snapshot.clear();
            self.smudge_snapshot.extend_from_slice(data);
            s.smudger = Some(Smudger::new(copies));
        }
        set_class(&dom().body().unwrap(), "drawing", true);
        let mut dabs = std::mem::take(&mut self.dabs);
        dabs.clear();
        match kind {
            StrokeKind::Paint | StrokeKind::Erase | StrokeKind::Smudge => s.engine.begin(p, &mut dabs),
            StrokeKind::Picker => {}
            _ => {}
        }
        self.stroke = Some(s);
        self.apply_dabs(&dabs);
        self.dabs = dabs;
        if kind == StrokeKind::Picker {
            self.pick_doc(p.x, p.y);
        }
        self.request_frame();
    }

    /// Stempel in Maske bzw. (Wischfinger) direkt in die Ebene schreiben.
    fn apply_dabs(&mut self, dabs: &[crate::engine::Dab]) {
        if dabs.is_empty() {
            return;
        }
        let Some(s) = &mut self.stroke else { return };
        let mut r = IRect::EMPTY;
        match s.kind {
            StrokeKind::Smudge => {
                let (w, h) = (self.doc.w, self.doc.h);
                let strength = s.opacity.clamp(0.02, 1.0);
                if let Some(sm) = &mut s.smudger {
                    let data = &mut self.doc.layers[s.layer].data;
                    for d in dabs {
                        sm.dab(data, w, h, d, strength);
                        r = r.union(&d.bounds());
                    }
                }
            }
            _ => {
                let buildup = s.engine.preset.buildup;
                for d in dabs {
                    render_dab(&mut self.mask, d, buildup, &self.noise);
                    r = r.union(&d.bounds());
                }
            }
        }
        self.invalidate(r);
    }

    pub fn stroke_move(&mut self, pts: &[InputPoint]) {
        let Some(kind) = self.stroke.as_ref().map(|s| s.kind) else { return };
        let Some(last) = pts.last().copied() else { return };
        let hold_thr = (5.0 / self.view.zoom()) as f32;
        match kind {
            StrokeKind::Paint | StrokeKind::Erase | StrokeKind::Smudge => {
                let quick = self.stroke.as_ref().and_then(|s| s.quick);
                if let Some(q) = quick {
                    if let QuickShape::Line { .. } = q {
                        if let Some(s) = &mut self.stroke {
                            s.cur = (last.x, last.y);
                        }
                        self.render_quickshape();
                    }
                    return;
                }
                let mut dabs = std::mem::take(&mut self.dabs);
                dabs.clear();
                let t = now();
                if let Some(s) = &mut self.stroke {
                    for p in pts {
                        s.engine.add(*p, &mut dabs);
                        let (hx, hy) = s.hold_pos;
                        if ((p.x - hx).powi(2) + (p.y - hy).powi(2)).sqrt() > hold_thr {
                            s.hold_pos = (p.x, p.y);
                            s.hold_t = t;
                        }
                    }
                    s.cur = (last.x, last.y);
                }
                self.apply_dabs(&dabs);
                self.dabs = dabs;
            }
            StrokeKind::Shape => {
                if let Some(s) = &mut self.stroke {
                    s.cur = (last.x, last.y);
                }
                self.render_shape();
            }
            StrokeKind::Lasso => {
                if let Some(s) = &mut self.stroke {
                    for p in pts {
                        let l = *s.lasso.last().unwrap();
                        if ((p.x - l.0).powi(2) + (p.y - l.1).powi(2)).sqrt() > 1.5 {
                            s.lasso.push((p.x, p.y));
                        }
                    }
                    s.cur = (last.x, last.y);
                }
                self.render_lasso();
            }
            StrokeKind::Move => {
                let Some(s) = &mut self.stroke else { return };
                let (ox, oy) = s.move_offset();
                s.cur = (last.x, last.y);
                let (nx, ny) = s.move_offset();
                if (ox, oy) != (nx, ny) {
                    // nur alten und neuen Bereich des Ebeneninhalts neu zeichnen
                    let b = s.move_bounds;
                    let shift = |dx: i32, dy: i32| IRect::new(b.x0 + dx, b.y0 + dy, b.x1 + dx, b.y1 + dy);
                    let r = shift(ox, oy).union(&shift(nx, ny));
                    self.invalidate(r);
                }
            }
            StrokeKind::Picker => self.pick_doc(last.x, last.y),
        }
    }

    fn pick_doc(&mut self, x: f32, y: f32) {
        if let Some(c) = self.doc.sample(x.floor() as i32, y.floor() as i32) {
            self.set_color(c, false);
        }
        let (sx, sy) = self.view.apply(x as f64, y as f64);
        self.show_picker_cursor(sx, sy);
    }

    /// Übernimmt die Maske als Patch in die Ebene (mit Rückgängig-Eintrag).
    fn commit_mask(&mut self, kind: StrokeKind, layer: usize, color: Color, opacity: f32) {
        let r = self.mask.dirty.clamp(self.doc.w, self.doc.h);
        if r.is_empty() {
            return;
        }
        let before = self.doc.read_region(layer, r);
        {
            let ov = match kind {
                StrokeKind::Erase => Overlay::Erase { layer, mask: &self.mask, opacity },
                _ => Overlay::Paint { layer, mask: &self.mask, color, opacity },
            };
            self.doc.commit_overlay(&ov);
        }
        let after = self.doc.read_region(layer, r);
        let id = self.doc.layers[layer].id;
        self.history.push(Action::patch(id, r, &before, &after));
        self.mask.clear();
        self.invalidate(r);
    }

    pub fn end_stroke(&mut self) {
        set_class(&dom().body().unwrap(), "drawing", false);
        let Some(mut s) = self.stroke.take() else { return };
        match s.kind {
            StrokeKind::Paint | StrokeKind::Erase => {
                if s.quick.is_none() {
                    let mut dabs = std::mem::take(&mut self.dabs);
                    dabs.clear();
                    s.engine.end(&mut dabs);
                    let buildup = s.engine.preset.buildup;
                    for d in &dabs {
                        render_dab(&mut self.mask, d, buildup, &self.noise);
                    }
                    self.dabs = dabs;
                }
                self.commit_mask(s.kind, s.layer, s.color, s.opacity);
                if s.kind == StrokeKind::Paint {
                    self.push_recent(s.color);
                }
            }
            StrokeKind::Shape | StrokeKind::Lasso => {
                self.stroke = Some(s);
                if self.stroke.as_ref().unwrap().kind == StrokeKind::Lasso {
                    self.render_lasso();
                }
                let s = self.stroke.take().unwrap();
                self.commit_mask(StrokeKind::Paint, s.layer, s.color, s.opacity);
                self.push_recent(s.color);
            }
            StrokeKind::Smudge => {
                if let Some(sm) = &s.smudger {
                    let r = sm.touched.clamp(self.doc.w, self.doc.h);
                    if !r.is_empty() {
                        let before = region_of(&self.smudge_snapshot, self.doc.w, r);
                        let after = self.doc.read_region(s.layer, r);
                        let id = self.doc.layers[s.layer].id;
                        self.history.push(Action::patch(id, r, &before, &after));
                        self.doc.layers[s.layer].touch();
                    }
                }
            }
            StrokeKind::Move => {
                let (dx, dy) = s.move_offset();
                if dx != 0 || dy != 0 {
                    let full = self.doc.bounds();
                    let before = self.doc.read_region(s.layer, full);
                    self.doc.commit_overlay(&Overlay::Offset { layer: s.layer, dx, dy });
                    let after = self.doc.read_region(s.layer, full);
                    let id = self.doc.layers[s.layer].id;
                    self.history.push(Action::patch(id, full, &before, &after));
                }
                let b = s.move_bounds;
                self.invalidate(b.union(&IRect::new(b.x0 + dx, b.y0 + dy, b.x1 + dx, b.y1 + dy)));
            }
            StrokeKind::Picker => {
                self.hide_cursor();
                self.push_recent(self.color);
                return;
            }
        }
        self.after_change();
    }

    /// Bricht den laufenden Strich ohne Änderung ab.
    pub fn cancel_stroke(&mut self) {
        set_class(&dom().body().unwrap(), "drawing", false);
        let Some(s) = self.stroke.take() else { return };
        match s.kind {
            StrokeKind::Smudge => {
                if let Some(sm) = &s.smudger {
                    let r = sm.touched.clamp(self.doc.w, self.doc.h);
                    if !r.is_empty() {
                        let before = region_of(&self.smudge_snapshot, self.doc.w, r);
                        self.doc.write_region(s.layer, r, &before);
                        self.invalidate(r);
                    }
                }
            }
            StrokeKind::Move => {
                let (dx, dy) = s.move_offset();
                let b = s.move_bounds;
                self.invalidate(b.union(&IRect::new(b.x0 + dx, b.y0 + dy, b.x1 + dx, b.y1 + dy)));
            }
            _ => {
                let r = self.mask.dirty;
                self.mask.clear();
                self.invalidate(r);
            }
        }
        self.hide_cursor();
    }

    // ----- Formen -----

    fn shape_points(&self) -> (Vec<(f32, f32)>, bool) {
        let s = self.stroke.as_ref().unwrap();
        let ((x0, y0), (x1, y1)) = (s.start, s.cur);
        match self.settings.shape {
            ShapeKind::Line | ShapeKind::Lasso => (vec![(x0, y0), (x1, y1)], false),
            ShapeKind::Rect => (vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)], true),
            ShapeKind::Ellipse => (ellipse_points((x0 + x1) * 0.5, (y0 + y1) * 0.5, (x1 - x0).abs() * 0.5, (y1 - y0).abs() * 0.5), true),
        }
    }

    fn clear_mask_invalidate(&mut self) {
        let r = self.mask.dirty;
        self.mask.clear();
        self.invalidate(r);
    }

    fn render_shape(&mut self) {
        self.clear_mask_invalidate();
        let (pts, closed) = self.shape_points();
        let fill = self.settings.shape_fill && closed;
        self.render_path(&pts, closed, fill, 1.0);
    }

    fn render_lasso(&mut self) {
        self.clear_mask_invalidate();
        let pts = self.stroke.as_ref().unwrap().lasso.clone();
        self.render_path(&pts, true, true, 1.0);
    }

    fn render_path(&mut self, pts: &[(f32, f32)], closed: bool, fill: bool, pressure: f32) {
        let Some(s) = &mut self.stroke else { return };
        if fill {
            for poly in s.engine.symmetric_polygons(pts) {
                fill_polygon(&mut self.mask, &poly);
            }
        } else {
            let mut dabs = std::mem::take(&mut self.dabs);
            dabs.clear();
            s.engine.path(pts, closed, pressure, &mut dabs);
            let buildup = s.engine.preset.buildup;
            for d in &dabs {
                render_dab(&mut self.mask, d, buildup, &self.noise);
            }
            self.dabs = dabs;
        }
        let r = self.mask.dirty;
        self.invalidate(r);
    }

    // ----- QuickShape -----

    pub fn check_quickshape(&mut self) {
        if !self.settings.quickshape {
            return;
        }
        let zoom = self.view.zoom() as f32;
        let ready = match &self.stroke {
            Some(s) => matches!(s.kind, StrokeKind::Paint | StrokeKind::Erase) && s.quick.is_none() && now() - s.hold_t > 550.0 && s.engine.path_len * zoom > 40.0,
            None => false,
        };
        if !ready {
            return;
        }
        let s = self.stroke.as_mut().unwrap();
        let Some(q) = detect_quickshape(&s.engine.raw) else {
            s.hold_t = f64::MAX / 2.0; // nicht erneut prüfen
            return;
        };
        let n = s.engine.raw.len().max(1) as f32;
        let avg = s.engine.raw.iter().map(|p| p.pressure).sum::<f32>() / n;
        s.quick_pressure = avg.clamp(0.3, 1.0);
        s.quick = Some(q);
        self.render_quickshape();
        let _ = window().navigator().vibrate_with_duration(15);
    }

    fn render_quickshape(&mut self) {
        let Some(s) = &self.stroke else { return };
        let Some(q) = s.quick else { return };
        let pr = s.quick_pressure;
        let cur = s.cur;
        self.clear_mask_invalidate();
        match q {
            QuickShape::Line { x0, y0 } => self.render_path(&[(x0, y0), cur], false, false, pr),
            QuickShape::Ellipse { cx, cy, rx, ry } => {
                let pts = ellipse_points(cx, cy, rx, ry);
                self.render_path(&pts, true, false, pr)
            }
        }
    }

    // ----- Füllen -----

    pub fn fill_at(&mut self, x: f32, y: f32) {
        if !self.layer_editable() {
            return;
        }
        let (w, h) = (self.doc.w, self.doc.h);
        let (ix, iy) = (x.floor() as i32, y.floor() as i32);
        if ix < 0 || iy < 0 || ix >= w as i32 || iy >= h as i32 {
            return;
        }
        let layer = self.doc.active;
        self.mask.clear();
        let r = if self.settings.fill_all {
            let mut src = Vec::new();
            self.doc.composite(self.doc.bounds(), &Overlay::None, false, &mut src);
            flood_fill(&src, w, h, ix, iy, self.settings.fill_tol, 1, &mut self.mask)
        } else {
            let src = &self.doc.layers[layer].data;
            flood_fill(src, w, h, ix, iy, self.settings.fill_tol, 1, &mut self.mask)
        };
        if r.is_empty() {
            return;
        }
        let color = self.color;
        let opacity = self.brush.opacity;
        self.commit_mask(StrokeKind::Paint, layer, color, opacity);
        self.push_recent(color);
        self.after_change();
    }

    // ----- Verlauf -----

    pub fn undo(&mut self) {
        if self.stroke.is_some() {
            return;
        }
        if self.history.undo(&mut self.doc) {
            self.redraw_all();
            self.after_change();
            self.toast("Rückgängig");
        }
    }

    pub fn redo(&mut self) {
        if self.stroke.is_some() {
            return;
        }
        if self.history.redo(&mut self.doc) {
            self.redraw_all();
            self.after_change();
            self.toast("Wiederholt");
        }
    }

    /// Nach jeder Dokumentänderung: Oberfläche und Autospeichern anstoßen.
    pub fn after_change(&mut self) {
        self.ui_layers_dirty = true;
        self.refresh_history_buttons();
        self.mark_unsaved();
        self.request_frame();
    }
}

pub fn region_of(buf: &[u8], w: usize, r: IRect) -> Vec<u8> {
    let mut out = Vec::with_capacity(r.area() * 4);
    for y in r.y0..r.y1 {
        let s = (y as usize * w + r.x0 as usize) * 4;
        out.extend_from_slice(&buf[s..s + r.width() * 4]);
    }
    out
}
