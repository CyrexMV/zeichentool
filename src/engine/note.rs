//! Notizbuch-Modell: Notizbuch → Seiten → Elemente (Striche, Text, Bilder).

use super::color::Color;
use super::geom::{Rect, Xf};
use super::ink::Stroke;
use super::paper::Template;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub struct TextBox {
    pub x: f32,
    pub y: f32,
    /// Breite des Textfelds (pt)
    pub w: f32,
    /// Schriftgröße (pt)
    pub size: f32,
    pub color: [u8; 4],
    pub bold: bool,
    pub text: String,
}

impl TextBox {
    /// Grobe Höhe (für Treffertests); die echte Höhe kennt erst der Textsatz.
    pub fn approx_rect(&self, lines: usize) -> Rect {
        Rect::new(self.x, self.y, self.x + self.w, self.y + self.size * 1.35 * lines.max(1) as f32)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImageItem {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// PNG-Daten (geteilt, damit Verlauf/Kopien keinen Speicher verdoppeln)
    pub png: Rc<Vec<u8>>,
    /// Inhalts-Prüfsumme (zum Zwischenspeichern im Browser)
    pub key: u64,
}

pub fn hash_bytes(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &x in b {
        h ^= x as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    Stroke(Stroke),
    Text(TextBox),
    Image(ImageItem),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: u32,
    /// Zeichenreihenfolge (aufsteigend)
    pub order: f64,
    pub content: Content,
}

impl Item {
    pub fn bbox(&self) -> Rect {
        match &self.content {
            Content::Stroke(s) => s.bbox(),
            Content::Text(t) => t.approx_rect(t.text.lines().count().max(1)),
            Content::Image(i) => Rect::new(i.x, i.y, i.x + i.w, i.y + i.h),
        }
    }

    pub fn transformed(&self, xf: &Xf) -> Item {
        let content = match &self.content {
            Content::Stroke(s) => Content::Stroke(s.transformed(xf)),
            Content::Text(t) => {
                let (x, y) = xf.apply(t.x, t.y);
                let s = xf.scale();
                Content::Text(TextBox { x, y, w: t.w * s, size: t.size * s, ..t.clone() })
            }
            Content::Image(i) => {
                let (x0, y0) = xf.apply(i.x, i.y);
                let (x1, y1) = xf.apply(i.x + i.w, i.y + i.h);
                let r = Rect::new(x0, y0, x1, y1);
                Content::Image(ImageItem { x: r.x0, y: r.y0, w: r.w(), h: r.h(), ..i.clone() })
            }
        };
        Item { id: self.id, order: self.order, content }
    }

    pub fn is_highlighter(&self) -> bool {
        matches!(&self.content, Content::Stroke(s) if s.pen == super::ink::PenKind::Highlighter)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub id: u32,
    pub template: Template,
    pub paper: Color,
    pub bookmark: bool,
    /// nach `order` sortiert
    pub items: Vec<Item>,
    /// Laufzeit-Zähler für Zwischenspeicher (nicht gespeichert)
    pub version: u64,
}

impl Page {
    pub fn new(id: u32, template: Template, paper: Color) -> Page {
        Page { id, template, paper, bookmark: false, items: Vec::new(), version: 0 }
    }

    pub fn touch(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    pub fn max_order(&self) -> f64 {
        self.items.last().map(|i| i.order).unwrap_or(0.0)
    }

    /// Fügt Elemente ein und hält die Reihenfolge sortiert.
    pub fn insert(&mut self, items: impl IntoIterator<Item = Item>) {
        for it in items {
            let pos = self.items.partition_point(|x| x.order <= it.order);
            self.items.insert(pos, it);
        }
        self.touch();
    }

    pub fn remove_ids(&mut self, ids: &[u32]) -> Vec<Item> {
        let mut removed = Vec::new();
        self.items.retain(|it| {
            if ids.contains(&it.id) {
                removed.push(it.clone());
                false
            } else {
                true
            }
        });
        if !removed.is_empty() {
            self.touch();
        }
        removed
    }

    pub fn get(&self, id: u32) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn stroke_count(&self) -> usize {
        self.items.iter().filter(|i| matches!(i.content, Content::Stroke(_))).count()
    }
}

/// Seitenformat in pt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Format {
    pub w: f32,
    pub h: f32,
}

impl Format {
    pub const A4_PORTRAIT: Format = Format { w: 595.0, h: 842.0 };
    pub const A4_LANDSCAPE: Format = Format { w: 842.0, h: 595.0 };
    pub const SQUARE: Format = Format { w: 700.0, h: 700.0 };
}

pub const COVERS: [(&str, &str, &str); 10] = [
    ("Violett", "#7c5cff", "#2fb5ff"),
    ("Ozean", "#0f62fe", "#00c2c7"),
    ("Sonnenuntergang", "#ff6b6b", "#ffb36b"),
    ("Wald", "#1d976c", "#93f9b9"),
    ("Mitternacht", "#232526", "#414345"),
    ("Beere", "#c2185b", "#7b1fa2"),
    ("Sand", "#c9a26b", "#f0d9b5"),
    ("Koralle", "#ff5e62", "#ff9966"),
    ("Schiefer", "#5b6b84", "#9aa9c2"),
    ("Minze", "#38ef7d", "#11998e"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Notebook {
    pub id: String,
    pub title: String,
    pub cover: u8,
    pub favorite: bool,
    pub created: f64,
    pub modified: f64,
    pub format: Format,
    pub pages: Vec<Page>,
    pub next_id: u32,
}

impl Notebook {
    pub fn new(id: String, title: &str, cover: u8, format: Format, template: Template, paper: Color, now: f64) -> Notebook {
        let mut nb = Notebook { id, title: title.to_string(), cover, favorite: false, created: now, modified: now, format, pages: Vec::new(), next_id: 1 };
        let pid = nb.alloc_id();
        nb.pages.push(Page::new(pid, template, paper));
        nb
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn page_index(&self, id: u32) -> Option<usize> {
        self.pages.iter().position(|p| p.id == id)
    }

    pub fn page_mut(&mut self, id: u32) -> Option<&mut Page> {
        self.pages.iter_mut().find(|p| p.id == id)
    }

    /// Neue leere Seite mit derselben Vorlage wie `like` (oder der letzten Seite).
    pub fn new_page_like(&mut self, like: usize) -> Page {
        let src = &self.pages[like.min(self.pages.len() - 1)];
        let (t, p) = (src.template, src.paper);
        let id = self.alloc_id();
        Page::new(id, t, p)
    }

    /// Kopie einer Seite mit neuen IDs.
    pub fn duplicate_page(&mut self, index: usize) -> Page {
        let mut p = self.pages[index].clone();
        p.id = self.alloc_id();
        p.bookmark = false;
        for it in p.items.iter_mut() {
            it.id = self.next_id;
            self.next_id += 1;
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ink::{PenKind, Pt};

    fn stroke(order: f64, id: u32) -> Item {
        Item { id, order, content: Content::Stroke(Stroke { pen: PenKind::Ball, color: [0, 0, 0, 255], width: 1.0, pts: vec![Pt::new(0.0, 0.0, 1.0)] }) }
    }

    #[test]
    fn insert_keeps_order() {
        let mut p = Page::new(1, Template::default(), Color::WHITE);
        p.insert([stroke(3.0, 3), stroke(1.0, 1), stroke(2.0, 2)]);
        let ids: Vec<u32> = p.items.iter().map(|i| i.id).collect();
        assert_eq!(ids, vec![1, 2, 3]);
        let removed = p.remove_ids(&[2]);
        assert_eq!(removed.len(), 1);
        assert_eq!(p.items.len(), 2);
    }

    #[test]
    fn duplicate_page_gets_new_ids() {
        let mut nb = Notebook::new("x".into(), "Test", 0, Format::A4_PORTRAIT, Template::default(), Color::WHITE, 0.0);
        let id = nb.alloc_id();
        nb.pages[0].insert([stroke(1.0, id)]);
        let d = nb.duplicate_page(0);
        assert_ne!(d.id, nb.pages[0].id);
        assert_ne!(d.items[0].id, nb.pages[0].items[0].id);
    }
}
