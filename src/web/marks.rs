//! Farbige Seitenmarkierungen und Schlagwörter (Dialog, Anzeige, Filter in der Seitenübersicht).

use super::*;
use crate::engine::note::MARK_COLORS;

impl App {
    pub fn open_marks_dialog(&mut self) {
        self.open_modal("modal-marks");
        self.refresh_marks_dialog();
    }

    /// Alle Schlagwörter im Notizbuch (sortiert, ohne Doppelte).
    fn all_tags(&self) -> Vec<String> {
        let Some(ed) = &self.ed else { return Vec::new() };
        let mut v: Vec<String> = ed.nb.pages.iter().flat_map(|p| p.tags.iter().cloned()).collect();
        v.sort_by_key(|t| t.to_lowercase());
        v.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        v
    }

    pub fn refresh_marks_dialog(&mut self) {
        let all = self.all_tags();
        let Some(ed) = &self.ed else { return };
        let p = &ed.nb.pages[ed.cur_page];
        set_text("marks-page", &format!("Seite {}{}", ed.cur_page + 1, if p.title.is_empty() { String::new() } else { format!(" · {}", p.title) }));
        let mut c = format!("<button class=\"mark-sw none{}\" data-act=\"mark-set\" data-arg=\"0\" title=\"Keine Markierung\"></button>", if p.mark == 0 { " active" } else { "" });
        for (i, (name, col)) in MARK_COLORS.iter().enumerate() {
            c.push_str(&format!(
                "<button class=\"mark-sw{}\" data-act=\"mark-set\" data-arg=\"{}\" title=\"{}\" style=\"background:{}\"></button>",
                if p.mark as usize == i + 1 { " active" } else { "" },
                i + 1,
                name,
                col
            ));
        }
        el("marks-colors").set_inner_html(&c);
        let chips: String = p
            .tags
            .iter()
            .map(|t| format!("<span class=\"tag-chip on\">#{t}<button data-act=\"tag-del\" data-arg=\"{t}\" title=\"Entfernen\"><svg><use href=\"#i-close\"/></svg></button></span>", t = esc(t)))
            .collect();
        el("marks-tags").set_inner_html(if chips.is_empty() { "<span class=\"muted\" style=\"font-size:13px\">Noch keine Schlagwörter</span>" } else { &chips });
        let sugg: String = all
            .iter()
            .filter(|t| !p.tags.iter().any(|x| x.eq_ignore_ascii_case(t)))
            .map(|t| format!("<button class=\"tag-chip\" data-act=\"tag-add\" data-arg=\"{t}\">+ #{t}</button>", t = esc(t)))
            .collect();
        el("marks-sugg").set_inner_html(&sugg);
        show("marks-sugg-wrap", !sugg.is_empty());
    }

    pub fn marks_action(&mut self, act: &str, arg: &str) {
        let Some(ed) = &mut self.ed else { return };
        let i = ed.cur_page;
        match act {
            "mark-set" => {
                let m = arg.parse::<u8>().unwrap_or(0).min(MARK_COLORS.len() as u8);
                ed.set_page_meta(i, |pm| pm.mark = m);
            }
            "tag-add" | "tag-new" => {
                let raw = if act == "tag-new" { input_el("marks-input").value() } else { arg.to_string() };
                let mut added = false;
                for t in raw.split([',', ';']) {
                    let t: String = t.trim().trim_start_matches('#').trim().chars().take(30).collect();
                    if t.is_empty() {
                        continue;
                    }
                    ed.set_page_meta(i, |pm| {
                        if !pm.tags.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
                            pm.tags.push(t.clone());
                        }
                    });
                    added = true;
                }
                if act == "tag-new" {
                    input_el("marks-input").set_value("");
                    if !added {
                        return;
                    }
                }
            }
            "tag-del" => {
                let t = arg.to_string();
                ed.set_page_meta(i, |pm| pm.tags.retain(|x| *x != t));
            }
            _ => return,
        }
        ed.dirty = true;
        ed.thumbs_dirty = true;
        self.refresh_marks_dialog();
        self.update_pill();
        if self.ed.as_ref().map(|e| e.nav_open).unwrap_or(false) {
            self.render_nav();
        }
        self.after_edit();
    }

    /// Filterleiste der Seitenübersicht (Farben und Schlagwörter, die vorkommen).
    pub fn nav_filter_bar(&self) -> String {
        let Some(ed) = &self.ed else { return String::new() };
        let mut used: Vec<u8> = ed.nb.pages.iter().map(|p| p.mark).filter(|m| *m > 0).collect();
        used.sort_unstable();
        used.dedup();
        let tags = self.all_tags();
        if used.is_empty() && tags.is_empty() {
            return String::new();
        }
        let f = ed.nav_filter.as_deref();
        let mut h = String::from("<div class=\"nav-filter\">");
        for m in used {
            let key = format!("c:{}", m);
            h.push_str(&format!(
                "<button class=\"mark-sw small{}\" data-act=\"nav-mark\" data-arg=\"{}\" title=\"{}\" style=\"background:{}\"></button>",
                if f == Some(key.as_str()) { " active" } else { "" },
                key,
                MARK_COLORS[(m - 1) as usize].0,
                MARK_COLORS[(m - 1) as usize].1
            ));
        }
        for t in tags {
            let key = format!("t:{}", t);
            h.push_str(&format!("<button class=\"tag-chip{}\" data-act=\"nav-mark\" data-arg=\"{}\">#{}</button>", if f == Some(key.as_str()) { " on" } else { "" }, esc(&key), esc(&t)));
        }
        h.push_str("</div>");
        h
    }

    pub fn nav_mark_filter(&mut self, key: &str) {
        if let Some(ed) = &mut self.ed {
            ed.nav_filter = if ed.nav_filter.as_deref() == Some(key) { None } else { Some(key.to_string()) };
            ed.nav_toc = false;
        }
        self.render_nav();
    }
}

/// Passt die Seite zum Filter der Seitenübersicht?
pub fn page_matches(p: &crate::engine::Page, f: Option<&str>) -> bool {
    match f.and_then(|f| f.split_once(':')) {
        Some(("c", m)) => m.parse::<u8>().map(|m| p.mark == m).unwrap_or(true),
        Some(("t", t)) => p.tags.iter().any(|x| x.eq_ignore_ascii_case(t)),
        _ => true,
    }
}
