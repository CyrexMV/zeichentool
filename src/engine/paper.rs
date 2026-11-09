//! Papiervorlagen (liniert, kariert, gepunktet, Cornell, Notenlinien, Planer) als Vektor-Grundformen.
//! Die Linien gehören zum Papier und lassen sich nicht wegradieren.

use super::color::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateKind {
    Blank = 0,
    Lined = 1,
    Grid = 2,
    Dots = 3,
    Cornell = 4,
    Music = 5,
    Planner = 6,
    Checklist = 7,
}

impl TemplateKind {
    pub const ALL: [TemplateKind; 8] = [
        TemplateKind::Blank,
        TemplateKind::Lined,
        TemplateKind::Grid,
        TemplateKind::Dots,
        TemplateKind::Cornell,
        TemplateKind::Music,
        TemplateKind::Planner,
        TemplateKind::Checklist,
    ];

    pub fn code(&self) -> &'static str {
        match self {
            TemplateKind::Blank => "blank",
            TemplateKind::Lined => "lined",
            TemplateKind::Grid => "grid",
            TemplateKind::Dots => "dots",
            TemplateKind::Cornell => "cornell",
            TemplateKind::Music => "music",
            TemplateKind::Planner => "planner",
            TemplateKind::Checklist => "checklist",
        }
    }

    pub fn from_code(s: &str) -> TemplateKind {
        TemplateKind::ALL.iter().copied().find(|k| k.code() == s).unwrap_or(TemplateKind::Blank)
    }

    pub fn from_u8(v: u8) -> TemplateKind {
        TemplateKind::ALL.get(v as usize).copied().unwrap_or(TemplateKind::Blank)
    }

    pub fn name_de(&self) -> &'static str {
        match self {
            TemplateKind::Blank => "Blanko",
            TemplateKind::Lined => "Liniert",
            TemplateKind::Grid => "Kariert",
            TemplateKind::Dots => "Gepunktet",
            TemplateKind::Cornell => "Cornell",
            TemplateKind::Music => "Notenlinien",
            TemplateKind::Planner => "Wochenplaner",
            TemplateKind::Checklist => "Checkliste",
        }
    }
}

/// Vorlage + Linienabstand (0 = eng, 1 = mittel, 2 = weit).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Template {
    pub kind: TemplateKind,
    pub spacing: u8,
}

impl Default for Template {
    fn default() -> Self {
        Template { kind: TemplateKind::Blank, spacing: 1 }
    }
}

/// Grundform einer Vorlage (Seitenkoordinaten in pt).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Prim {
    /// Linie; `accent` = farbige Randlinie, `strong` = kräftige Strukturlinie
    Line { x0: f32, y0: f32, x1: f32, y1: f32, accent: bool, strong: bool },
    Dot { x: f32, y: f32, r: f32 },
    /// Kästchen (Checkliste), linke obere Ecke + Kantenlänge
    Box { x: f32, y: f32, size: f32 },
}

/// Eine Zeile der Checkliste: Zeilenband (y0..y1), Kästchen, Beginn des Schreibbereichs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CheckRow {
    pub y0: f32,
    pub y1: f32,
    pub bx: f32,
    pub by: f32,
    pub size: f32,
    pub text_x: f32,
}

impl CheckRow {
    pub fn box_contains(&self, x: f32, y: f32, pad: f32) -> bool {
        x >= self.bx - pad && x <= self.bx + self.size + pad && y >= self.by - pad && y <= self.by + self.size + pad
    }
}

/// Linienfarben passend zur Papierhelligkeit: (Linie, Akzent) als RGBA 0..1.
pub fn line_colors(paper: Color) -> ([f32; 4], [f32; 4]) {
    if paper.luminance() > 0.5 {
        ([0.47, 0.6, 0.78, 0.55], [0.92, 0.42, 0.42, 0.7])
    } else {
        ([1.0, 1.0, 1.0, 0.2], [0.95, 0.45, 0.45, 0.55])
    }
}

impl Template {
    /// Grundabstand in pt (mittel ≈ 8 mm auf A4).
    pub fn unit(&self, w: f32, h: f32) -> f32 {
        let base = w.max(h);
        base / match self.spacing {
            0 => 48.0,
            2 => 28.0,
            _ => 37.0,
        }
    }

    pub fn line_width(&self, w: f32, h: f32) -> f32 {
        (self.unit(w, h) / 40.0).max(0.35)
    }

    /// Zeilen der Checkliste (leer für andere Vorlagen).
    pub fn check_rows(&self, w: f32, h: f32) -> Vec<CheckRow> {
        if self.kind != TemplateKind::Checklist {
            return Vec::new();
        }
        let s = self.unit(w, h);
        let size = s * 0.52;
        let bx = s * 0.9;
        let mut rows = Vec::new();
        let mut y0 = s * 2.5;
        while y0 + s <= h - s * 0.5 {
            let y1 = y0 + s;
            rows.push(CheckRow { y0, y1, bx, by: y0 + (s - size) * 0.5 + s * 0.08, size, text_x: bx + size + s * 0.45 });
            y0 = y1;
        }
        rows
    }

    pub fn primitives(&self, w: f32, h: f32) -> Vec<Prim> {
        let s = self.unit(w, h);
        let mut v = Vec::new();
        let hline = |v: &mut Vec<Prim>, y: f32, x0: f32, x1: f32, strong: bool| v.push(Prim::Line { x0, y0: y, x1, y1: y, accent: false, strong });
        match self.kind {
            TemplateKind::Blank => {}
            TemplateKind::Lined => {
                let mut y = s * 2.5;
                while y < h - s * 0.5 {
                    hline(&mut v, y, 0.0, w, false);
                    y += s;
                }
                if w > s * 12.0 {
                    v.push(Prim::Line { x0: s * 2.2, y0: 0.0, x1: s * 2.2, y1: h, accent: true, strong: false });
                }
            }
            TemplateKind::Grid => {
                let g = s * 0.62;
                let ox = (w % g) * 0.5;
                let oy = (h % g) * 0.5;
                let mut x = ox;
                while x <= w {
                    v.push(Prim::Line { x0: x, y0: 0.0, x1: x, y1: h, accent: false, strong: false });
                    x += g;
                }
                let mut y = oy;
                while y <= h {
                    hline(&mut v, y, 0.0, w, false);
                    y += g;
                }
            }
            TemplateKind::Dots => {
                let g = s * 0.62;
                let ox = (w % g) * 0.5;
                let oy = (h % g) * 0.5;
                let r = (g / 18.0).max(0.5);
                let mut y = oy;
                while y <= h {
                    let mut x = ox;
                    while x <= w {
                        v.push(Prim::Dot { x, y, r });
                        x += g;
                    }
                    y += g;
                }
            }
            TemplateKind::Cornell => {
                let top = s * 3.0;
                let bottom = h - s * 6.0;
                let cue = w * 0.28;
                hline(&mut v, top, 0.0, w, true);
                hline(&mut v, bottom, 0.0, w, true);
                let mut y = top + s;
                while y < bottom - s * 0.3 {
                    hline(&mut v, y, cue, w, false);
                    y += s;
                }
                v.push(Prim::Line { x0: cue, y0: top, x1: cue, y1: bottom, accent: false, strong: true });
            }
            TemplateKind::Music => {
                let gap = s * 0.28;
                let mut top = s * 2.5;
                let margin = s * 1.5;
                while top + gap * 4.0 < h - s {
                    for k in 0..5 {
                        hline(&mut v, top + gap * k as f32, margin, w - margin, false);
                    }
                    top += s * 2.6;
                }
            }
            TemplateKind::Checklist => {
                let rows = self.check_rows(w, h);
                if let Some(first) = rows.first() {
                    hline(&mut v, first.y0, 0.0, w, false);
                }
                for r in &rows {
                    hline(&mut v, r.y1, 0.0, w, false);
                    v.push(Prim::Box { x: r.bx, y: r.by, size: r.size });
                }
            }
            TemplateKind::Planner => {
                // 2 Spalten × 4 Felder (Montag–Sonntag + Notizen)
                let m = s * 1.2;
                let head = s * 2.6;
                let (cw, ch) = ((w - m * 2.0) / 2.0, (h - head - m) / 4.0);
                for row in 0..=4 {
                    let y = head + ch * row as f32;
                    hline(&mut v, y, m, w - m, true);
                }
                for col in 0..=2 {
                    let x = m + cw * col as f32;
                    v.push(Prim::Line { x0: x, y0: head, x1: x, y1: h - m, accent: false, strong: true });
                }
                for row in 0..4 {
                    let y0 = head + ch * row as f32;
                    let mut y = y0 + s * 1.6;
                    while y < y0 + ch - s * 0.4 {
                        hline(&mut v, y, m, w - m, false);
                        y += s * 0.85;
                    }
                }
            }
        }
        v
    }
}

/// Wochentage für den Planer (Feld-Beschriftungen).
pub const PLANNER_LABELS: [&str; 8] = ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag", "Notizen"];

/// Position der Planer-Beschriftungen (x, y) in pt.
pub fn planner_label_pos(t: &Template, w: f32, h: f32) -> Vec<(f32, f32, &'static str)> {
    let s = t.unit(w, h);
    let m = s * 1.2;
    let head = s * 2.6;
    let (cw, ch) = ((w - m * 2.0) / 2.0, (h - head - m) / 4.0);
    let mut out = Vec::new();
    for (i, l) in PLANNER_LABELS.iter().enumerate() {
        let (col, row) = (i / 4, i % 4);
        out.push((m + cw * col as f32 + s * 0.4, head + ch * row as f32 + s * 1.1, *l));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_produce_primitives() {
        for k in TemplateKind::ALL {
            let t = Template { kind: k, spacing: 1 };
            let p = t.primitives(595.0, 842.0);
            if k == TemplateKind::Blank {
                assert!(p.is_empty());
            } else {
                assert!(p.len() > 5, "{:?}", k);
            }
            assert_eq!(TemplateKind::from_code(k.code()), k);
            assert_eq!(TemplateKind::from_u8(k as u8), k);
        }
        let c = Template { kind: TemplateKind::Checklist, spacing: 1 };
        let rows = c.check_rows(595.0, 842.0);
        assert!(rows.len() > 20);
        assert!(rows[0].box_contains(rows[0].bx + 1.0, rows[0].by + 1.0, 0.0));
        assert!(c.primitives(595.0, 842.0).iter().any(|p| matches!(p, Prim::Box { .. })));
        // mittel ≈ 8 mm
        let u = Template::default().unit(595.0, 842.0);
        assert!((u - 22.75).abs() < 0.1);
    }
}
