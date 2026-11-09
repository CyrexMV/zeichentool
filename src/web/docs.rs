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

fn bg_item(jpeg: Vec<u8>, w: f32, h: f32) -> ImageItem {
    ImageItem { x: 0.0, y: 0.0, w, h, key: hash_bytes(&jpeg), png: Rc::new(jpeg) }
}

impl App {
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
                    let (w, h) = (ed.nb.format.w, ed.nb.format.h);
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
