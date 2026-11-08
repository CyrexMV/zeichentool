//! Speicherung: Einstellungen (localStorage), Autospeichern (IndexedDB),
//! Projektdateien, PNG-Export, Bildimport, Service Worker (Offline-Nutzung).

use super::*;
use crate::engine::history::StoredLayer;
use crate::engine::project;
use crate::engine::{Action, Symmetry};
use js_sys::{Array, Promise, Uint8Array};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, HtmlImageElement, IdbDatabase, IdbOpenDbRequest, IdbRequest, IdbTransactionMode, Url};

const SETTINGS_KEY: &str = "zeichentool.settings.v1";
const DB_NAME: &str = "zeichentool";
const STORE: &str = "projekte";
const AUTOSAVE_KEY: &str = "aktuell";

// ---------------------------------------------------------------------------
// Einstellungen
// ---------------------------------------------------------------------------

impl App {
    pub fn save_settings(&mut self) {
        let s = &self.settings;
        let mut out = vec![
            format!("smoothing={}", s.smoothing),
            format!("gamma={}", s.gamma),
            format!("pen_only={}", s.pen_only),
            format!("pen_button={}", s.pen_button.code()),
            format!("quickshape={}", s.quickshape),
            format!("hover={}", s.hover_cursor),
            format!("side_right={}", s.side_right),
            format!("symmetry={}", s.symmetry.code()),
            format!("radial_n={}", s.radial_n),
            format!("fill_tol={}", s.fill_tol),
            format!("fill_all={}", s.fill_all),
            format!("shape={}", s.shape.code()),
            format!("shape_fill={}", s.shape_fill),
            format!("pen_seen={}", self.pen_seen),
            format!("tool={}", self.tool.code()),
            format!("color={}", self.color.to_hex()),
            format!("recent={}", self.recent.iter().map(|c| c.to_hex()).collect::<Vec<_>>().join(",")),
        ];
        for (name, slot) in [("brush", self.brush), ("eraser", self.eraser), ("smudge", self.smudge)] {
            out.push(format!("{}={},{},{}", name, self.presets[slot.preset].id, slot.size, slot.opacity));
        }
        for (i, p) in self.presets.iter().enumerate() {
            let (sz, op) = self.preset_mem[i];
            out.push(format!(
                "p.{}={},{},{},{},{},{},{},{},{}",
                p.id, p.hardness, p.spacing, p.flow, p.pressure_size, p.pressure_opacity, p.grain, p.tilt_shading, sz, op
            ));
        }
        if let Ok(Some(ls)) = window().local_storage() {
            let _ = ls.set_item(SETTINGS_KEY, &out.join("\n"));
        }
    }

    pub fn load_settings(&mut self) {
        let Ok(Some(ls)) = window().local_storage() else { return };
        let Ok(Some(text)) = ls.get_item(SETTINGS_KEY) else { return };
        let b = |v: &str| v == "true";
        let f = |v: &str, d: f32| v.parse::<f32>().unwrap_or(d);
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let s = &mut self.settings;
            match k {
                "smoothing" => s.smoothing = f(v, 0.25).clamp(0.0, 0.95),
                "gamma" => s.gamma = f(v, 1.0).clamp(0.3, 2.5),
                "pen_only" => s.pen_only = b(v),
                "pen_button" => s.pen_button = PenButton::from_code(v),
                "quickshape" => s.quickshape = b(v),
                "hover" => s.hover_cursor = b(v),
                "side_right" => s.side_right = b(v),
                "symmetry" => s.symmetry = Symmetry::from_code(v),
                "radial_n" => s.radial_n = v.parse::<u8>().unwrap_or(6).clamp(2, 24),
                "fill_tol" => s.fill_tol = f(v, 0.12).clamp(0.0, 1.0),
                "fill_all" => s.fill_all = b(v),
                "shape" => s.shape = ShapeKind::from_code(v),
                "shape_fill" => s.shape_fill = b(v),
                "pen_seen" => self.pen_seen = b(v),
                "tool" => self.tool = Tool::from_code(v).unwrap_or(Tool::Brush),
                "color" => {
                    if let Some(c) = Color::from_hex(v) {
                        self.color = c;
                        self.prev_color = c;
                        self.hsv = c.to_hsv();
                    }
                }
                "recent" => self.recent = v.split(',').filter_map(Color::from_hex).collect(),
                "brush" | "eraser" | "smudge" => {
                    let parts: Vec<&str> = v.split(',').collect();
                    if parts.len() == 3 {
                        if let Some(idx) = self.presets.iter().position(|p| p.id == parts[0]) {
                            let slot = Slot { preset: idx, size: f(parts[1], 10.0).clamp(0.5, 1000.0), opacity: f(parts[2], 1.0).clamp(0.01, 1.0) };
                            match k {
                                "brush" => self.brush = slot,
                                "eraser" => self.eraser = slot,
                                _ => self.smudge = slot,
                            }
                        }
                    }
                }
                _ if k.starts_with("p.") => {
                    let id = &k[2..];
                    let vals: Vec<f32> = v.split(',').map(|x| f(x, 0.0)).collect();
                    if let (Some(i), 9) = (self.presets.iter().position(|p| p.id == id), vals.len()) {
                        let p = &mut self.presets[i];
                        p.hardness = vals[0].clamp(0.0, 1.0);
                        p.spacing = vals[1].clamp(0.02, 1.0);
                        p.flow = vals[2].clamp(0.01, 1.0);
                        p.pressure_size = vals[3].clamp(0.0, 1.0);
                        p.pressure_opacity = vals[4].clamp(0.0, 1.0);
                        p.grain = vals[5].clamp(0.0, 1.0);
                        p.tilt_shading = vals[6].clamp(0.0, 1.0);
                        self.preset_mem[i] = (vals[7].clamp(0.5, 1000.0), vals[8].clamp(0.01, 1.0));
                    }
                }
                _ => {}
            }
        }
        self.preset_mem[self.brush.preset] = (self.brush.size, self.brush.opacity);
    }

    // -----------------------------------------------------------------------
    // Autospeichern
    // -----------------------------------------------------------------------

    pub fn mark_unsaved(&mut self) {
        self.unsaved = true;
        self.last_change = now();
        set_out_text("save-state", "nicht gespeichert");
        set_timeout(2600, || {
            with_app(|a| a.maybe_autosave());
        });
    }

    fn maybe_autosave(&mut self) {
        if !self.unsaved {
            return;
        }
        if self.stroke.is_some() || now() - self.last_change < 2400.0 {
            set_timeout(1200, || {
                with_app(|a| a.maybe_autosave());
            });
            return;
        }
        self.autosave_now();
    }

    pub fn autosave_now(&mut self) {
        let Some(db) = self.db.clone() else { return };
        let bytes = project::save(&self.doc, &mut self.pack_cache);
        self.unsaved = false;
        let data = Uint8Array::from(&bytes[..]);
        let ok = (|| -> Result<(), JsValue> {
            let tx = db.transaction_with_str_and_mode(STORE, IdbTransactionMode::Readwrite)?;
            let store = tx.object_store(STORE)?;
            store.put_with_key(&data, &JsValue::from_str(AUTOSAVE_KEY))?;
            Ok(())
        })()
        .is_ok();
        set_out_text("save-state", if ok { "automatisch gespeichert" } else { "Speichern fehlgeschlagen" });
    }

    // -----------------------------------------------------------------------
    // Dateien
    // -----------------------------------------------------------------------

    pub fn download_project(&mut self) {
        let bytes = project::save(&self.doc, &mut self.pack_cache);
        download(&bytes, "application/octet-stream", &format!("{}.zeichen", file_stamp()));
        self.toast("Projekt gespeichert");
    }

    pub fn export_png(&mut self, with_paper: bool) {
        self.toast("PNG wird erstellt …");
        let png = project::export_png(&self.doc, with_paper && self.doc.paper.is_some());
        download(&png, "image/png", &format!("{}.png", file_stamp()));
    }

    fn import_image(&mut self, img: &HtmlImageElement, name: &str) {
        let (w, h) = (self.doc.w as f64, self.doc.h as f64);
        let (iw, ih) = (img.natural_width() as f64, img.natural_height() as f64);
        if iw < 1.0 || ih < 1.0 {
            self.toast("Bild konnte nicht gelesen werden");
            return;
        }
        let s = (w / iw).min(h / ih).min(1.0);
        let (dw, dh) = (iw * s, ih * s);
        let canvas: HtmlCanvasElement = dom().create_element("canvas").unwrap().unchecked_into();
        canvas.set_width(w as u32);
        canvas.set_height(h as u32);
        let ctx: CanvasRenderingContext2d = canvas.get_context("2d").ok().flatten().unwrap().unchecked_into();
        let _ = ctx.draw_image_with_html_image_element_and_dw_and_dh(img, (w - dw) * 0.5, (h - dh) * 0.5, dw, dh);
        let Ok(data) = ctx.get_image_data(0.0, 0.0, w, h) else {
            self.toast("Bild konnte nicht gelesen werden");
            return;
        };
        let rgba = data.data().0;
        let idx = self.doc.insert_image_layer(name, &rgba, self.doc.active + 1);
        let stored = StoredLayer::from_layer(&self.doc.layers[idx]);
        self.history.push(Action::AddLayer { index: idx, layer: stored });
        self.redraw_all();
        self.after_change();
        self.render_layers();
        self.set_tool(Tool::Move);
        self.toast("Bild importiert – mit „Verschieben“ positionieren");
    }
}

fn set_out_text(id: &str, t: &str) {
    if let Some(e) = dom().get_element_by_id(id) {
        e.set_text_content(Some(t));
    }
}

fn file_stamp() -> String {
    let d = js_sys::Date::new_0();
    format!(
        "zeichnung-{:04}-{:02}-{:02}-{:02}{:02}",
        d.get_full_year(),
        d.get_month() + 1,
        d.get_date(),
        d.get_hours(),
        d.get_minutes()
    )
}

fn download(bytes: &[u8], mime: &str, name: &str) {
    let parts = Array::new();
    parts.push(&Uint8Array::from(bytes));
    let opts = BlobPropertyBag::new();
    opts.set_type(mime);
    let Ok(blob) = Blob::new_with_u8_array_sequence_and_options(&parts, &opts) else { return };
    let Ok(url) = Url::create_object_url_with_blob(&blob) else { return };
    let a: HtmlAnchorElement = dom().create_element("a").unwrap().unchecked_into();
    a.set_href(&url);
    a.set_download(name);
    let _ = dom().body().unwrap().append_child(&a);
    a.click();
    a.remove();
    set_timeout(10_000, move || {
        let _ = Url::revoke_object_url(&url);
    });
}

pub fn bind_files() {
    let open = input_el("file-open");
    on(&open, "change", |e: Event| {
        let inp: HtmlInputElement = e.target().unwrap().unchecked_into();
        let Some(file) = inp.files().and_then(|f| f.get(0)) else { return };
        inp.set_value("");
        spawn_local(async move {
            let Ok(buf) = JsFuture::from(file.array_buffer()).await else { return };
            let bytes = Uint8Array::new(&buf).to_vec();
            with_app(|a| match project::load(&bytes) {
                Ok(doc) => {
                    a.replace_document(doc);
                    a.mark_unsaved();
                    a.toast("Projekt geöffnet");
                }
                Err(err) => a.toast(&err),
            });
        });
    });
    let imp = input_el("file-import");
    on(&imp, "change", |e: Event| {
        let inp: HtmlInputElement = e.target().unwrap().unchecked_into();
        let Some(file) = inp.files().and_then(|f| f.get(0)) else { return };
        inp.set_value("");
        let name: String = file.name().chars().take(32).collect();
        let Ok(url) = Url::create_object_url_with_blob(&file) else { return };
        let img = HtmlImageElement::new().unwrap();
        let img2 = img.clone();
        let url2 = url.clone();
        let onload = Closure::once_into_js(move || {
            with_app(|a| a.import_image(&img2, &name));
            let _ = Url::revoke_object_url(&url2);
        });
        img.set_onload(Some(onload.unchecked_ref()));
        img.set_src(&url);
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
    let req: IdbOpenDbRequest = factory.open_with_u32(DB_NAME, 1)?;
    let r2 = req.clone();
    let upgrade = Closure::once_into_js(move || {
        if let Ok(db) = r2.result() {
            let db: IdbDatabase = db.unchecked_into();
            if !db.object_store_names().contains(STORE) {
                let _ = db.create_object_store(STORE);
            }
        }
    });
    req.set_onupgradeneeded(Some(upgrade.unchecked_ref()));
    let db = JsFuture::from(request_promise(&req)).await?;
    Ok(db.unchecked_into())
}

async fn load_autosave(db: &IdbDatabase) -> Result<Option<Vec<u8>>, JsValue> {
    let tx = db.transaction_with_str_and_mode(STORE, IdbTransactionMode::Readonly)?;
    let store = tx.object_store(STORE)?;
    let req = store.get(&JsValue::from_str(AUTOSAVE_KEY))?;
    let v = JsFuture::from(request_promise(&req)).await?;
    if v.is_undefined() || v.is_null() {
        return Ok(None);
    }
    Ok(Some(Uint8Array::new(&v).to_vec()))
}

fn hide_loading() {
    set_class(&el("loading"), "done", true);
}

pub fn init_storage() {
    // Sicherheitsnetz, falls IndexedDB hängt
    set_timeout(4000, hide_loading);
    spawn_local(async {
        match open_db().await {
            Ok(db) => {
                let saved = load_autosave(&db).await.ok().flatten();
                with_app(|a| {
                    a.db = Some(db);
                    if let Some(bytes) = saved {
                        match project::load(&bytes) {
                            Ok(doc) => {
                                a.replace_document(doc);
                                set_out_text("save-state", "automatisch gespeichert");
                            }
                            Err(_) => a.toast("Letzte Zeichnung konnte nicht geladen werden"),
                        }
                    }
                });
            }
            Err(_) => {
                with_app(|a| a.toast("Autospeichern nicht verfügbar"));
            }
        }
        hide_loading();
    });
    // Beim Verlassen/Ausblenden sofort sichern
    on(&dom(), "visibilitychange", |_: Event| {
        if dom().visibility_state() == web_sys::VisibilityState::Hidden {
            with_app(|a| {
                if a.unsaved && a.stroke.is_none() {
                    a.autosave_now();
                }
            });
        }
    });
}

pub fn register_service_worker() {
    let nav = window().navigator();
    let has = js_sys::Reflect::get(&nav, &"serviceWorker".into()).map(|v| !v.is_undefined()).unwrap_or(false);
    if has {
        let _ = nav.service_worker().register("sw.js");
    }
}
