//! Stift-Sets: Lieblingskombinationen aus Werkzeug, Stiftart, Farbe und Stärke.

use super::*;
use crate::engine::PenKind;

const MAX_SETS: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenSet {
    /// Stift oder Textmarker
    pub highlighter: bool,
    pub kind: PenKind,
    pub color: [u8; 4],
    pub size: f32,
}

impl PenSet {
    pub fn encode(&self) -> String {
        format!("{}:{}:{:02x}{:02x}{:02x}{:02x}:{}", if self.highlighter { "hl" } else { "pen" }, self.kind.code(), self.color[0], self.color[1], self.color[2], self.color[3], self.size)
    }

    pub fn decode(s: &str) -> Option<PenSet> {
        let f: Vec<&str> = s.split(':').collect();
        if f.len() != 4 || f[2].len() != 8 {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&f[2][i * 2..i * 2 + 2], 16).ok();
        Some(PenSet { highlighter: f[0] == "hl", kind: PenKind::from_code(f[1]), color: [byte(0)?, byte(1)?, byte(2)?, byte(3)?], size: f[3].parse().ok()? })
    }

    fn label(&self) -> String {
        let name = if self.highlighter { "Textmarker" } else { self.kind.name_de() };
        format!("{} · {}", name, format!("{:.1}", self.size).replace('.', ","))
    }

    /// kleine Strichprobe als SVG
    fn preview(&self) -> String {
        let c = self.color;
        let w = if self.highlighter { (self.size * 0.8).clamp(6.0, 14.0) } else { (self.size * 1.4).clamp(1.5, 7.0) };
        format!(
            "<svg viewBox=\"0 0 64 24\"><path d=\"M4 16 C 14 4, 22 4, 30 12 S 46 22, 60 8\" fill=\"none\" stroke=\"rgba({},{},{},{:.2})\" stroke-width=\"{:.1}\" stroke-linecap=\"{}\"/></svg>",
            c[0],
            c[1],
            c[2],
            if self.highlighter { 0.55 } else { c[3] as f32 / 255.0 },
            w,
            if self.highlighter { "butt" } else { "round" }
        )
    }
}

impl App {
    pub fn open_pen_sets(&mut self, anchor: &web_sys::Element) {
        self.render_pen_sets();
        self.open_popover("pop-sets", anchor);
    }

    fn render_pen_sets(&mut self) {
        let mut h = String::from("<div class=\"pop-title\">Stift-Sets</div>");
        if self.pen_sets.is_empty() {
            h.push_str("<p class=\"hint\">Noch keine Sets. Stell Stiftart, Farbe und Stärke ein und speichere die Kombination hier.</p>");
        }
        h.push_str("<div class=\"set-list\">");
        for (i, s) in self.pen_sets.iter().enumerate() {
            h.push_str(&format!(
                "<div class=\"set-item\" data-act=\"set-apply\" data-arg=\"{i}\"><span class=\"set-prev\">{}</span><span class=\"set-label\">{}</span><button class=\"set-del\" data-act=\"set-del\" data-arg=\"{i}\" title=\"Set löschen\"><svg><use href=\"#i-close\"/></svg></button></div>",
                s.preview(),
                s.label(),
                i = i
            ));
        }
        h.push_str("</div><button class=\"btn small primary set-save\" data-act=\"set-save\"><svg><use href=\"#i-plus\"/></svg>Aktuellen Stift speichern</button>");
        el("pop-sets").set_inner_html(&h);
    }

    pub fn pen_set_action(&mut self, act: &str, arg: &str) {
        let i = arg.parse::<usize>().unwrap_or(usize::MAX);
        match act {
            "set-save" => {
                let p = &self.prefs;
                let set = match self.effective_tool() {
                    Tool::Highlighter => PenSet { highlighter: true, kind: PenKind::Highlighter, color: p.hl_colors[p.hl_ci], size: p.hl_sizes[p.hl_si] },
                    Tool::Pen | Tool::Shape => PenSet { highlighter: false, kind: p.pen_kind, color: p.pen_colors[p.pen_ci], size: p.pen_sizes[p.pen_si] },
                    _ => {
                        self.toast("Wähle zuerst den Stift oder den Textmarker");
                        return;
                    }
                };
                if self.pen_sets.contains(&set) {
                    self.toast("Dieses Set gibt es schon");
                    return;
                }
                self.pen_sets.push(set);
                if self.pen_sets.len() > MAX_SETS {
                    self.pen_sets.remove(0);
                }
                self.toast("Stift-Set gespeichert");
            }
            "set-del" => {
                if i < self.pen_sets.len() {
                    self.pen_sets.remove(i);
                }
            }
            "set-apply" => {
                let Some(s) = self.pen_sets.get(i).copied() else { return };
                let p = &mut self.prefs;
                if s.highlighter {
                    p.hl_colors[p.hl_ci] = s.color;
                    p.hl_sizes[p.hl_si] = s.size;
                    self.set_tool(Tool::Highlighter);
                } else {
                    p.pen_kind = s.kind;
                    p.pen_colors[p.pen_ci] = s.color;
                    p.pen_sizes[p.pen_si] = s.size;
                    self.set_tool(Tool::Pen);
                }
                self.close_popovers();
                self.refresh_favs();
                self.save_settings();
                self.toast(&format!("{} gewählt", s.label()));
                return;
            }
            _ => return,
        }
        self.save_settings();
        self.render_pen_sets();
    }
}
