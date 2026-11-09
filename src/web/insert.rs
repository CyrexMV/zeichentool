//! Einfügen: Tabellen und Funktionsgraphen (als normale, auswählbare Vektorelemente).

use super::*;
use crate::engine::history::Action;
use crate::engine::plot::{self, PlotSpec};
use crate::engine::{Content, Item, Rect};
use std::fmt::Write as _;

/// Kopfzeilen-Farben der Tabelle (Anzeige, RGBA mit Transparenz).
pub const HEAD_COLORS: [(&str, [u8; 4]); 6] = [
    ("Violett", [109, 92, 255, 46]),
    ("Blau", [47, 140, 255, 46]),
    ("Grün", [31, 191, 117, 46]),
    ("Gelb", [245, 196, 0, 60]),
    ("Rot", [229, 72, 77, 46]),
    ("Grau", [120, 130, 150, 40]),
];

/// Einstellungen im Tabellen-Dialog.
#[derive(Clone, Copy, Debug)]
pub struct TableOpts {
    pub cols: usize,
    pub rows: usize,
    /// Kopfzeile: Index in `HEAD_COLORS`, `None` = ohne
    pub head: Option<usize>,
}

impl Default for TableOpts {
    fn default() -> Self {
        TableOpts { cols: 3, rows: 5, head: Some(0) }
    }
}

fn rgba(c: [u8; 4]) -> String {
    format!("rgba({},{},{},{:.2})", c[0], c[1], c[2], c[3] as f32 / 255.0)
}

/// Elemente als SVG-Vorschau.
fn preview_svg(items: &[Content], w: f32, h: f32) -> String {
    let mut s = format!("<svg viewBox=\"-6 -6 {} {}\" font-family=\"Inter, system-ui, sans-serif\">", w + 12.0, h + 12.0);
    for c in items {
        match c {
            Content::Stroke(st) if st.pen == crate::engine::PenKind::Fill => {
                let pts: String = st.pts.iter().map(|p| format!("{:.1},{:.1} ", p.x, p.y)).collect();
                let _ = write!(s, "<polygon points=\"{}\" fill=\"{}\"/>", pts, rgba(st.color));
            }
            Content::Stroke(st) => {
                let pts: String = st.pts.iter().map(|p| format!("{:.1},{:.1} ", p.x, p.y)).collect();
                let _ = write!(s, "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.1}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>", pts, rgba(st.color), st.width);
            }
            Content::Text(t) => {
                let _ = write!(s, "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"{:.1}\" font-weight=\"{}\" fill=\"{}\">{}</text>", t.x, t.y + t.size * 0.95, t.size, if t.bold { 700 } else { 400 }, rgba(t.color), esc(&t.text));
            }
            _ => {}
        }
    }
    s.push_str("</svg>");
    s
}

impl App {
    pub fn ink_color(&self) -> [u8; 4] {
        if self.dark && self.ed.as_ref().map(|e| e.nb.pages[e.cur_page].paper.luminance() < 0.5).unwrap_or(false) {
            [235, 236, 245, 255]
        } else {
            let p = self.ed.as_ref().map(|e| e.nb.pages[e.cur_page].paper.luminance()).unwrap_or(1.0);
            if p < 0.5 {
                [235, 236, 245, 255]
            } else {
                [30, 32, 44, 255]
            }
        }
    }

    /// Elemente in die Mitte des sichtbaren Bereichs der aktuellen Seite einfügen und auswählen.
    pub fn insert_contents(&mut self, contents: Vec<Content>, w: f32, h: f32) {
        let Some(ed) = &mut self.ed else { return };
        let i = ed.cur_page;
        let (pw, ph) = ed.nb.page_size(i);
        let (cx, cy) = ed.to_nb(ed.vw * 0.5, ed.vh * 0.45);
        let (cx, cy) = ed.nb_to_page(i, cx, cy);
        let (mut x0, mut y0) = (cx as f32 - w * 0.5, cy as f32 - h * 0.5);
        // Liegt dort schon ein gerade Eingefügtes, versetzt daneben (Kaskade statt Stapel)
        for _ in 0..12 {
            let taken = ed.nb.pages[i].items.iter().any(|it| {
                let (mx, my) = it.bbox().center();
                (mx - (x0 + w * 0.5)).abs() < 10.0 && (my - (y0 + h * 0.5)).abs() < 10.0
            });
            if !taken {
                break;
            }
            x0 += 26.0;
            y0 += 26.0;
        }
        let x0 = x0.clamp(0.0, (pw - w).max(0.0));
        let y0 = y0.clamp(0.0, (ph - h).max(0.0));
        let xf = crate::engine::Xf::translate(x0, y0);
        let base = ed.nb.pages[i].max_order();
        let mut items = Vec::new();
        for (k, c) in contents.into_iter().enumerate() {
            let id = ed.nb.alloc_id();
            items.push(Item { id, order: base + 1.0 + k as f64 * 0.001, content: c, pinned: false }.transformed(&xf));
        }
        let page = ed.nb.pages[i].id;
        let ids: Vec<u32> = items.iter().map(|it| it.id).collect();
        let bbox = items.iter().fold(Rect::EMPTY, |r, it| r.union(&it.bbox()));
        ed.nb.pages[i].insert(items.clone());
        ed.push(Action::Items { page, removed: vec![], added: items });
        ed.selection = Some(super::editor::Selection { page, ids, bbox, poly: Vec::new() });
        self.tool = Tool::Lasso;
        self.refresh_toolbar();
        self.after_edit();
    }

    // ---------------- Tabelle ----------------

    /// Heutiges Datum als Textfeld einfügen – lang („Montag, 9. November 2026“) oder kurz („09.11.2026“).
    pub fn insert_date(&mut self, short: bool) {
        if self.ed.is_none() {
            return;
        }
        let d = js_sys::Date::new_0();
        let (y, m, day) = (d.get_full_year(), d.get_month() as usize, d.get_date());
        let text = if short {
            format!("{day:02}.{:02}.{y}", m + 1)
        } else {
            const DAYS: [&str; 7] = ["Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"];
            format!("{}, {day}. {} {y}", DAYS[d.get_day() as usize % 7], crate::engine::cal::MONTHS[m % 12])
        };
        let size = self.prefs.text_size.max(10.0);
        let w = (text.chars().count() as f32 * size * 0.54 + 14.0).min(560.0);
        let h = size * 1.5;
        let color = self.ink_color();
        let tb = crate::engine::TextBox { x: 0.0, y: 0.0, w, size, color, bold: false, text };
        self.insert_contents(vec![Content::Text(tb)], w, h);
    }

    pub fn open_table_dialog(&mut self) {
        self.open_modal("modal-table");
        self.refresh_table_dialog();
    }

    fn table_contents(&self) -> (Vec<Content>, f32, f32) {
        let o = self.table_opts;
        let (pw, ph) = self.ed.as_ref().map(|e| e.nb.page_size(e.cur_page)).unwrap_or((595.0, 842.0));
        let unit = self.ed.as_ref().map(|e| e.nb.pages[e.cur_page].template.unit(pw, ph)).unwrap_or(22.0);
        let cw = (pw * 0.84 / o.cols as f32).clamp(36.0, 160.0);
        let rh = (unit * 1.1).clamp(18.0, 40.0);
        let head = o.head.map(|k| HEAD_COLORS[k % HEAD_COLORS.len()].1);
        let c = plot::table_items(0.0, 0.0, o.cols, o.rows, cw, rh, head, self.ink_color());
        (c, cw * o.cols as f32, rh * o.rows as f32)
    }

    pub fn refresh_table_dialog(&mut self) {
        let o = self.table_opts;
        set_text("tbl-cols", &o.cols.to_string());
        set_text("tbl-rows", &o.rows.to_string());
        let mut sw = format!("<button class=\"tbl-sw none{}\" data-act=\"tbl-head\" data-arg=\"-1\" title=\"Ohne Kopfzeile\"></button>", if o.head.is_none() { " active" } else { "" });
        for (k, (name, c)) in HEAD_COLORS.iter().enumerate() {
            let _ = write!(
                sw,
                "<button class=\"tbl-sw{}\" data-act=\"tbl-head\" data-arg=\"{}\" title=\"{}\" style=\"background:rgb({},{},{})\"></button>",
                if o.head == Some(k) { " active" } else { "" },
                k,
                name,
                c[0],
                c[1],
                c[2]
            );
        }
        el("tbl-heads").set_inner_html(&sw);
        let (c, w, h) = self.table_contents();
        el("tbl-preview").set_inner_html(&preview_svg(&c, w, h));
    }

    pub fn table_action(&mut self, act: &str, arg: &str) {
        let o = &mut self.table_opts;
        match act {
            "tbl-cols" => o.cols = (o.cols as i32 + arg.parse::<i32>().unwrap_or(0)).clamp(1, 12) as usize,
            "tbl-rows" => o.rows = (o.rows as i32 + arg.parse::<i32>().unwrap_or(0)).clamp(1, 40) as usize,
            "tbl-head" => o.head = arg.parse::<i32>().ok().filter(|v| *v >= 0).map(|v| v as usize),
            "tbl-insert" => {
                let (c, w, h) = self.table_contents();
                self.close_modals();
                self.insert_contents(c, w, h);
                self.toast("Tabelle eingefügt – schreib mit dem Stift in die Zellen; mit dem Lasso verschieben oder skalieren");
                return;
            }
            _ => {}
        }
        self.refresh_table_dialog();
    }

    // ---------------- Funktionsgraph ----------------

    pub fn open_plot_dialog(&mut self) {
        self.open_modal("modal-plot");
        if input_el("plot-f").value().trim().is_empty() {
            input_el("plot-f").set_value("x^2 - 2");
        }
        self.refresh_plot();
        let _ = input_el("plot-f").focus();
    }

    /// Eingaben lesen; Fehlertext bei ungültigen Formeln.
    fn plot_spec(&self) -> Result<PlotSpec, String> {
        let src = input_el("plot-f").value();
        let mut funcs = Vec::new();
        for part in src.split(';').map(|s| s.trim()).filter(|s| !s.is_empty()).take(5) {
            funcs.push(plot::parse(part).map_err(|e| format!("{} (in „{}“)", e, part))?);
        }
        if funcs.is_empty() {
            return Err("Bitte eine Funktion eingeben, z. B. x^2 - 2".into());
        }
        let num = |id: &str, d: f64| input_el(id).value().replace(',', ".").trim().parse::<f64>().unwrap_or(d);
        let (xmin, xmax) = (num("plot-x0", -5.0), num("plot-x1", 5.0));
        if xmax <= xmin {
            return Err("„x bis“ muss größer sein als „x von“".into());
        }
        let y = if input_el("plot-yauto").checked() {
            None
        } else {
            let (a, b) = (num("plot-y0", -5.0), num("plot-y1", 5.0));
            if b <= a {
                return Err("„y bis“ muss größer sein als „y von“".into());
            }
            Some((a, b))
        };
        Ok(PlotSpec { funcs, xmin, xmax, y, grid: input_el("plot-grid").checked() })
    }

    fn plot_size(&self) -> (f32, f32) {
        let pw = self.ed.as_ref().map(|e| e.nb.page_size(e.cur_page).0).unwrap_or(595.0);
        let w = (pw * 0.72).min(460.0);
        (w, w * 0.68)
    }

    pub fn refresh_plot(&mut self) {
        set_class(&el("plot-yrow"), "section-off", input_el("plot-yauto").checked());
        match self.plot_spec() {
            Ok(spec) => {
                let (w, h) = self.plot_size();
                let c = plot::plot_items(&spec, 0.0, 0.0, w, h, if self.dark { [230, 232, 245, 255] } else { [30, 32, 44, 255] });
                el("plot-preview").set_inner_html(&preview_svg(&c, w, h));
                set_text("plot-err", "");
                set_class(&el("plot-insert"), "disabled", false);
            }
            Err(e) => {
                set_text("plot-err", &e);
                set_class(&el("plot-insert"), "disabled", true);
            }
        }
    }

    pub fn insert_plot(&mut self) {
        let spec = match self.plot_spec() {
            Ok(s) => s,
            Err(e) => {
                self.toast(&e);
                return;
            }
        };
        let (w, h) = self.plot_size();
        let c = plot::plot_items(&spec, 0.0, 0.0, w, h, self.ink_color());
        self.close_modals();
        self.insert_contents(c, w, h);
        self.toast("Funktionsgraph eingefügt – mit dem Lasso verschieben oder skalieren");
    }
}
