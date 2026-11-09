//! Notizbuch-Modell: Notizbuch → Seiten → Elemente (Striche, Text, Bilder).

use super::color::Color;
use super::geom::{Rect, Xf};
use super::ink::Stroke;
use super::paper::{Template, TemplateKind};
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

/// Mindmap-Blase: abgerundetes Feld mit Text; `parent` = ID der Elternblase (0 = Wurzel).
/// Die Verbindungslinie wird beim Zeichnen aus den Positionen berechnet und folgt deshalb jeder Verschiebung.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub text: String,
    pub color: [u8; 4],
    pub parent: u32,
}

impl Node {
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w * 0.5, self.y + self.h * 0.5)
    }

    /// Verbindung Eltern → Kind als kubische Bézierkurve (von Mitte zu Mitte, waagerecht auslaufend).
    pub fn edge_to(&self, child: &Node) -> [(f32, f32); 4] {
        let (a, b) = (self.center(), child.center());
        let dx = (b.0 - a.0) * 0.5;
        [a, (a.0 + dx, a.1), (b.0 - dx, b.1), b]
    }

    /// Innenabstand links/rechts.
    pub fn pad(&self) -> f32 {
        self.font_size() * 0.8
    }

    /// Schriftgröße passend zur Blasenhöhe.
    pub fn font_size(&self) -> f32 {
        (self.h * 0.32).clamp(8.0, 30.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    Stroke(Stroke),
    Text(TextBox),
    Image(ImageItem),
    Node(Node),
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
            Content::Node(n) => Rect::new(n.x, n.y, n.x + n.w, n.y + n.h),
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
            Content::Node(n) => {
                let (x0, y0) = xf.apply(n.x, n.y);
                let (x1, y1) = xf.apply(n.x + n.w, n.y + n.h);
                let r = Rect::new(x0, y0, x1, y1);
                Content::Node(Node { x: r.x0, y: r.y0, w: r.w(), h: r.h(), ..n.clone() })
            }
        };
        Item { id: self.id, order: self.order, content }
    }

    pub fn is_highlighter(&self) -> bool {
        matches!(&self.content, Content::Stroke(s) if s.pen == super::ink::PenKind::Highlighter)
    }
}

/// Ziel eines Verweises.
#[derive(Clone, Debug, PartialEq)]
pub enum LinkTarget {
    /// Seite in diesem Notizbuch (Seiten-ID)
    Page(u32),
    /// Anderes Notizbuch (ID)
    Notebook(String),
}

/// Verweis: Fläche auf der Seite, die beim Antippen springt.
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub rect: Rect,
    pub target: LinkTarget,
    /// Unsichtbar (z. B. Reiter im Jahresplaner – die Vorlage zeigt sie schon)
    pub hidden: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub id: u32,
    pub template: Template,
    pub paper: Color,
    pub bookmark: bool,
    /// Abgehakte Zeilen der Checkliste (Zeilenindizes)
    pub checks: Vec<u16>,
    /// Seitentitel für das Inhaltsverzeichnis (leer = kein Eintrag)
    pub title: String,
    /// Eigenes Papier (Bild oder PDF-Seite), seitenfüllend
    pub bg: Option<ImageItem>,
    /// Farbmarkierung (0 = keine, sonst Index in `MARK_COLORS` + 1)
    pub mark: u8,
    /// Schlagwörter der Seite
    pub tags: Vec<String>,
    /// Hintergrund ist eigenes Papier (wird für neue Seiten übernommen); sonst Dokument (PDF)
    pub bg_paper: bool,
    /// Verweise auf andere Seiten/Notizbücher
    pub links: Vec<Link>,
    /// nach `order` sortiert
    pub items: Vec<Item>,
    /// Laufzeit-Zähler für Zwischenspeicher (nicht gespeichert)
    pub version: u64,
}

impl Page {
    pub fn new(id: u32, template: Template, paper: Color) -> Page {
        Page { id, template, paper, bookmark: false, checks: Vec::new(), title: String::new(), mark: 0, tags: Vec::new(), bg: None, bg_paper: false, links: Vec::new(), items: Vec::new(), version: 0 }
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

    pub fn is_checked(&self, row: usize) -> bool {
        self.checks.contains(&(row as u16))
    }

    /// Durchstreichen einer abgehakten Zeile: (x0, x1, y) – nur, wenn dort etwas steht.
    pub fn row_strike(&self, row: &super::paper::CheckRow) -> Option<(f32, f32, f32)> {
        let mut r = Rect::EMPTY;
        for it in &self.items {
            if it.is_highlighter() {
                continue;
            }
            let b = it.bbox();
            let (cx, cy) = b.center();
            if cy >= row.y0 && cy < row.y1 && cx > row.bx + row.size && cx < row.x1 {
                r = r.union(&b);
            }
        }
        if r.is_empty() {
            return None;
        }
        let x0 = row.text_x - (row.text_x - row.bx - row.size) * 0.5;
        let y = ((r.y0 + r.y1) * 0.5).clamp(row.y0 + (row.y1 - row.y0) * 0.3, row.y1 - (row.y1 - row.y0) * 0.2);
        Some((x0.min(r.x0), (r.x1 + 4.0).min(row.x1), y))
    }

    /// Verweis an einer Stelle (Seitenkoordinaten).
    pub fn link_at(&self, x: f32, y: f32) -> Option<&Link> {
        self.links.iter().rev().find(|l| l.rect.contains(x, y))
    }

    pub fn stroke_count(&self) -> usize {
        self.items.iter().filter(|i| matches!(i.content, Content::Stroke(_))).count()
    }
}

/// Farben der Seitenmarkierung (Name, Farbe).
pub const MARK_COLORS: [(&str, &str); 7] = [("Rot", "#e5484d"), ("Orange", "#f5a524"), ("Gelb", "#f5d90a"), ("Grün", "#1fbf75"), ("Blau", "#2f8cff"), ("Lila", "#8e5cff"), ("Grau", "#8792a8")];

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

pub const COVERS: [(&str, &str, &str); 23] = [
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
    ("Leder Braun", "#8b5a2b", "#5a3617"),
    ("Leder Schwarz", "#3a3a3d", "#151517"),
    ("Leinen", "#34508a", "#22345e"),
    ("Kraftpapier", "#cfaa74", "#b88f57"),
    ("Marmor", "#f7f6f3", "#e1e1e5"),
    ("Punkte", "#ffd6e0", "#ffffff"),
    ("Streifen", "#eaf3ff", "#3b5bdb"),
    ("Karo", "#b22d2d", "#3d0f12"),
    ("Sternenhimmel", "#0f1c3f", "#3a1f6b"),
    ("Wellen", "#00b4db", "#0062a3"),
    ("Terrazzo", "#f4ede3", "#e8dccb"),
    ("Regenbogen", "#ffadad", "#bdb2ff"),
    ("Retro-Sonne", "#ff7e5f", "#5b2a86"),
];

/// Bereich auf einer Seite (für Lernkarten).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub page: u32,
    pub rect: Rect,
}

/// Lernkarte: Vorder- und Rückseite verweisen auf Bereiche im Notizbuch (immer aktuell).
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub id: u32,
    pub front: Region,
    pub back: Region,
    /// Fach im Karteikasten (0 = neu/nicht gewusst … 4 = sicher)
    pub level: u8,
    pub right: u32,
    pub wrong: u32,
    /// Nächste Wiederholung (ms seit 1970, 0 = sofort fällig)
    pub due: f64,
}

/// Wiederholungsabstand in Tagen je Fach (Karteikasten).
pub const INTERVAL_DAYS: [f64; 5] = [0.0, 1.0, 3.0, 7.0, 21.0];

impl Card {
    pub fn answer(&mut self, known: bool, now: f64) {
        if known {
            self.level = (self.level + 1).min(4);
            self.right += 1;
        } else {
            self.level = 0;
            self.wrong += 1;
        }
        // nicht gewusst: noch heute wieder; sonst nach dem Fach-Abstand
        self.due = now + INTERVAL_DAYS[self.level as usize] * 86_400_000.0 - 3_600_000.0;
    }

    pub fn is_due(&self, now: f64) -> bool {
        self.due <= now
    }
}

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
    /// Ordner in der Bibliothek (leer = oberste Ebene)
    pub folder: String,
    /// Als Notizbuchvorlage gespeichert (erscheint nicht in der Bibliothek)
    pub is_template: bool,
    pub cards: Vec<Card>,
    /// PIN-Sperre: Prüfsumme (leer = nicht gesperrt)
    pub lock: String,
    /// Im Papierkorb seit (ms), 0 = nicht gelöscht
    pub trashed: f64,
    /// Lernkarten-Set (erscheint nur bei den Lernkarten, nicht bei den Notizbüchern)
    pub is_deck: bool,
}

impl Notebook {
    pub fn new(id: String, title: &str, cover: u8, format: Format, template: Template, paper: Color, now: f64) -> Notebook {
        let mut nb = Notebook { id, title: title.to_string(), cover, favorite: false, created: now, modified: now, format, pages: Vec::new(), next_id: 1, folder: String::new(), is_template: false, cards: Vec::new(), lock: String::new(), trashed: 0.0, is_deck: false };
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
        let bg = if src.bg_paper { src.bg.clone() } else { None };
        let id = self.alloc_id();
        let mut page = Page::new(id, t, p);
        if bg.is_some() {
            page.bg = bg;
            page.bg_paper = true;
        }
        page
    }

    /// Übernimmt eine Seite aus einem anderen Notizbuch (neue IDs, Seitenformat bleibt das dieses Notizbuchs).
    pub fn adopt_page(&mut self, src: &Page) -> Page {
        let mut p = src.clone();
        p.id = self.alloc_id();
        let mut map = Vec::new();
        for it in p.items.iter_mut() {
            map.push((it.id, self.next_id));
            it.id = self.next_id;
            self.next_id += 1;
        }
        remap_parents(&mut p.items, &map);
        p.touch();
        p
    }

    /// Kopie dieses Notizbuchs mit neuer ID (für Vorlagen und „Duplizieren“).
    pub fn copy_as(&self, id: String, title: &str, now: f64) -> Notebook {
        let mut nb = self.clone();
        nb.id = id;
        nb.title = title.to_string();
        nb.created = now;
        nb.modified = now;
        nb.favorite = false;
        nb.is_template = false;
        nb.lock.clear();
        nb.trashed = 0.0;
        for c in nb.cards.iter_mut() {
            c.level = 0;
            c.right = 0;
            c.wrong = 0;
            c.due = 0.0;
        }
        nb
    }

    /// Lernkarten entfernen, deren Seite nicht mehr existiert.
    pub fn prune_cards(&mut self) {
        let ids: Vec<u32> = self.pages.iter().map(|p| p.id).collect();
        self.cards.retain(|c| ids.contains(&c.front.page) && ids.contains(&c.back.page));
    }

    /// Bereiche (Frage, Antwort) einer Seite mit der Vorlage „Lernkarte“.
    pub fn flashcard_regions(&self, page: u32) -> (Region, Region) {
        let ((a0, a1, a2, a3), (b0, b1, b2, b3)) = Template::flashcard_halves(self.format.w, self.format.h);
        (Region { page, rect: Rect::new(a0, a1, a2, a3) }, Region { page, rect: Rect::new(b0, b1, b2, b3) })
    }

    pub fn is_auto_card(&self, c: &Card) -> bool {
        c.front.page == c.back.page && (c.front, c.back) == self.flashcard_regions(c.front.page)
    }

    /// Jede Seite mit der Vorlage „Lernkarte“ ist automatisch eine Karte (oben/links Frage,
    /// unten/rechts Antwort). Legt fehlende Karten an und entfernt verwaiste.
    pub fn sync_template_cards(&mut self) {
        let pages: Vec<u32> = self.pages.iter().filter(|p| p.template.kind == TemplateKind::Flashcard).map(|p| p.id).collect();
        let keep: Vec<bool> = self.cards.iter().map(|c| !self.is_auto_card(c) || pages.contains(&c.front.page)).collect();
        let mut k = keep.iter();
        self.cards.retain(|_| *k.next().unwrap_or(&true));
        for pid in pages {
            if !self.cards.iter().any(|c| c.front.page == pid && self.is_auto_card(c)) {
                let (front, back) = self.flashcard_regions(pid);
                let id = self.alloc_id();
                self.cards.push(Card { id, front, back, level: 0, right: 0, wrong: 0, due: 0.0 });
            }
        }
    }

    pub fn region_has_content(&self, r: &Region) -> bool {
        self.pages.iter().find(|p| p.id == r.page).map(|p| p.items.iter().any(|it| it.bbox().intersects(&r.rect))).unwrap_or(false)
    }

    /// Karten, die sich lernen lassen (Seiten vorhanden, Frage oder Antwort beschrieben).
    pub fn study_cards(&self) -> Vec<&Card> {
        self.cards
            .iter()
            .filter(|c| self.page_index(c.front.page).is_some() && self.page_index(c.back.page).is_some())
            .filter(|c| self.region_has_content(&c.front) || self.region_has_content(&c.back))
            .collect()
    }

    /// Hat das Notizbuch ein Deckblatt (erste Seite mit dem Cover)?
    pub fn has_cover_page(&self) -> bool {
        self.pages.first().map(|p| p.template.kind == TemplateKind::Cover).unwrap_or(false)
    }

    /// Neues Deckblatt (Cover + Titelschild mit dem Titel als Textfeld) – noch nicht eingefügt.
    pub fn make_cover_page(&mut self) -> Page {
        let id = self.alloc_id();
        let mut t = Template::new(TemplateKind::Cover, 1);
        t.plan.idx = self.cover as u16;
        let mut p = Page::new(id, t, Color::WHITE);
        let (w, h) = (self.format.w, self.format.h);
        let (x, y, pw, ph) = super::covers::title_plate(w, h);
        let s = w.min(h) / 20.0;
        let size = (ph * 0.3).min(s * 1.5);
        let tid = self.alloc_id();
        p.insert([Item {
            id: tid,
            order: 1.0,
            content: Content::Text(TextBox { x: x + s * 0.6, y: y + ph * 0.5 - size * 0.75, w: pw - s * 1.2, size, color: [28, 30, 44, 255], bold: true, text: self.title.clone() }),
        }]);
        p
    }

    /// Titel ändern; ein unverändertes Titelfeld auf dem Deckblatt wandert mit. `true`, wenn sich das Deckblatt geändert hat.
    pub fn set_title(&mut self, title: &str) -> bool {
        let old = std::mem::replace(&mut self.title, title.to_string());
        if !self.has_cover_page() {
            return false;
        }
        let p = &mut self.pages[0];
        let mut changed = false;
        for it in p.items.iter_mut() {
            if let Content::Text(t) = &mut it.content {
                if t.text == old {
                    t.text = title.to_string();
                    changed = true;
                    break;
                }
            }
        }
        if changed {
            p.touch();
        }
        changed
    }

    /// Cover wechseln (auch auf dem Deckblatt).
    pub fn set_cover(&mut self, idx: u8) {
        self.cover = idx;
        if self.has_cover_page() {
            self.pages[0].template.plan.idx = idx as u16;
            self.pages[0].touch();
        }
    }

    /// Kopie einer Seite mit neuen IDs.
    pub fn duplicate_page(&mut self, index: usize) -> Page {
        let mut p = self.pages[index].clone();
        p.id = self.alloc_id();
        p.bookmark = false;
        p.checks.clear();
        p.links.retain(|l| matches!(l.target, LinkTarget::Notebook(_)));
        if !p.title.is_empty() {
            p.title = format!("{} (Kopie)", p.title);
        }
        let mut map = Vec::new();
        for it in p.items.iter_mut() {
            map.push((it.id, self.next_id));
            it.id = self.next_id;
            self.next_id += 1;
        }
        remap_parents(&mut p.items, &map);
        p
    }
}

/// Mindmap-Verbindungen nach dem Umnummerieren anpassen (alte → neue ID).
/// Blasen, deren Eltern nicht mitkopiert wurden, werden zu Wurzeln.
pub fn remap_parents(items: &mut [Item], map: &[(u32, u32)]) {
    for it in items.iter_mut() {
        if let Content::Node(n) = &mut it.content {
            if n.parent != 0 {
                n.parent = map.iter().find(|(o, _)| *o == n.parent).map(|(_, nw)| *nw).unwrap_or(0);
            }
        }
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
    fn checklist_strike_follows_handwriting() {
        let t = Template::new(TemplateKind::Checklist, 1);
        let rows = t.check_rows(595.0, 842.0);
        let r = rows[2];
        let mut p = Page::new(1, t, Color::WHITE);
        let y = (r.y0 + r.y1) * 0.5;
        p.insert([Item { id: 1, order: 1.0, content: Content::Stroke(Stroke { pen: PenKind::Ball, color: [0, 0, 0, 255], width: 1.0, pts: vec![Pt::new(r.text_x + 5.0, y, 1.0), Pt::new(300.0, y, 1.0)] }) }]);
        let (x0, x1, sy) = p.row_strike(&r).expect("Zeile mit Schrift wird durchgestrichen");
        assert!(x0 < r.text_x + 5.0 && x1 > 300.0);
        assert!(sy > r.y0 && sy < r.y1);
        assert!(p.row_strike(&rows[0]).is_none(), "leere Zeile ohne Strich");
    }

    #[test]
    fn flashcard_pages_become_cards() {
        let t = Template::new(TemplateKind::Flashcard, 1);
        let mut nb = Notebook::new("x".into(), "Vokabeln", 0, Format { w: 420.0, h: 595.0 }, t, Color::WHITE, 0.0);
        nb.sync_template_cards();
        assert_eq!(nb.cards.len(), 1);
        assert!(nb.study_cards().is_empty(), "leere Karte wird nicht abgefragt");
        let id = nb.alloc_id();
        nb.pages[0].insert([stroke(1.0, id)]);
        assert_eq!(nb.study_cards().len(), 1);
        let p = nb.new_page_like(0);
        nb.pages.push(p);
        nb.sync_template_cards();
        nb.sync_template_cards();
        assert_eq!(nb.cards.len(), 2);
        nb.pages.remove(1);
        nb.sync_template_cards();
        assert_eq!(nb.cards.len(), 1);
        nb.pages[0].template.kind = TemplateKind::Lined;
        nb.sync_template_cards();
        assert!(nb.cards.is_empty());
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
