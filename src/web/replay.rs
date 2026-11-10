//! Zeitraffer: eine Seite zeichnet sich Strich für Strich selbst nach – in der App oder als Video.

use super::render::{draw_page, Caches, DrawOpts};
use super::*;
use crate::engine::{Content, Page};
use js_sys::{Function, Promise, Reflect, Uint8Array};
use wasm_bindgen_futures::{spawn_local, JsFuture};

/// Wiedergabe-Zustand (im Editor).
#[derive(Clone, Copy, Debug)]
pub struct Replay {
    pub page: u32,
    /// Fortschritt 0..1
    pub pos: f64,
    pub playing: bool,
    /// 1, 2 oder 4
    pub speed: f64,
    /// Dauer bei 1× (ms)
    pub dur: f64,
    pub last_t: f64,
}

/// Gewicht eines Elements für die Wiedergabe (Striche nach Länge, Rest kurz).
fn weight(c: &Content) -> f64 {
    match c {
        Content::Stroke(s) if !s.is_area() => (s.pts.len() as f64).clamp(3.0, 400.0),
        Content::Stroke(_) => 10.0,
        _ => 14.0,
    }
}

/// Fortschritt → (Index des wachsenden Elements, Anteil).
pub fn limit_for(page: &Page, pos: f64) -> Option<(usize, f32)> {
    if page.items.is_empty() {
        return None;
    }
    let total: f64 = page.items.iter().map(|i| weight(&i.content)).sum();
    let mut target = pos.clamp(0.0, 1.0) * total;
    for (k, it) in page.items.iter().enumerate() {
        let w = weight(&it.content);
        if target < w {
            return Some((k, (target / w) as f32));
        }
        target -= w;
    }
    Some((page.items.len() - 1, 1.0))
}

/// Dauer bei 1×: etwa 7 ms pro Punkt, zwischen 4 und 30 Sekunden.
fn duration_for(page: &Page) -> f64 {
    let total: f64 = page.items.iter().map(|i| weight(&i.content)).sum();
    (total * 7.0).clamp(4000.0, 30000.0)
}

impl App {
    pub fn replay_start(&mut self) {
        self.close_popovers();
        if self.present.is_some() {
            self.present_exit();
        }
        let Some(ed) = &mut self.ed else { return };
        if ed.text_edit.is_some() {
            self.commit_text();
        }
        let Some(ed) = &mut self.ed else { return };
        let i = ed.cur_page;
        let page = &ed.nb.pages[i];
        if page.items.is_empty() {
            self.toast("Auf dieser Seite ist noch nichts geschrieben");
            return;
        }
        ed.selection = None;
        ed.op = None;
        ed.replay = Some(Replay { page: page.id, pos: 0.0, playing: true, speed: 1.0, dur: duration_for(page), last_t: now() });
        ed.fit_page(i);
        ed.dirty = true;
        set_class(&el("editor"), "replaying", true);
        show("sel-menu", false);
        self.replay_ui();
        self.after_view_change();
    }

    pub fn replay_stop(&mut self) {
        if let Some(ed) = &mut self.ed {
            ed.replay = None;
            ed.bmps.clear();
            ed.dirty = true;
        }
        set_class(&el("editor"), "replaying", false);
        self.request_frame();
    }

    /// Steuerleiste an den Zustand anpassen.
    pub fn replay_ui(&self) {
        let Some(r) = self.ed.as_ref().and_then(|e| e.replay) else { return };
        set_class(&el("replay-play"), "paused", !r.playing);
        let _ = el("replay-play").set_attribute("title", if r.playing { "Pause" } else { "Abspielen" });
        set_text("replay-speed", &format!("{}×", r.speed as u32));
        input_el("replay-pos").set_value(&format!("{:.0}", r.pos * 1000.0));
        let secs = (r.dur / r.speed / 1000.0).round() as u32;
        set_text("replay-time", &format!("{}:{:02}", secs / 60, secs % 60));
    }

    /// Ein Wiedergabe-Schritt pro Bild; `true`, solange es weiterläuft.
    pub fn replay_tick(&mut self, t: f64) -> bool {
        let Some(ed) = &mut self.ed else { return false };
        let Some(r) = &mut ed.replay else { return false };
        if !r.playing {
            r.last_t = t;
            return false;
        }
        let dt = (t - r.last_t).clamp(0.0, 100.0);
        r.last_t = t;
        r.pos = (r.pos + dt * r.speed / r.dur).min(1.0);
        if r.pos >= 1.0 {
            r.playing = false;
        }
        ed.dirty = true;
        let pos = r.pos;
        input_el("replay-pos").set_value(&format!("{:.0}", pos * 1000.0));
        if pos >= 1.0 {
            self.replay_ui();
        }
        true
    }

    pub fn replay_action(&mut self, act: &str) {
        let now_t = now();
        match act {
            "replay" => {
                self.replay_start();
                return;
            }
            "replay-close" => {
                self.replay_stop();
                return;
            }
            "replay-video" | "replay-share" => {
                self.replay_video(act == "replay-share");
                return;
            }
            _ => {}
        }
        let Some(ed) = &mut self.ed else { return };
        let Some(r) = &mut ed.replay else { return };
        match act {
            "replay-play" => {
                if r.pos >= 1.0 {
                    r.pos = 0.0;
                }
                r.playing = !r.playing;
                r.last_t = now_t;
            }
            "replay-restart" => {
                r.pos = 0.0;
                r.playing = true;
                r.last_t = now_t;
            }
            "replay-speed" => {
                r.speed = if r.speed >= 4.0 { 1.0 } else { r.speed * 2.0 };
            }
            _ => {}
        }
        ed.dirty = true;
        self.replay_ui();
        self.request_frame();
    }

    /// Schieberegler: an eine Stelle springen.
    pub fn replay_seek(&mut self, v: f64) {
        let Some(ed) = &mut self.ed else { return };
        let Some(r) = &mut ed.replay else { return };
        r.pos = (v / 1000.0).clamp(0.0, 1.0);
        r.playing = false;
        ed.dirty = true;
        self.replay_ui();
        self.request_frame();
    }

    /// Zeitraffer als Video (WebM) aufnehmen und speichern bzw. teilen.
    fn replay_video(&mut self, share: bool) {
        let Some(ed) = &self.ed else { return };
        let Some(r) = ed.replay else { return };
        let Some(i) = ed.page_index(r.page) else { return };
        let (pw, ph) = ed.nb.page_size(i);
        let Ok(rec) = Reflect::get(&window(), &"notionRec".into()) else { return };
        if rec.is_undefined() {
            self.toast("Video-Aufnahme wird hier nicht unterstützt");
            return;
        }
        // Bildgröße: lange Kante 1280 px (gerade Zahlen für den Encoder)
        let scale = 1280.0 / pw.max(ph) as f64;
        let (cw, ch) = (((pw as f64 * scale) as u32) & !1, ((ph as f64 * scale) as u32) & !1);
        let (canvas, _) = super::editor::new_canvas();
        canvas.set_width(cw);
        canvas.set_height(ch);
        let start = Reflect::get(&rec, &"start".into()).ok().and_then(|f| f.dyn_into::<Function>().ok());
        let Some(start) = start else { return };
        let Ok(handle) = start.call2(&rec, &canvas, &JsValue::from(30)) else {
            self.toast("Video-Aufnahme wird hier nicht unterstützt");
            return;
        };
        if handle.is_null() || handle.is_undefined() {
            self.toast("Video-Aufnahme wird hier nicht unterstützt");
            return;
        }
        // Länge: wie die Wiedergabe beim gewählten Tempo, höchstens 20 s; 1 s Standbild am Ende
        let secs = (r.dur / r.speed / 1000.0).clamp(3.0, 20.0);
        let frames = (secs * 30.0) as u32;
        let title = ed.nb.title.clone();
        let page_no = i + 1;
        if let Some(e) = &mut self.ed {
            if let Some(rr) = &mut e.replay {
                rr.playing = false;
            }
        }
        show("busy", true);
        set_text("busy-text", "Video wird aufgenommen …");
        let canvas2 = canvas.clone();
        // Bild für Bild zeichnen (im Takt der Aufnahme)
        #[allow(clippy::too_many_arguments)]
        fn step(k: u32, frames: u32, canvas: web_sys::HtmlCanvasElement, rec: JsValue, handle: JsValue, scale: f64, title: String, page_no: usize, share: bool) {
            let done = with_app(|a| {
                let images = &mut a.images;
                let Some(ed) = &mut a.ed else { return true };
                let Some(r) = ed.replay else { return true };
                let Some(i) = ed.page_index(r.page) else { return true };
                let (pw, ph) = ed.nb.page_size(i);
                let Ok(Some(ctx)) = canvas.get_context("2d") else { return true };
                let ctx: web_sys::CanvasRenderingContext2d = ctx.unchecked_into();
                let _ = ctx.set_transform(scale, 0.0, 0.0, scale, 0.0, 0.0);
                let pos = (k as f64 / frames as f64).min(1.0);
                let page = &ed.nb.pages[i];
                let limit = limit_for(page, pos);
                let mut c = Caches { paths: &mut ed.paths, tpl: &mut ed.tpl_cache, wraps: &mut ed.wraps, images };
                draw_page(&ctx, page, pw, ph, &mut c, &DrawOpts { skip: None, moving: None, clip: crate::engine::Rect::new(0.0, 0.0, pw, ph), limit });
                set_text("busy-text", &format!("Video wird aufgenommen … {} %", (pos * 100.0).round()));
                false
            })
            .unwrap_or(true);
            if let Ok(f) = Reflect::get(&rec, &"frame".into()).and_then(|f| f.dyn_into::<Function>()) {
                let _ = f.call1(&rec, &handle);
            }
            // nach dem letzten Bild noch 1 s stehen lassen
            if !done && k < frames + 30 {
                set_timeout(33, move || step(k + 1, frames, canvas, rec, handle, scale, title, page_no, share));
                return;
            }
            spawn_local(async move {
                let stop = Reflect::get(&rec, &"stop".into()).ok().and_then(|f| f.dyn_into::<Function>().ok());
                let res = match stop.map(|f| f.call1(&rec, &handle)) {
                    Some(Ok(p)) => JsFuture::from(Promise::from(p)).await.ok(),
                    _ => None,
                };
                with_app(|a| {
                    show("busy", false);
                    let Some(v) = res else {
                        a.toast("Video konnte nicht erstellt werden");
                        return;
                    };
                    let bytes = Uint8Array::new(&Reflect::get(&v, &"bytes".into()).unwrap_or(JsValue::NULL)).to_vec();
                    let mime = Reflect::get(&v, &"mime".into()).ok().and_then(|m| m.as_string()).unwrap_or_else(|| "video/webm".into());
                    if bytes.is_empty() {
                        a.toast("Video konnte nicht erstellt werden");
                        return;
                    }
                    let ext = if mime.contains("mp4") { "mp4" } else { "webm" };
                    let name = format!("{}-Seite-{}-Zeitraffer.{}", super::storage::file_name(&title), page_no, ext);
                    if share {
                        super::storage::share_next();
                    }
                    super::storage::save_file(&bytes, &mime, &name);
                    if !share {
                        a.toast("Zeitraffer-Video gespeichert");
                    }
                });
            });
        }
        // erst nach diesem Aufruf beginnen (die App ist gerade noch in Benutzung)
        set_timeout(0, move || step(0, frames, canvas2, rec, handle, scale, title, page_no, share));
    }
}
