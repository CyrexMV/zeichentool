//! Seiten-Schieberegler am rechten Rand: ziehen = schnell durch das Notizbuch blättern.

use super::*;
use web_sys::PointerEvent;

pub fn bind_scrubber() {
    let s = el("scrubber");
    on(&s, "pointerdown", |e: PointerEvent| {
        e.prevent_default();
        e.stop_propagation();
        let _ = el("scrubber").set_pointer_capture(e.pointer_id());
        with_app(|a| {
            a.scrub_active = true;
            a.scrub_to(e.client_y() as f64);
        });
    });
    on(&s, "pointermove", |e: PointerEvent| {
        e.stop_propagation();
        if e.buttons() & 1 != 0 {
            with_app(|a| {
                if a.scrub_active {
                    a.scrub_to(e.client_y() as f64);
                }
            });
        }
    });
    for ev in ["pointerup", "pointercancel"] {
        on(&s, ev, |e: PointerEvent| {
            e.stop_propagation();
            with_app(|a| {
                a.scrub_active = false;
                set_class(&el("scrubber"), "dragging", false);
            });
        });
    }
}

impl App {
    /// Bereich der Leiste (oben, unten) in Bildschirmpixeln.
    fn scrub_track(&self) -> (f64, f64) {
        let r = el("scrubber").get_bounding_client_rect();
        (r.top() + 24.0, r.bottom() - 24.0)
    }

    fn scrub_to(&mut self, y: f64) {
        let (t, b) = self.scrub_track();
        let Some(ed) = &mut self.ed else { return };
        let n = ed.nb.pages.len();
        if n < 2 {
            return;
        }
        let f = ((y - t) / (b - t).max(1.0)).clamp(0.0, 1.0);
        let i = (f * (n - 1) as f64).round() as usize;
        if i != ed.cur_page || !el("scrubber").class_list().contains("dragging") {
            ed.goto_page(i);
            ed.cur_page = i;
        }
        set_class(&el("scrubber"), "dragging", true);
        self.after_view_change();
    }

    /// Daumen und Vorschau-Label an die aktuelle Seite anpassen.
    pub fn update_scrubber(&mut self) {
        let sc = el("scrubber");
        let Some(ed) = &self.ed else { return };
        let n = ed.nb.pages.len();
        let visible = n >= 3 && ed.presenting.is_none() && ed.zoomwin.is_none();
        set_class(&sc, "hidden", !visible);
        if !visible {
            return;
        }
        let i = ed.cur_page.min(n - 1);
        let f = i as f64 / (n - 1) as f64;
        style(&el("scrub-thumb"), "top", &format!("calc(24px + (100% - 48px) * {:.4})", f));
        let p = &ed.nb.pages[i];
        let label = if p.title.is_empty() { format!("Seite {} / {}", i + 1, n) } else { format!("{} / {} · {}", i + 1, n, p.title.chars().take(28).collect::<String>()) };
        set_text("scrub-label", &label);
    }
}
