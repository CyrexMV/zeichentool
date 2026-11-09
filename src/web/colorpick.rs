//! Eigener Farbwähler (statt des System-Dialogs): Farbfeld, Farbton-Leiste, Hex-Eingabe, zuletzt benutzte Farben.

use super::*;
use web_sys::PointerEvent;

const RECENT_MAX: usize = 12;

fn knob_pos(id: &str, x: f32, y: Option<f32>) {
    let k = el(id);
    style(&k, "left", &format!("{:.2}%", x * 100.0));
    if let Some(y) = y {
        style(&k, "top", &format!("{:.2}%", y * 100.0));
    }
}

/// Relativposition (0..1) eines Zeigers in einem Element.
fn rel(e: &PointerEvent, id: &str) -> (f32, f32) {
    let r = el(id).get_bounding_client_rect();
    let (x, y) = (e.client_x() as f64, e.client_y() as f64);
    (((x - r.left()) / r.width().max(1.0)).clamp(0.0, 1.0) as f32, ((y - r.top()) / r.height().max(1.0)).clamp(0.0, 1.0) as f32)
}

pub fn bind_color_picker() {
    for id in ["cp-sv", "cp-hue"] {
        let target = el(id);
        on(&target, "pointerdown", move |e: PointerEvent| {
            e.prevent_default();
            let _ = el(id).set_pointer_capture(e.pointer_id());
            with_app(|a| a.cp_drag(id, &e));
        });
        on(&target, "pointermove", move |e: PointerEvent| {
            if e.buttons() & 1 != 0 {
                with_app(|a| a.cp_drag(id, &e));
            }
        });
    }
}

impl App {
    pub fn open_color_picker(&mut self) {
        let p = &self.prefs;
        let cur = match self.tool {
            Tool::Highlighter => p.hl_colors[p.hl_ci],
            Tool::Text => p.text_colors[p.text_ci],
            _ => p.pen_colors[p.pen_ci],
        };
        let c = Color::from_u8(cur[0], cur[1], cur[2]);
        self.cp_old = c;
        self.cp = c.to_hsv();
        style(&el("cp-old"), "background", &c.to_hex());
        let recent: String = self
            .recent_colors
            .iter()
            .map(|h| format!("<button class=\"sw\" data-act=\"cp-recent\" data-arg=\"{h}\" style=\"background:{h}\" title=\"{h}\"></button>", h = esc(h)))
            .collect();
        el("cp-recent").set_inner_html(&recent);
        show("cp-recent-wrap", !self.recent_colors.is_empty());
        self.open_modal("modal-color");
        self.cp_refresh(true);
    }

    fn cp_color(&self) -> Color {
        Color::from_hsv(self.cp.0, self.cp.1, self.cp.2)
    }

    /// Anzeige aktualisieren; `hex` = auch das Eingabefeld überschreiben.
    pub fn cp_refresh(&mut self, hex: bool) {
        let (h, s, v) = self.cp;
        let c = self.cp_color();
        style(&el("cp-sv"), "background-color", &Color::from_hsv(h, 1.0, 1.0).to_hex());
        knob_pos("cp-sv-knob", s, Some(1.0 - v));
        knob_pos("cp-hue-knob", h / 360.0, None);
        style(&el("cp-sv-knob"), "background", &c.to_hex());
        style(&el("cp-hue-knob"), "background", &Color::from_hsv(h, 1.0, 1.0).to_hex());
        style(&el("cp-new"), "background", &c.to_hex());
        if hex {
            input_el("cp-hex").set_value(c.to_hex().trim_start_matches('#'));
        }
    }

    fn cp_drag(&mut self, id: &str, e: &PointerEvent) {
        let (x, y) = rel(e, id);
        if id == "cp-hue" {
            self.cp.0 = (x * 360.0).min(359.9);
        } else {
            self.cp.1 = x;
            self.cp.2 = 1.0 - y;
        }
        self.cp_refresh(true);
    }

    pub fn cp_hex_input(&mut self, value: &str) {
        let v = value.trim().trim_start_matches('#');
        if v.len() == 6 {
            if let Some(c) = Color::from_hex(&format!("#{}", v)) {
                self.cp = c.to_hsv();
                self.cp_refresh(false);
            }
        }
    }

    pub fn cp_pick(&mut self, hex: &str) {
        if let Some(c) = Color::from_hex(hex) {
            self.cp = c.to_hsv();
            self.cp_refresh(true);
        }
    }

    pub fn cp_ok(&mut self) {
        let c = self.cp_color();
        let hex = c.to_hex();
        self.recent_colors.retain(|h| !h.eq_ignore_ascii_case(&hex));
        self.recent_colors.insert(0, hex);
        self.recent_colors.truncate(RECENT_MAX);
        self.close_modals();
        self.set_active_color_pub(c);
    }
}
