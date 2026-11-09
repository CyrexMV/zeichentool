//! Seitenübersicht: mehrere Seiten auswählen und gemeinsam löschen, duplizieren, drehen oder exportieren.

use super::*;
use crate::engine::Action;

impl App {
    /// Gewählte Seiten als Indizes (aufsteigend).
    fn sel_indices(&self) -> Vec<usize> {
        let Some(ed) = &self.ed else { return Vec::new() };
        let Some(sel) = &ed.nav_sel else { return Vec::new() };
        let mut v: Vec<usize> = sel.iter().filter_map(|id| ed.nb.page_index(*id)).collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Leiste unten in der Seitenübersicht (nur im Auswahlmodus).
    pub fn nav_selbar(&self) -> String {
        let n = self.sel_indices().len();
        let dis = if n == 0 { " disabled" } else { "" };
        format!(
            "<div class=\"nav-selbar\"><div class=\"nav-sel-count\"><b>{n}</b> {w} ausgewählt<button class=\"btn small ghost\" data-act=\"nav-sel-all\">{all}</button></div>\
             <div class=\"nav-sel-actions\">\
               <button data-act=\"nav-sel-pdf\" title=\"Als PDF exportieren\"{dis}><svg><use href=\"#i-pdf\"/></svg><span>PDF</span></button>\
               <button data-act=\"nav-sel-dup\" title=\"Duplizieren\"{dis}><svg><use href=\"#i-dup\"/></svg><span>Kopie</span></button>\
               <button data-act=\"nav-sel-rot\" title=\"Drehen\"{dis}><svg><use href=\"#i-rot-cw\"/></svg><span>Drehen</span></button>\
               <button class=\"danger\" data-act=\"nav-sel-del\" title=\"Löschen\"{dis}><svg><use href=\"#i-trash\"/></svg><span>Löschen</span></button>\
             </div></div>",
            w = if n == 1 { "Seite" } else { "Seiten" },
            all = if self.ed.as_ref().is_some_and(|e| n == e.nb.pages.len()) { "Keine" } else { "Alle" }
        )
    }

    pub fn page_sel_action(&mut self, act: &str) {
        match act {
            "nav-sel" => {
                if let Some(ed) = &mut self.ed {
                    ed.nav_sel = if ed.nav_sel.is_some() { None } else { Some(Vec::new()) };
                }
            }
            "nav-sel-all" => {
                if let Some(ed) = &mut self.ed {
                    let all: Vec<u32> = ed.nb.pages.iter().map(|p| p.id).collect();
                    let full = ed.nav_sel.as_ref().is_some_and(|s| s.len() >= all.len());
                    ed.nav_sel = Some(if full { Vec::new() } else { all });
                }
            }
            "nav-sel-pdf" => {
                let idx = self.sel_indices();
                if idx.is_empty() {
                    return;
                }
                if let Some(ed) = &self.ed {
                    let nb = ed.nb.clone();
                    self.export_notebook_pdf(&nb, Some(idx));
                }
                return;
            }
            "nav-sel-dup" => {
                let idx = self.sel_indices();
                let Some(ed) = &mut self.ed else { return };
                let mut group = Vec::new();
                // von hinten, damit die Indizes stimmen; Kopie jeweils direkt hinter dem Original
                for &i in idx.iter().rev() {
                    let p = ed.nb.duplicate_page(i);
                    ed.nb.pages.insert(i + 1, p.clone());
                    group.push(Action::AddPage { index: i + 1, page: p });
                }
                let n = group.len();
                if n > 0 {
                    ed.push(Action::Group(group));
                    ed.nav_sel = Some(Vec::new());
                    self.toast(&format!("{} {} dupliziert", n, if n == 1 { "Seite" } else { "Seiten" }));
                }
                self.after_pages_change();
            }
            "nav-sel-rot" => {
                let idx = self.sel_indices();
                let mut group = Vec::new();
                for &i in &idx {
                    if let Some(a) = self.rotate_page_at(i) {
                        group.push(a);
                    }
                }
                if let Some(ed) = &mut self.ed {
                    if !group.is_empty() {
                        ed.push(Action::Group(group));
                    }
                    ed.update_fit();
                    ed.bmps.clear();
                }
                self.toast(&format!("{} {} gedreht", idx.len(), if idx.len() == 1 { "Seite" } else { "Seiten" }));
                self.after_pages_change();
            }
            "nav-sel-del" => {
                let idx = self.sel_indices();
                let total = self.ed.as_ref().map(|e| e.nb.pages.len()).unwrap_or(0);
                if idx.is_empty() {
                    return;
                }
                if idx.len() >= total {
                    self.toast("Mindestens eine Seite muss übrig bleiben");
                    return;
                }
                let visual = "<div class=\"dlg-icon danger\"><svg><use href=\"#i-trash\"/></svg></div>";
                let text = format!("{} {} gelöscht. Mit „Rückgängig“ lassen sie sich zurückholen.", idx.len(), if idx.len() == 1 { "Seite wird" } else { "Seiten werden" });
                self.open_dialog(visual, "Seiten löschen?", &text, "Löschen", true, None, Pending::DeletePages(idx));
                return;
            }
            _ => {}
        }
        self.render_nav();
    }

    /// Seiten (Indizes) in einem Schritt löschen – rückgängig machbar.
    pub fn delete_pages(&mut self, idx: &[usize]) {
        let Some(ed) = &mut self.ed else { return };
        let mut v: Vec<usize> = idx.iter().copied().filter(|&i| i < ed.nb.pages.len()).collect();
        v.sort_unstable();
        v.dedup();
        if v.is_empty() || v.len() >= ed.nb.pages.len() {
            return;
        }
        let mut group = Vec::new();
        for &i in v.iter().rev() {
            let p = ed.nb.pages.remove(i);
            ed.bmps.remove(&p.id);
            group.push(Action::RemovePage { index: i, page: p });
        }
        ed.selection = None;
        ed.push(Action::Group(group));
        ed.nav_sel = Some(Vec::new());
        ed.update_fit();
        ed.clamp();
        let n = v.len();
        self.toast(&format!("{} {} gelöscht", n, if n == 1 { "Seite" } else { "Seiten" }));
        self.after_pages_change();
        self.render_nav();
    }
}
