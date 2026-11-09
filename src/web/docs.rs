//! PDF importieren (pdf.js über `web/pdfimport.js`) und eigenes Papier (Bild als Seitenhintergrund).

use super::*;
use crate::engine::history::{Action, PageMeta};
use crate::engine::note::hash_bytes;
use crate::engine::{Format, ImageItem, Notebook, Page, COVERS};
use js_sys::{Array, Function, Promise, Reflect, Uint8Array};
use std::rc::Rc;
use wasm_bindgen_futures::{spawn_local, JsFuture};

/// Eine gerenderte Dokumentseite: Größe in pt + JPEG.
pub struct DocPage {
    pub w: f32,
    pub h: f32,
    pub jpeg: Vec<u8>,
}

fn helper(name: &str) -> Result<(JsValue, Function), String> {
    let obj = Reflect::get(&window(), &"notionPdf".into()).map_err(|_| "PDF-Modul fehlt".to_string())?;
    let f = Reflect::get(&obj, &name.into()).ok().and_then(|f| f.dyn_into::<Function>().ok()).ok_or("PDF-Modul fehlt")?;
    Ok((obj, f))
}

fn js_err(e: JsValue) -> String {
    e.as_string()
        .or_else(|| Reflect::get(&e, &"message".into()).ok().and_then(|m| m.as_string()))
        .unwrap_or_else(|| "PDF konnte nicht gelesen werden".into())
}

fn num(o: &JsValue, k: &str) -> f64 {
    Reflect::get(o, &k.into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn bytes_of(o: &JsValue, k: &str) -> Vec<u8> {
    Reflect::get(o, &k.into()).map(|v| Uint8Array::new(&v).to_vec()).unwrap_or_default()
}

/// Alle Seiten eines PDF als JPEG rendern.
pub async fn render_pdf(bytes: Vec<u8>) -> Result<Vec<DocPage>, String> {
    let (obj, f) = helper("render")?;
    let progress = Closure::<dyn FnMut(u32, u32)>::new(|i: u32, n: u32| {
        set_text("busy-text", &format!("PDF wird importiert … Seite {} von {}", i, n));
    });
    show("busy", true);
    set_text("busy-text", "PDF wird geöffnet …");
    let arr = Uint8Array::from(&bytes[..]);
    let res = match f.call3(&obj, &arr, &JsValue::from(1700), progress.as_ref().unchecked_ref()) {
        Ok(p) => JsFuture::from(Promise::from(p)).await,
        Err(e) => Err(e),
    };
    show("busy", false);
    drop(progress);
    let list: Array = res.map_err(js_err)?.unchecked_into();
    Ok(list
        .iter()
        .map(|o| DocPage { w: num(&o, "w") as f32, h: num(&o, "h") as f32, jpeg: bytes_of(&o, "jpeg") })
        .filter(|p| !p.jpeg.is_empty() && p.w > 1.0 && p.h > 1.0)
        .collect())
}

/// Handschrift-PDF: Seiten mit Hintergrund (ohne Handschrift) und herausgelösten Strichen.
pub async fn render_pdf_ink(bytes: Vec<u8>) -> Result<Vec<(DocPage, Vec<crate::engine::Content>, bool)>, String> {
    use crate::engine::{Content, PenKind, Pt, Stroke};
    let (obj, f) = helper("renderInk")?;
    let progress = Closure::<dyn FnMut(u32, u32)>::new(|i: u32, n: u32| {
        set_text("busy-text", &format!("Handschrift wird übernommen … Seite {} von {}", i, n));
    });
    show("busy", true);
    set_text("busy-text", "PDF wird geöffnet …");
    let arr = Uint8Array::from(&bytes[..]);
    let res = match f.call3(&obj, &arr, &JsValue::from(1700), progress.as_ref().unchecked_ref()) {
        Ok(p) => JsFuture::from(Promise::from(p)).await,
        Err(e) => Err(e),
    };
    show("busy", false);
    drop(progress);
    let list: Array = res.map_err(js_err)?.unchecked_into();
    let mut out = Vec::new();
    for o in list.iter() {
        let page = DocPage { w: num(&o, "w") as f32, h: num(&o, "h") as f32, jpeg: bytes_of(&o, "jpeg") };
        if page.jpeg.is_empty() || page.w <= 1.0 || page.h <= 1.0 {
            continue;
        }
        let ok = Reflect::get(&o, &"ok".into()).ok().and_then(|v| v.as_bool()).unwrap_or(false);
        let mut contents = Vec::new();
        let ink: Array = Reflect::get(&o, &"ink".into()).map(|v| v.unchecked_into()).unwrap_or_default();
        for it in ink.iter() {
            let c: Array = Reflect::get(&it, &"c".into()).map(|v| v.unchecked_into()).unwrap_or_default();
            let ch = |k: u32| c.get(k).as_f64().unwrap_or(0.0).clamp(0.0, 255.0) as u8;
            let color = [ch(0), ch(1), ch(2), ch(3)];
            let fill = Reflect::get(&it, &"fill".into()).ok().and_then(|v| v.as_bool()).unwrap_or(true);
            let width = num(&it, "w") as f32;
            let p: Vec<f32> = Reflect::get(&it, &"p".into()).map(|v| js_sys::Float32Array::new(&v).to_vec()).unwrap_or_default();
            // Teilpfade (durch NaN getrennt)
            let mut subs: Vec<Vec<(f32, f32)>> = vec![Vec::new()];
            for q in p.chunks_exact(2) {
                if q[0].is_nan() || q[1].is_nan() {
                    subs.push(Vec::new());
                } else if let Some(last) = subs.last_mut() {
                    last.push((q[0], q[1]));
                }
            }
            subs.retain(|s| s.len() >= 2);
            if subs.is_empty() {
                continue;
            }
            if fill {
                let mut pts = Vec::new();
                for sp in subs.iter().filter(|s| s.len() >= 3) {
                    for (k, &(x, y)) in sp.iter().enumerate() {
                        pts.push(Pt::new(x, y, if k == 0 && !pts.is_empty() { 0.0 } else { 1.0 }));
                    }
                }
                if pts.len() >= 3 {
                    contents.push(Content::Stroke(Stroke { pen: PenKind::Outline, color, width: 0.0, pts }));
                }
            } else {
                // gezogene Linien (z. B. Bruchstriche) → normale Striche
                for sp in subs {
                    let pts = sp.iter().map(|&(x, y)| Pt::new(x, y, 0.6)).collect();
                    contents.push(Content::Stroke(Stroke { pen: PenKind::Ball, color, width: width.max(0.4), pts }));
                }
            }
        }
        out.push((page, contents, ok));
    }
    Ok(out)
}

fn bg_item(jpeg: Vec<u8>, w: f32, h: f32) -> ImageItem {
    ImageItem { x: 0.0, y: 0.0, w, h, key: hash_bytes(&jpeg), png: Rc::new(jpeg) }
}

impl App {
    /// Handschrift-PDF (z. B. aus GoodNotes) als neues Notizbuch: Hintergrund als Bild,
    /// jeder Handschrift-Strich als eigenes, radierbares Element.
    pub fn import_pdf_ink(&mut self, name: String, bytes: Vec<u8>) {
        let folder = self.folder.clone().unwrap_or_default();
        spawn_local(async move {
            let _ = match render_pdf_ink(bytes).await {
                Ok(pages) if !pages.is_empty() => with_app(|a| {
                    let (fw, fh) = (pages[0].0.w.clamp(100.0, 3000.0), pages[0].0.h.clamp(100.0, 3000.0));
                    let title: String = name.rsplit_once('.').map(|(t, _)| t.to_string()).unwrap_or(name.clone());
                    let cover = (js_sys::Math::random() * COVERS.len() as f64) as u8 % COVERS.len() as u8;
                    let mut nb = Notebook::new(super::storage::new_id(), &title, cover, Format { w: fw, h: fh }, Default::default(), Color::WHITE, wall_clock());
                    nb.folder = folder;
                    nb.pages.clear();
                    let (n, mut strokes, mut flat) = (pages.len(), 0usize, 0usize);
                    for (p, contents, ok) in pages {
                        let id = nb.alloc_id();
                        let mut page = Page::new(id, Default::default(), Color::WHITE);
                        // andere Seitengröße als die erste → Hintergrund in die Seite einpassen
                        page.bg = Some(bg_item(p.jpeg, fw, fh));
                        let sx = fw / p.w.max(1.0);
                        let sy = fh / p.h.max(1.0);
                        let xf = crate::engine::Xf::scale_about(sx, sy, 0.0, 0.0);
                        let mut items = Vec::new();
                        for (k, c) in contents.into_iter().enumerate() {
                            let iid = nb.alloc_id();
                            let it = crate::engine::Item { id: iid, order: k as f64 + 1.0, content: c, pinned: false };
                            items.push(if (sx - 1.0).abs() > 1e-3 || (sy - 1.0).abs() > 1e-3 { it.transformed(&xf) } else { it });
                        }
                        strokes += items.len();
                        if !ok {
                            flat += 1;
                        }
                        page.insert(items);
                        nb.pages.push(page);
                    }
                    a.add_notebook(nb, false);
                    let msg = if strokes == 0 {
                        format!("PDF importiert – {} {}, aber keine Handschrift als Vektoren gefunden (nur als Bild)", n, if n == 1 { "Seite" } else { "Seiten" })
                    } else if flat > 0 {
                        format!("{} Striche übernommen – {} {} nur als Bild", strokes, flat, if flat == 1 { "Seite" } else { "Seiten" })
                    } else {
                        format!("Handschrift übernommen – {} Striche auf {} {}, einzeln radierbar", strokes, n, if n == 1 { "Seite" } else { "Seiten" })
                    };
                    a.toast(&msg);
                }),
                Ok(_) => with_app(|a| a.toast("Das PDF enthält keine Seiten")),
                Err(e) => with_app(|a| a.toast(&e)),
            };
        });
    }

    /// PDF als neues Notizbuch (jede PDF-Seite wird zum Hintergrund einer Seite).
    pub fn import_pdf(&mut self, name: String, bytes: Vec<u8>) {
        let folder = self.folder.clone().unwrap_or_default();
        spawn_local(async move {
            let _ = match render_pdf(bytes).await {
                Ok(pages) if !pages.is_empty() => with_app(|a| {
                    let (fw, fh) = (pages[0].w.clamp(100.0, 3000.0), pages[0].h.clamp(100.0, 3000.0));
                    let title: String = name.rsplit_once('.').map(|(t, _)| t.to_string()).unwrap_or(name.clone());
                    let cover = (js_sys::Math::random() * COVERS.len() as f64) as u8 % COVERS.len() as u8;
                    let mut nb = Notebook::new(super::storage::new_id(), &title, cover, Format { w: fw, h: fh }, Default::default(), Color::WHITE, wall_clock());
                    nb.folder = folder;
                    nb.pages.clear();
                    let n = pages.len();
                    for p in pages {
                        let id = nb.alloc_id();
                        let mut page = Page::new(id, Default::default(), Color::WHITE);
                        page.bg = Some(bg_item(p.jpeg, fw, fh));
                        nb.pages.push(page);
                    }
                    a.add_notebook(nb, false);
                    a.toast(&format!("PDF importiert – {} {}. Du kannst jetzt direkt hineinschreiben.", n, if n == 1 { "Seite" } else { "Seiten" }));
                }),
                Ok(_) => with_app(|a| a.toast("Das PDF enthält keine Seiten")),
                Err(e) => with_app(|a| a.toast(&e)),
            };
        });
    }

    /// PDF-Seiten hinter der aktuellen Seite einfügen (rückgängig machbar).
    pub fn append_pdf(&mut self, bytes: Vec<u8>) {
        spawn_local(async move {
            let _ = match render_pdf(bytes).await {
                Ok(pages) => with_app(|a| {
                    let Some(ed) = &mut a.ed else { return };
                    let (w, h) = (ed.nb.format.w, ed.nb.format.h);
                    let mut group = Vec::new();
                    let n = pages.len();
                    for (at, p) in (ed.cur_page + 1..).zip(pages) {
                        let id = ed.nb.alloc_id();
                        let mut page = Page::new(id, Default::default(), Color::WHITE);
                        page.bg = Some(bg_item(p.jpeg, w, h));
                        ed.nb.pages.insert(at, page.clone());
                        group.push(Action::AddPage { index: at, page });
                    }
                    if n > 0 {
                        ed.push(Action::Group(group));
                        let next = ed.cur_page + 1;
                        ed.goto_page(next);
                    }
                    a.toast(&format!("{} PDF-{} eingefügt", n, if n == 1 { "Seite" } else { "Seiten" }));
                    a.after_pages_change();
                }),
                Err(e) => with_app(|a| a.toast(&e)),
            };
        });
    }

    /// Bild als eigenes Papier für die aktuelle oder alle Seiten.
    pub fn set_paper_image(&mut self, file: web_sys::File, all: bool) {
        spawn_local(async move {
            let res = match helper("image") {
                Ok((obj, f)) => match f.call2(&obj, &file, &JsValue::from(2000)) {
                    Ok(p) => JsFuture::from(Promise::from(p)).await.map_err(js_err),
                    Err(e) => Err(js_err(e)),
                },
                Err(e) => Err(e),
            };
            let _ = with_app(|a| match res {
                Ok(o) => {
                    let jpeg = bytes_of(&o, "jpeg");
                    let Some(ed) = &mut a.ed else { return };
                    let (w, h) = ed.nb.page_size(ed.cur_page);
                    let img = bg_item(jpeg, w, h);
                    let idx: Vec<usize> = if all { (0..ed.nb.pages.len()).collect() } else { vec![ed.cur_page] };
                    ed.update_pages_meta(&idx, |m| {
                        m.bg = Some(img.clone());
                        m.bg_paper = true;
                    });
                    a.close_modals();
                    a.toast(if all { "Eigenes Papier für alle Seiten gesetzt" } else { "Eigenes Papier für diese Seite gesetzt" });
                    a.after_pages_change();
                }
                Err(e) => a.toast(&e),
            });
        });
    }

    pub fn clear_paper(&mut self) {
        let Some(ed) = &mut self.ed else { return };
        let i = ed.cur_page;
        if ed.nb.pages[i].bg.is_none() {
            self.toast("Diese Seite hat keinen eigenen Hintergrund");
            return;
        }
        ed.update_pages_meta(&[i], |m| {
            m.bg = None;
            m.bg_paper = false;
        });
        self.close_modals();
        self.toast("Hintergrund entfernt – Rückgängig ist möglich");
        self.after_pages_change();
    }
}

impl super::editor::Editor {
    /// Seiteneigenschaften mehrerer Seiten in einer rückgängig machbaren Aktion ändern.
    pub fn update_pages_meta(&mut self, idx: &[usize], f: impl Fn(&mut PageMeta)) {
        let mut group = Vec::new();
        for &i in idx {
            let Some(p) = self.nb.pages.get_mut(i) else { continue };
            let before = PageMeta::of(p);
            let mut after = before.clone();
            f(&mut after);
            if after != before {
                after.apply(p);
                self.bmps.remove(&p.id);
                group.push(Action::PageMeta { page: p.id, before, after });
            }
        }
        if !group.is_empty() {
            self.push(Action::Group(group));
            self.dirty = true;
            self.thumbs_dirty = true;
        }
    }
}
