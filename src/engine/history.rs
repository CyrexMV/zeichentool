//! Rückgängig/Wiederholen für Notizbücher.

use super::color::Color;
use super::note::{ImageItem, Item, Link, Notebook, Page};
use super::paper::Template;

#[derive(Clone, Debug, PartialEq)]
pub struct PageMeta {
    pub template: Template,
    pub paper: Color,
    pub bookmark: bool,
    pub checks: Vec<u16>,
    pub title: String,
    pub mark: u8,
    pub tags: Vec<String>,
    pub bg: Option<ImageItem>,
    pub bg_paper: bool,
    pub links: Vec<Link>,
    pub land: bool,
}

impl PageMeta {
    pub fn of(p: &Page) -> PageMeta {
        PageMeta { template: p.template, paper: p.paper, bookmark: p.bookmark, checks: p.checks.clone(), title: p.title.clone(), mark: p.mark, tags: p.tags.clone(), bg: p.bg.clone(), bg_paper: p.bg_paper, links: p.links.clone(), land: p.land }
    }

    pub fn apply(&self, p: &mut Page) {
        p.template = self.template;
        p.paper = self.paper;
        p.bookmark = self.bookmark;
        p.checks = self.checks.clone();
        p.title = self.title.clone();
        p.mark = self.mark;
        p.tags = self.tags.clone();
        p.bg = self.bg.clone();
        p.bg_paper = self.bg_paper;
        p.links = self.links.clone();
        p.land = self.land;
        p.touch();
    }
}

#[derive(Clone, Debug)]
pub enum Action {
    /// Elemente einer Seite ersetzt: `removed` (alte Fassungen) ↔ `added` (neue Fassungen)
    Items { page: u32, removed: Vec<Item>, added: Vec<Item> },
    AddPage { index: usize, page: Page },
    RemovePage { index: usize, page: Page },
    MovePage { from: usize, to: usize },
    PageMeta { page: u32, before: PageMeta, after: PageMeta },
    Group(Vec<Action>),
}

impl Action {
    /// Seite, die nach Rückgängig/Wiederholen angezeigt werden sollte.
    pub fn focus_page(&self, nb: &Notebook) -> Option<usize> {
        match self {
            Action::Items { page, .. } | Action::PageMeta { page, .. } => nb.page_index(*page),
            Action::AddPage { index, .. } | Action::RemovePage { index, .. } => Some((*index).min(nb.pages.len().saturating_sub(1))),
            Action::MovePage { to, .. } => Some(*to),
            Action::Group(v) => v.iter().rev().find_map(|a| a.focus_page(nb)),
        }
    }

    fn apply(&self, nb: &mut Notebook, undo: bool) {
        match self {
            Action::Items { page, removed, added } => {
                if let Some(p) = nb.page_mut(*page) {
                    let (take, put) = if undo { (added, removed) } else { (removed, added) };
                    let ids: Vec<u32> = take.iter().map(|i| i.id).collect();
                    p.remove_ids(&ids);
                    p.insert(put.iter().cloned());
                }
            }
            Action::AddPage { index, page } | Action::RemovePage { index, page } => {
                let insert = matches!(self, Action::AddPage { .. }) != undo;
                if insert {
                    let i = (*index).min(nb.pages.len());
                    let mut p = page.clone();
                    p.touch();
                    nb.pages.insert(i, p);
                } else if let Some(i) = nb.page_index(page.id) {
                    if nb.pages.len() > 1 {
                        nb.pages.remove(i);
                    }
                }
            }
            Action::MovePage { from, to } => {
                let (f, t) = if undo { (*to, *from) } else { (*from, *to) };
                if f < nb.pages.len() && t < nb.pages.len() {
                    let p = nb.pages.remove(f);
                    nb.pages.insert(t, p);
                }
            }
            Action::PageMeta { page, before, after } => {
                if let Some(p) = nb.page_mut(*page) {
                    if undo { before } else { after }.apply(p);
                }
            }
            Action::Group(v) => {
                if undo {
                    v.iter().rev().for_each(|a| a.apply(nb, true));
                } else {
                    v.iter().for_each(|a| a.apply(nb, false));
                }
            }
        }
    }
}

#[derive(Default)]
pub struct History {
    undo: Vec<Action>,
    redo: Vec<Action>,
}

impl History {
    const MAX: usize = 300;

    pub fn push(&mut self, a: Action) {
        self.undo.push(a);
        self.redo.clear();
        if self.undo.len() > Self::MAX {
            self.undo.remove(0);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Gibt die Seite zurück, die betroffen war.
    pub fn undo(&mut self, nb: &mut Notebook) -> Option<Option<usize>> {
        let a = self.undo.pop()?;
        a.apply(nb, true);
        let f = a.focus_page(nb);
        self.redo.push(a);
        Some(f)
    }

    pub fn redo(&mut self, nb: &mut Notebook) -> Option<Option<usize>> {
        let a = self.redo.pop()?;
        a.apply(nb, false);
        let f = a.focus_page(nb);
        self.undo.push(a);
        Some(f)
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ink::{PenKind, Pt, Stroke};
    use crate::engine::note::{Content, Format};

    fn nb() -> Notebook {
        Notebook::new("a".into(), "T", 0, Format::A4_PORTRAIT, Template::default(), Color::WHITE, 0.0)
    }

    fn item(id: u32, x: f32) -> Item {
        Item { id, order: id as f64, content: Content::Stroke(Stroke { pen: PenKind::Ball, color: [0; 4], width: 1.0, pts: vec![Pt::new(x, 0.0, 1.0)] }), pinned: false }
    }

    #[test]
    fn items_undo_redo_and_move() {
        let mut n = nb();
        let pid = n.pages[0].id;
        let mut h = History::default();
        n.pages[0].insert([item(10, 0.0)]);
        h.push(Action::Items { page: pid, removed: vec![], added: vec![item(10, 0.0)] });
        // verschieben (gleiche ID, neue Fassung)
        n.pages[0].remove_ids(&[10]);
        n.pages[0].insert([item(10, 5.0)]);
        h.push(Action::Items { page: pid, removed: vec![item(10, 0.0)], added: vec![item(10, 5.0)] });
        h.undo(&mut n);
        assert_eq!(n.pages[0].items[0], item(10, 0.0));
        h.undo(&mut n);
        assert!(n.pages[0].items.is_empty());
        h.redo(&mut n);
        h.redo(&mut n);
        assert_eq!(n.pages[0].items[0], item(10, 5.0));
    }

    #[test]
    fn pages_undo() {
        let mut n = nb();
        let p = n.new_page_like(0);
        n.pages.push(p.clone());
        let mut h = History::default();
        h.push(Action::AddPage { index: 1, page: p });
        assert_eq!(n.pages.len(), 2);
        h.undo(&mut n);
        assert_eq!(n.pages.len(), 1);
        h.redo(&mut n);
        assert_eq!(n.pages.len(), 2);
    }
}
