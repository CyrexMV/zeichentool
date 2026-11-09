//! Speicherung: IndexedDB (Notizbücher + Bibliothek), Einstellungen, Autospeichern,
//! Export (PDF/PNG/Backup), Import, Übernahme alter Zeichnungen, Android-Brücke.

use super::editor::new_canvas;
use super::render::wrap_text;
use super::*;
use crate::engine::format::{decode_png, encode_png, legacy_drawing_to_rgba, load_library, load_notebook, save_library, save_notebook, NoteMeta};
use crate::engine::note::hash_bytes;
use crate::engine::pdf::export_pdf;
use crate::engine::{Content, Format, ImageItem, Item, Notebook, TextBox};
use js_sys::{Array, Promise, Uint8Array};
use std::rc::Rc;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, HtmlCanvasElement, IdbDatabase, IdbOpenDbRequest, IdbRequest, IdbTransactionMode, Url};

const SETTINGS_KEY: &str = "notizen.settings.v2";
const DB_NAME: &str = "zeichentool";
const DB_VERSION: u32 = 2;
const LEGACY: &str = "projekte";
const NOTEBOOKS: &str = "notebooks";
const META: &str = "meta";

pub fn new_id() -> String {
    format!("nb{:x}{:06x}", wall_clock() as u64, (js_sys::Math::random() * 16_777_215.0) as u32)
}

// ---------------------------------------------------------------------------
// Einstellungen
// ---------------------------------------------------------------------------

fn c4(c: [u8; 4]) -> String {
    format!("{:02x}{:02x}{:02x}{:02x}", c[0], c[1], c[2], c[3])
}

fn p4(s: &str) -> Option<[u8; 4]> {
    if s.len() != 8 {
        return None;
    }
    let b = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some([b(0)?, b(2)?, b(4)?, b(6)?])
}

impl App {
    pub fn save_settings(&mut self) {
        let s = &self.settings;
        let p = &self.prefs;
        let colors = |c: &[[u8; 4]; 3]| c.iter().map(|x| c4(*x)).collect::<Vec<_>>().join(",");
        let sizes = |v: &[f32; 3]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
        let lines = vec![
            format!("theme={}", s.theme.code()),
            format!("pressure={}", s.pressure),
            format!("smoothing={}", s.smoothing),
            format!("quickshape={}", s.quickshape),
            format!("pen_button={}", s.pen_button.code()),
            format!("palm_lock={}", s.palm_lock),
            format!("one_finger_scroll={}", s.one_finger_scroll),
            format!("finger_draw={}", s.finger_draw),
            format!("pen_seen={}", s.pen_seen),
            format!("migrated={}", s.migrated),
            format!("lib_sort={}", self.lib_sort),
            format!("pen_kind={}", p.pen_kind.code()),
            format!("pen_colors={}", colors(&p.pen_colors)),
            format!("pen_sizes={}", sizes(&p.pen_sizes)),
            format!("pen_sel={},{}", p.pen_ci, p.pen_si),
            format!("hl_colors={}", colors(&p.hl_colors)),
            format!("hl_sizes={}", sizes(&p.hl_sizes)),
            format!("hl_sel={},{}", p.hl_ci, p.hl_si),
            format!("hl_straight={}", p.hl_straight),
            format!("er_sizes={}", sizes(&p.er_sizes)),
            format!("er_sel={}", p.er_si),
            format!("er_mode={}", if p.er_mode == EraserMode::Stroke { "stroke" } else { "precise" }),
            format!("er_hl_only={}", p.er_hl_only),
            format!("er_auto_back={}", p.er_auto_back),
            format!("text_colors={}", colors(&p.text_colors)),
            format!("text_sel={}", p.text_ci),
            format!("text_size={}", p.text_size),
            format!("text_bold={}", p.text_bold),
            format!("lasso={},{},{}", p.lasso_ink, p.lasso_text, p.lasso_img),
            format!("laser={}", c4(p.laser_color)),
            format!("new={},{},{},{},{}", self.new_nb.cover, self.new_nb.template.kind.code(), self.new_nb.template.spacing, self.new_nb.paper.to_hex(), self.new_nb.format),
            format!("tool={}", self.tool.code()),
        ];
        if let Ok(Some(ls)) = window().local_storage() {
            let _ = ls.set_item(SETTINGS_KEY, &lines.join("\n"));
        }
        self.sync_native_settings();
    }

    pub fn load_settings(&mut self) {
        let Ok(Some(ls)) = window().local_storage() else { return };
        let Ok(Some(text)) = ls.get_item(SETTINGS_KEY) else { return };
        let b = |v: &str| v == "true";
        let f = |v: &str, d: f32| v.parse::<f32>().unwrap_or(d);
        let colors = |v: &str, dst: &mut [[u8; 4]; 3]| {
            for (i, c) in v.split(',').enumerate().take(3) {
                if let Some(c) = p4(c) {
                    dst[i] = c;
                }
            }
        };
        let sizes = |v: &str, dst: &mut [f32; 3], lo: f32, hi: f32| {
            for (i, x) in v.split(',').enumerate().take(3) {
                if let Ok(x) = x.parse::<f32>() {
                    dst[i] = x.clamp(lo, hi);
                }
            }
        };
        let pair = |v: &str| -> (usize, usize) {
            let mut it = v.split(',').map(|x| x.parse::<usize>().unwrap_or(0).min(2));
            (it.next().unwrap_or(0), it.next().unwrap_or(0))
        };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let s = &mut self.settings;
            let p = &mut self.prefs;
            match k {
                "theme" => s.theme = Theme::from_code(v),
                "pressure" => s.pressure = b(v),
                "smoothing" => s.smoothing = f(v, 0.35).clamp(0.0, 0.9),
                "quickshape" => s.quickshape = b(v),
                "pen_button" => s.pen_button = PenButton::from_code(v),
                "palm_lock" => s.palm_lock = f(v, 2.0).clamp(0.0, 10.0),
                "one_finger_scroll" => s.one_finger_scroll = b(v),
                "finger_draw" => s.finger_draw = b(v),
                "pen_seen" => s.pen_seen = b(v),
                "migrated" => s.migrated = b(v),
                "lib_sort" => self.lib_sort = v.to_string(),
                "pen_kind" => p.pen_kind = PenKind::from_code(v),
                "pen_colors" => colors(v, &mut p.pen_colors),
                "pen_sizes" => sizes(v, &mut p.pen_sizes, 0.3, 8.0),
                "pen_sel" => (p.pen_ci, p.pen_si) = pair(v),
                "hl_colors" => colors(v, &mut p.hl_colors),
                "hl_sizes" => sizes(v, &mut p.hl_sizes, 4.0, 30.0),
                "hl_sel" => (p.hl_ci, p.hl_si) = pair(v),
                "hl_straight" => p.hl_straight = b(v),
                "er_sizes" => sizes(v, &mut p.er_sizes, 2.0, 60.0),
                "er_sel" => p.er_si = pair(v).0,
                "er_mode" => p.er_mode = if v == "stroke" { EraserMode::Stroke } else { EraserMode::Precise },
                "er_hl_only" => p.er_hl_only = b(v),
                "er_auto_back" => p.er_auto_back = b(v),
                "text_colors" => colors(v, &mut p.text_colors),
                "text_sel" => p.text_ci = pair(v).0,
                "text_size" => p.text_size = f(v, 16.0).clamp(8.0, 48.0),
                "text_bold" => p.text_bold = b(v),
                "lasso" => {
                    let l: Vec<bool> = v.split(',').map(b).collect();
                    if l.len() == 3 {
                        (p.lasso_ink, p.lasso_text, p.lasso_img) = (l[0], l[1], l[2]);
                    }
                }
                "laser" => {
                    if let Some(c) = p4(v) {
                        p.laser_color = c;
                    }
                }
                "new" => {
                    let l: Vec<&str> = v.split(',').collect();
                    if l.len() == 5 {
                        self.new_nb.cover = l[0].parse().unwrap_or(0);
                        self.new_nb.template.kind = TemplateKind::from_code(l[1]);
                        self.new_nb.template.spacing = l[2].parse::<u8>().unwrap_or(1).min(2);
                        self.new_nb.paper = Color::from_hex(l[3]).unwrap_or(Color::WHITE);
                        self.new_nb.format = l[4].parse().unwrap_or(0);
                    }
                }
                "tool" => self.tool = Tool::from_code(v).unwrap_or(Tool::Pen),
                _ => {}
            }
        }
    }

    /// Meldet der Android-App alle Bedienflächen (Leisten, Menüs, Dialoge), damit der
    /// Handballen-Filter Finger dort nie blockiert. Format: "x,y,w,h;…" in CSS-Pixeln.
    pub fn sync_ui_rects(&mut self) {
        let Some(bridge) = android_bridge() else { return };
        let mut parts: Vec<String> = Vec::new();
        let vw = window().inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(4000.0);
        let vh = window().inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(4000.0);
        let full = self.screen != Screen::Editor
            || dom().query_selector(".modal:not(.hidden), .popover:not(.hidden), .study:not(.hidden)").ok().flatten().is_some()
            || self.ed.as_ref().map(|e| e.text_edit.is_some()).unwrap_or(false);
        if full {
            parts.push(format!("0,0,{},{}", vw, vh));
        } else if let Ok(list) = dom().query_selector_all(".ed-left, .toolbar, .ed-right, .page-pill, #sel-menu:not(.hidden), #paste-btn:not(.hidden), #navigator.open") {
            for i in 0..list.length() {
                let Some(n) = list.get(i) else { continue };
                let el: web_sys::Element = n.unchecked_into();
                let r = el.get_bounding_client_rect();
                if r.width() > 0.0 && r.height() > 0.0 {
                    parts.push(format!("{:.0},{:.0},{:.0},{:.0}", r.left() - 6.0, r.top() - 6.0, r.width() + 12.0, r.height() + 12.0));
                }
            }
            if let Some(r) = self.ed.as_ref().and_then(|e| e.ruler) {
                let (x, y, w, h) = r.bounds();
                parts.push(format!("{:.0},{:.0},{:.0},{:.0}", x, y, w, h));
            }
        }
        let s = parts.join(";");
        if s == self.last_ui_rects {
            return;
        }
        self.last_ui_rects = s.clone();
        if let Ok(f) = js_sys::Reflect::get(&bridge, &"setUiRects".into()) {
            if let Some(f) = f.dyn_ref::<js_sys::Function>() {
                let _ = f.call1(&bridge, &JsValue::from_str(&s));
            }
        }
    }

    pub fn sync_native_settings(&self) {
        if let Some(bridge) = android_bridge() {
            if let Ok(f) = js_sys::Reflect::get(&bridge, &"setPalmLock".into()) {
                if let Some(f) = f.dyn_ref::<js_sys::Function>() {
                    let _ = f.call1(&bridge, &JsValue::from_f64(self.settings.palm_lock as f64));
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Notizbücher speichern
    // -----------------------------------------------------------------------

    pub fn persist_notebook(&self, nb: &Notebook) {
        if let Some(db) = &self.db {
            let bytes = save_notebook(nb);
            idb_put(db, NOTEBOOKS, &nb.id, &Uint8Array::from(&bytes[..]).into());
        }
    }

    pub fn persist_folders(&self) {
        if let Some(db) = &self.db {
            idb_put(db, META, "folders", &JsValue::from_str(&self.folders.join("\n")));
        }
    }

    pub fn persist_library(&self) {
        if let Some(db) = &self.db {
            idb_put(db, META, "library", &JsValue::from_str(&save_library(&self.library)));
        }
    }

    pub fn mark_unsaved(&mut self) {
        self.unsaved = true;
        self.last_change = now();
        set_text("save-state", "nicht gespeichert");
        set_timeout(900, || {
            with_app(|a| a.maybe_autosave());
        });
    }

    fn maybe_autosave(&mut self) {
        if !self.unsaved {
            return;
        }
        let busy = self.ed.as_ref().map(|e| e.op.is_some()).unwrap_or(false);
        if busy || now() - self.last_change < 800.0 {
            set_timeout(600, || {
                with_app(|a| a.maybe_autosave());
            });
            return;
        }
        self.save_now();
    }

    /// Aktuelles Notizbuch sofort speichern.
    pub fn save_now(&mut self) {
        let Some(ed) = &self.ed else { return };
        if !self.unsaved {
            return;
        }
        self.unsaved = false;
        let meta = NoteMeta::of(&ed.nb);
        self.persist_notebook(&ed.nb);
        match self.library.iter_mut().find(|m| m.id == meta.id) {
            Some(m) => *m = meta,
            None => self.library.push(meta),
        }
        self.persist_library();
        set_text("save-state", "automatisch gespeichert");
    }

    pub fn open_notebook(&mut self, id: String) {
        let Some(db) = self.db.clone() else { return };
        spawn_local(async move {
            let res = idb_get(&db, NOTEBOOKS, &id).await;
            with_app(|a| match res.map(|v| Uint8Array::new(&v).to_vec()) {
                Some(bytes) => match load_notebook(&bytes) {
                    Ok(nb) => a.enter_editor(nb),
                    Err(e) => a.toast(&e),
                },
                None => a.toast("Notizbuch nicht gefunden"),
            });
        });
    }

    /// Gespeichertes Notizbuch laden, ändern, zurückschreiben.
    pub fn modify_stored(&mut self, id: String, f: impl FnOnce(&mut Notebook) + 'static) {
        self.with_stored(id, move |a, mut nb| {
            f(&mut nb);
            a.add_notebook(nb, false);
        });
    }

    pub fn with_stored(&mut self, id: String, f: impl FnOnce(&mut App, Notebook) + 'static) {
        let Some(db) = self.db.clone() else { return };
        spawn_local(async move {
            let res = idb_get(&db, NOTEBOOKS, &id).await;
            with_app(|a| match res.map(|v| load_notebook(&Uint8Array::new(&v).to_vec())) {
                Some(Ok(nb)) => f(a, nb),
                Some(Err(e)) => a.toast(&e),
                None => a.toast("Notizbuch nicht gefunden"),
            });
        });
    }

    pub fn duplicate_stored(&mut self, id: String) {
        self.with_stored(id, |a, mut nb| {
            nb.id = new_id();
            nb.title = format!("{} (Kopie)", nb.title);
            nb.created = wall_clock();
            nb.modified = nb.created;
            a.add_notebook(nb, false);
            a.toast("Notizbuch dupliziert");
        });
    }

    pub fn delete_stored(&mut self, id: &str) {
        self.library.retain(|m| m.id != id);
        if let Some(db) = &self.db {
            idb_delete(db, NOTEBOOKS, id);
        }
        self.persist_library();
        self.render_library();
    }

    // -----------------------------------------------------------------------
    // Export
    // -----------------------------------------------------------------------

    pub fn export_pdf(&mut self, page_only: bool) {
        let Some(ed) = &self.ed else { return };
        let pages: Vec<usize> = if page_only { vec![ed.cur_page] } else { (0..ed.nb.pages.len()).collect() };
        let nb = ed.nb.clone();
        self.export_notebook_pdf(&nb, Some(pages));
    }

    pub fn export_notebook_pdf(&mut self, nb: &Notebook, pages: Option<Vec<usize>>) {
        self.toast("PDF wird erstellt …");
        let pages = pages.unwrap_or_else(|| (0..nb.pages.len()).collect());
        let (_, ctx) = new_canvas();
        let wrap = move |t: &TextBox| wrap_text(&ctx, t);
        let bytes = export_pdf(nb, &pages, &wrap);
        save_file(&bytes, "application/pdf", &format!("{}.pdf", file_name(&nb.title)));
    }

    pub fn export_png(&mut self) {
        let Some(ed) = &mut self.ed else { return };
        let i = ed.cur_page;
        let name = format!("{}-Seite-{}.png", file_name(&ed.nb.title), i + 1);
        if let Some(url) = ed.page_png(i, &mut self.images) {
            if let Some(b64) = url.split(',').nth(1) {
                save_b64(b64, "image/png", &name, &url);
            }
        }
    }

    pub fn export_znote(&mut self, nb: Option<&Notebook>) {
        let bytes = match nb {
            Some(n) => save_notebook(n),
            None => match &self.ed {
                Some(ed) => save_notebook(&ed.nb),
                None => return,
            },
        };
        let title = nb.map(|n| n.title.clone()).or_else(|| self.ed.as_ref().map(|e| e.nb.title.clone())).unwrap_or_default();
        save_file(&bytes, "application/octet-stream", &format!("{}.znote", file_name(&title)));
    }

    // -----------------------------------------------------------------------
    // Import
    // -----------------------------------------------------------------------

    fn import_bytes(&mut self, name: &str, bytes: Vec<u8>) {
        if bytes.starts_with(b"ZNB1") {
            match load_notebook(&bytes) {
                Ok(mut nb) => {
                    if self.library.iter().any(|m| m.id == nb.id) {
                        nb.id = new_id();
                        nb.title = format!("{} (importiert)", nb.title);
                    }
                    self.add_notebook(nb, false);
                    self.toast("Notizbuch importiert");
                }
                Err(e) => self.toast(&e),
            }
        } else if bytes.starts_with(b"ZTL1") {
            match legacy_drawing_to_rgba(&bytes) {
                Ok((w, h, rgba)) => {
                    let nb = image_notebook(name.trim_end_matches(".zeichen"), w, h, &rgba);
                    self.add_notebook(nb, false);
                    self.toast("Zeichnung als Notizbuch übernommen");
                }
                Err(e) => self.toast(&e),
            }
        } else {
            self.toast("Unbekanntes Dateiformat");
        }
    }
}

fn file_name(title: &str) -> String {
    let s: String = title.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' }).collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        "Notion".into()
    } else {
        s
    }
}

/// Notizbuch mit einem Bild auf der ersten Seite (für Bild-/Zeichnungs-Import).
fn image_notebook(title: &str, w: u32, h: u32, rgba: &[u8]) -> Notebook {
    let base = 842.0f32;
    let format = if w >= h { Format { w: base, h: base * h as f32 / w as f32 } } else { Format { w: base * w as f32 / h as f32, h: base } };
    let mut nb = Notebook::new(new_id(), if title.is_empty() { "Importiert" } else { title }, 8, format, Default::default(), Color::WHITE, wall_clock());
    let png = encode_png(w, h, rgba);
    let id = nb.alloc_id();
    nb.pages[0].insert([Item { id, order: 1.0, content: Content::Image(ImageItem { x: 0.0, y: 0.0, w: format.w, h: format.h, key: hash_bytes(&png), png: Rc::new(png) }) }]);
    nb
}

// ---------------------------------------------------------------------------
// Datei-Ausgabe (Browser-Download oder Android-Brücke)
// ---------------------------------------------------------------------------

pub fn android_bridge() -> Option<js_sys::Object> {
    let v = js_sys::Reflect::get(&window(), &"ZeichentoolAndroid".into()).ok()?;
    if v.is_undefined() || v.is_null() {
        None
    } else {
        Some(v.unchecked_into())
    }
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

fn bridge_save(b64: &str, mime: &str, name: &str) -> Option<bool> {
    let bridge = android_bridge()?;
    let f = js_sys::Reflect::get(&bridge, &"saveFile".into()).ok()?;
    let f = f.dyn_ref::<js_sys::Function>()?;
    Some(f.call3(&bridge, &name.into(), &mime.into(), &b64.into()).map(|v| v.as_bool().unwrap_or(false)).unwrap_or(false))
}

fn save_b64(b64: &str, mime: &str, name: &str, data_url: &str) {
    match bridge_save(b64, mime, name) {
        Some(true) => {}
        Some(false) => {
            with_app(|a| a.toast("Speichern fehlgeschlagen"));
        }
        None => click_download(data_url, name),
    }
}

pub fn save_file(bytes: &[u8], mime: &str, name: &str) {
    if android_bridge().is_some() {
        if !bridge_save(&base64(bytes), mime, name).unwrap_or(false) {
            with_app(|a| a.toast("Speichern fehlgeschlagen"));
        }
        return;
    }
    let parts = Array::new();
    parts.push(&Uint8Array::from(bytes));
    let opts = BlobPropertyBag::new();
    opts.set_type(mime);
    let Ok(blob) = Blob::new_with_u8_array_sequence_and_options(&parts, &opts) else { return };
    let Ok(url) = Url::create_object_url_with_blob(&blob) else { return };
    click_download(&url, name);
    set_timeout(15_000, move || {
        let _ = Url::revoke_object_url(&url);
    });
}

fn click_download(url: &str, name: &str) {
    // außerhalb des laufenden Klick-Handlers auslösen
    let (url, name) = (url.to_string(), name.to_string());
    set_timeout(0, move || {
        let a: HtmlAnchorElement = dom().create_element("a").unwrap().unchecked_into();
        a.set_href(&url);
        a.set_download(&name);
        let _ = dom().body().unwrap().append_child(&a);
        a.click();
        a.remove();
    });
}

// ---------------------------------------------------------------------------
// Dateiauswahl
// ---------------------------------------------------------------------------

/// Bild dekodieren (Browser), verkleinern und als PNG-Bytes liefern.
fn decode_image(file: web_sys::File, done: impl FnOnce(Vec<u8>, u32, u32) + 'static) {
    let Ok(url) = Url::create_object_url_with_blob(&file) else { return };
    let img = HtmlImageElement::new().unwrap();
    let img2 = img.clone();
    let url2 = url.clone();
    let onload = Closure::once_into_js(move || {
        let _ = Url::revoke_object_url(&url2);
        let (nw, nh) = (img2.natural_width() as f64, img2.natural_height() as f64);
        if nw < 1.0 || nh < 1.0 {
            with_app(|a| a.toast("Bild konnte nicht gelesen werden"));
            return;
        }
        let f = (2000.0 / nw.max(nh)).min(1.0);
        let (w, h) = ((nw * f).round().max(1.0), (nh * f).round().max(1.0));
        let (canvas, ctx): (HtmlCanvasElement, _) = new_canvas();
        canvas.set_width(w as u32);
        canvas.set_height(h as u32);
        let _ = ctx.draw_image_with_html_image_element_and_dw_and_dh(&img2, 0.0, 0.0, w, h);
        if let Ok(data) = ctx.get_image_data(0.0, 0.0, w, h) {
            let png = encode_png(w as u32, h as u32, &data.data().0);
            done(png, w as u32, h as u32);
        }
    });
    img.set_onload(Some(onload.unchecked_ref()));
    img.set_src(&url);
}

pub fn bind_files() {
    on(&input_el("file-image"), "change", |e: Event| {
        let inp: HtmlInputElement = e.target().unwrap().unchecked_into();
        let Some(file) = inp.files().and_then(|f| f.get(0)) else { return };
        inp.set_value("");
        decode_image(file, |png, w, h| {
            with_app(|a| a.insert_image(png, w as f32, h as f32));
        });
    });
    on(&input_el("file-import"), "change", |e: Event| {
        let inp: HtmlInputElement = e.target().unwrap().unchecked_into();
        let Some(file) = inp.files().and_then(|f| f.get(0)) else { return };
        inp.set_value("");
        let name = file.name();
        if file.type_().starts_with("image/") {
            let title: String = name.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or(name.clone());
            decode_image(file, move |png, w, h| {
                if let Some((w, h, rgba)) = decode_png(&png) {
                    let nb = image_notebook(&title, w, h, &rgba);
                    with_app(|a| {
                        a.add_notebook(nb, false);
                        a.toast("Bild als Notizbuch importiert");
                    });
                }
                let _ = (w, h);
            });
            return;
        }
        spawn_local(async move {
            let Ok(buf) = JsFuture::from(file.array_buffer()).await else { return };
            let bytes = Uint8Array::new(&buf).to_vec();
            with_app(|a| a.import_bytes(&name, bytes));
        });
    });
}

// ---------------------------------------------------------------------------
// IndexedDB
// ---------------------------------------------------------------------------

fn request_promise(req: &IdbRequest) -> Promise {
    let req = req.clone();
    Promise::new(&mut |resolve, reject| {
        let r2 = req.clone();
        let ok = Closure::once_into_js(move || {
            let _ = resolve.call1(&JsValue::NULL, &r2.result().unwrap_or(JsValue::UNDEFINED));
        });
        let err = Closure::once_into_js(move || {
            let _ = reject.call0(&JsValue::NULL);
        });
        req.set_onsuccess(Some(ok.unchecked_ref()));
        req.set_onerror(Some(err.unchecked_ref()));
    })
}

async fn open_db() -> Result<IdbDatabase, JsValue> {
    let factory = window().indexed_db()?.ok_or(JsValue::NULL)?;
    let req: IdbOpenDbRequest = factory.open_with_u32(DB_NAME, DB_VERSION)?;
    let r2 = req.clone();
    let upgrade = Closure::once_into_js(move || {
        if let Ok(db) = r2.result() {
            let db: IdbDatabase = db.unchecked_into();
            for s in [LEGACY, NOTEBOOKS, META] {
                if !db.object_store_names().contains(s) {
                    let _ = db.create_object_store(s);
                }
            }
        }
    });
    req.set_onupgradeneeded(Some(upgrade.unchecked_ref()));
    let db = JsFuture::from(request_promise(&req)).await?;
    Ok(db.unchecked_into())
}

async fn idb_get(db: &IdbDatabase, store: &str, key: &str) -> Option<JsValue> {
    let tx = db.transaction_with_str_and_mode(store, IdbTransactionMode::Readonly).ok()?;
    let req = tx.object_store(store).ok()?.get(&JsValue::from_str(key)).ok()?;
    let v = JsFuture::from(request_promise(&req)).await.ok()?;
    if v.is_undefined() || v.is_null() {
        None
    } else {
        Some(v)
    }
}

fn idb_put(db: &IdbDatabase, store: &str, key: &str, value: &JsValue) {
    let ok = (|| -> Result<(), JsValue> {
        let tx = db.transaction_with_str_and_mode(store, IdbTransactionMode::Readwrite)?;
        tx.object_store(store)?.put_with_key(value, &JsValue::from_str(key))?;
        Ok(())
    })();
    if ok.is_err() {
        with_app(|a| a.toast("Speichern fehlgeschlagen"));
    }
}

fn idb_delete(db: &IdbDatabase, store: &str, key: &str) {
    if let Ok(tx) = db.transaction_with_str_and_mode(store, IdbTransactionMode::Readwrite) {
        if let Ok(s) = tx.object_store(store) {
            let _ = s.delete(&JsValue::from_str(key));
        }
    }
}

fn hide_loading() {
    set_class(&el("loading"), "done", true);
}

pub fn init_storage() {
    set_timeout(5000, hide_loading);
    spawn_local(async {
        match open_db().await {
            Ok(db) => {
                let lib = idb_get(&db, META, "library").await.and_then(|v| v.as_string()).map(|s| load_library(&s)).unwrap_or_default();
                let folders: Vec<String> = idb_get(&db, META, "folders").await.and_then(|v| v.as_string()).map(|s| s.lines().filter(|l| !l.trim().is_empty()).map(|l| l.to_string()).collect()).unwrap_or_default();
                let migrated = with_app(|a| a.settings.migrated).unwrap_or(true);
                let legacy = if migrated { None } else { idb_get(&db, LEGACY, "aktuell").await.map(|v| Uint8Array::new(&v).to_vec()) };
                with_app(|a| {
                    a.db = Some(db);
                    a.library = lib;
                    // Ordner: gespeicherte Liste + alle, in denen Notizbücher liegen
                    let mut f = folders;
                    for m in &a.library {
                        if !m.folder.is_empty() && !f.contains(&m.folder) {
                            f.push(m.folder.clone());
                        }
                    }
                    f.sort_by_key(|x| x.to_lowercase());
                    a.folders = f;
                    if !a.settings.migrated {
                        a.settings.migrated = true;
                        if let Some(bytes) = legacy {
                            if let Ok((w, h, rgba)) = legacy_drawing_to_rgba(&bytes) {
                                // nur übernehmen, wenn wirklich etwas gezeichnet wurde
                                if rgba.chunks_exact(4).any(|p| p[3] > 0 && (p[0] < 250 || p[1] < 250 || p[2] < 250)) {
                                    let nb = image_notebook("Meine Zeichnung", w, h, &rgba);
                                    a.library.push(NoteMeta::of(&nb));
                                    a.persist_notebook(&nb);
                                    a.persist_library();
                                    a.toast("Deine bisherige Zeichnung wurde als Notizbuch übernommen");
                                }
                            }
                        }
                        a.save_settings();
                    }
                    a.render_library();
                });
            }
            Err(_) => {
                with_app(|a| {
                    a.toast("Speicher nicht verfügbar – Notizen werden nicht gesichert");
                    a.render_library();
                });
            }
        }
        hide_loading();
    });
    on(&dom(), "visibilitychange", |_: Event| {
        if dom().visibility_state() == web_sys::VisibilityState::Hidden {
            with_app(|a| a.save_now());
        }
    });
}

pub fn register_service_worker() {
    if android_bridge().is_some() {
        return;
    }
    let nav = window().navigator();
    let has = js_sys::Reflect::get(&nav, &"serviceWorker".into()).map(|v| !v.is_undefined()).unwrap_or(false);
    if has {
        let _ = nav.service_worker().register("sw.js");
    }
}
