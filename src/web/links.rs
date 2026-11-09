//! Verweise: Bereich per Lasso markieren → auf Seite oder Notizbuch verlinken; antippen springt.

use super::*;
use crate::engine::format::NoteMeta;
use crate::engine::{Link, LinkTarget, Rect};

impl App {
    /// Auswahldialog für das Linkziel (aus dem Lasso-Menü).
    pub fn open_link_dialog(&mut self) {
        let Some(ed) = &self.ed else { return };
        let Some(sel) = &ed.selection else { return };
        let (page, rect) = (sel.page, sel.bbox.expand(4.0));
        let Some(pi) = ed.nb.page_index(page) else { return };
        let existing = ed.nb.pages[pi].links.iter().any(|l| !l.hidden && l.rect.intersects(&rect));
        let mut items = String::new();
        if existing {
            items.push_str("<button class=\"pick-item danger\" data-act=\"pick\" data-arg=\"x\"><span class=\"fico\"><svg><use href=\"#i-close\"/></svg></span><span>Link entfernen<small>Vorhandenen Link an dieser Stelle löschen</small></span></button>");
        }
        items.push_str("<p class=\"pick-sec\">Seite in diesem Notizbuch</p>");
        for (i, p) in ed.nb.pages.iter().enumerate() {
            if p.id == page {
                continue;
            }
            let label = if p.title.is_empty() { format!("Seite {}", i + 1) } else { format!("Seite {} · {}", i + 1, esc(&p.title)) };
            items.push_str(&format!("<button class=\"pick-item\" data-act=\"pick\" data-arg=\"p:{}\"><span class=\"fico\"><svg><use href=\"#i-pages\"/></svg></span><span>{}</span></button>", p.id, label));
        }
        let cur = ed.nb.id.clone();
        let mut others: Vec<&NoteMeta> = self.library.iter().filter(|m| m.visible() && m.id != cur).collect();
        others.sort_by_key(|m| m.title.to_lowercase());
        if !others.is_empty() {
            items.push_str("<p class=\"pick-sec\">Anderes Notizbuch</p>");
            for m in others {
                items.push_str(&format!(
                    "<button class=\"pick-item\" data-act=\"pick\" data-arg=\"n:{}\"><span class=\"mini\" style=\"{}\"></span><span>{}<small>{} {}</small></span></button>",
                    esc(&m.id),
                    super::library::cover_css(m.cover),
                    esc(&m.title),
                    m.pages,
                    if m.pages == 1 { "Seite" } else { "Seiten" }
                ));
            }
        }
        self.open_pick("i-link", "Link setzen", "Wohin soll der markierte Bereich führen? Später genügt ein Tipp darauf.", &items, false, Pending::Link(page, rect.x0, rect.y0, rect.x1, rect.y1));
    }

    pub fn create_link(&mut self, page: u32, rect: Rect, value: &str) {
        let Some(ed) = &mut self.ed else { return };
        let Some(pi) = ed.nb.page_index(page) else { return };
        let target = match value.split_once(':') {
            Some(("p", id)) => id.parse().ok().map(LinkTarget::Page),
            Some(("n", id)) => Some(LinkTarget::Notebook(id.to_string())),
            _ => None,
        };
        let msg = if target.is_some() { "Link gesetzt – antippen (Finger oder kurz mit dem Stift) springt zum Ziel" } else { "Link entfernt" };
        ed.update_pages_meta(&[pi], move |m| {
            m.links.retain(|l| l.hidden || !l.rect.intersects(&rect));
            if let Some(t) = &target {
                m.links.push(Link { rect, target: t.clone(), hidden: false });
            }
        });
        ed.selection = None;
        show("sel-menu", false);
        self.toast(msg);
        self.after_edit();
    }

    /// Verweis an einem Bildschirmpunkt?
    pub fn link_at_screen(&self, sx: f64, sy: f64) -> Option<LinkTarget> {
        let ed = self.ed.as_ref()?;
        let (i, x, y) = ed.hit_page(sx, sy, 0.0)?;
        ed.nb.pages[i].link_at(x, y).map(|l| l.target.clone())
    }

    /// Antippen: folgt einem Verweis, falls dort einer ist.
    pub fn tap_link(&mut self, sx: f64, sy: f64) -> bool {
        let Some(t) = self.link_at_screen(sx, sy) else { return false };
        self.follow_link(t);
        true
    }

    pub fn follow_link(&mut self, t: LinkTarget) {
        match t {
            LinkTarget::Page(id) => {
                let Some(ed) = &mut self.ed else { return };
                let Some(i) = ed.nb.page_index(id) else {
                    self.toast("Die verlinkte Seite gibt es nicht mehr");
                    return;
                };
                ed.selection = None;
                ed.goto_page(i);
                let label = if ed.nb.pages[i].title.is_empty() { format!("Seite {}", i + 1) } else { ed.nb.pages[i].title.clone() };
                show("sel-menu", false);
                self.toast(&format!("→ {}", label));
                self.after_view_change();
            }
            LinkTarget::Notebook(id) => {
                if !self.library.iter().any(|m| m.id == id && m.trashed == 0.0) {
                    self.toast("Das verlinkte Notizbuch gibt es nicht mehr");
                    return;
                }
                self.close_editor();
                self.open_guarded(id);
            }
        }
    }
}
