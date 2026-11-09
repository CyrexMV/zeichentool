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
    WeekColor = 8,
    Shopping = 9,
    Daily = 10,
    Month = 11,
    Timetable = 12,
    Meals = 13,
    Habits = 14,
    Flashcard = 15,
}

impl TemplateKind {
    pub const ALL: [TemplateKind; 16] = [
        TemplateKind::Blank,
        TemplateKind::Lined,
        TemplateKind::Grid,
        TemplateKind::Dots,
        TemplateKind::Cornell,
        TemplateKind::Music,
        TemplateKind::Planner,
        TemplateKind::Checklist,
        TemplateKind::WeekColor,
        TemplateKind::Shopping,
        TemplateKind::Daily,
        TemplateKind::Month,
        TemplateKind::Timetable,
        TemplateKind::Meals,
        TemplateKind::Habits,
        TemplateKind::Flashcard,
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
            TemplateKind::WeekColor => "weekcolor",
            TemplateKind::Shopping => "shopping",
            TemplateKind::Daily => "daily",
            TemplateKind::Month => "month",
            TemplateKind::Timetable => "timetable",
            TemplateKind::Meals => "meals",
            TemplateKind::Habits => "habits",
            TemplateKind::Flashcard => "flashcard",
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
            TemplateKind::WeekColor => "Woche farbig",
            TemplateKind::Shopping => "Einkaufsliste",
            TemplateKind::Daily => "Tagesplaner",
            TemplateKind::Month => "Monatskalender",
            TemplateKind::Timetable => "Stundenplan",
            TemplateKind::Meals => "Essensplan",
            TemplateKind::Habits => "Gewohnheiten",
            TemplateKind::Flashcard => "Lernkarte",
        }
    }

    /// Farbige Planer-Vorlagen (Vorschau als SVG statt CSS).
    pub fn is_colored(&self) -> bool {
        (*self as u8) >= 8
    }

    /// Vorlagen mit abhakbaren Kästchen.
    pub fn has_checks(&self) -> bool {
        matches!(self, TemplateKind::Checklist | TemplateKind::Shopping | TemplateKind::Daily | TemplateKind::Timetable | TemplateKind::Meals)
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

/// Rechteck (x0, y0, x1, y1).
pub type Area = (f32, f32, f32, f32);

/// Farbe als RGBA (0…255).
pub type Rgba = [u8; 4];

/// Grundform einer Vorlage (Seitenkoordinaten in pt).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Prim {
    /// Linie; `accent` = farbige Randlinie, `strong` = kräftige Strukturlinie
    Line { x0: f32, y0: f32, x1: f32, y1: f32, accent: bool, strong: bool },
    Dot { x: f32, y: f32, r: f32 },
    /// Kästchen (Checkliste), linke obere Ecke + Kantenlänge
    Box { x: f32, y: f32, size: f32 },
    /// Farbfläche (abgerundetes Rechteck mit Eckradius `r`)
    Fill { x: f32, y: f32, w: f32, h: f32, r: f32, color: Rgba },
    /// Beschriftung ab Grundlinie (x, y); `color: None` = Linienfarbe des Papiers
    Label { x: f32, y: f32, size: f32, text: &'static str, color: Option<Rgba>, bold: bool },
}

/// Eine abhakbare Zeile: Zeilenband (y0..y1), Kästchen, Schreibbereich text_x..x1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CheckRow {
    pub y0: f32,
    pub y1: f32,
    pub bx: f32,
    pub by: f32,
    pub size: f32,
    pub text_x: f32,
    pub x1: f32,
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

// Farbpalette der Planer
pub const VIOLET: Rgba = [109, 92, 255, 255];
pub const SKY: Rgba = [36, 160, 237, 255];
pub const GREEN: Rgba = [31, 178, 110, 255];
pub const AMBER: Rgba = [240, 152, 25, 255];
pub const CORAL: Rgba = [240, 88, 88, 255];
pub const PINK: Rgba = [214, 72, 186, 255];
pub const TEAL: Rgba = [0, 168, 155, 255];
pub const SLATE: Rgba = [118, 130, 155, 255];
const WHITE: Rgba = [255, 255, 255, 255];
/// Eine Farbe je Wochentag (Mo…So) + Notizen
pub const DAY_COLORS: [Rgba; 8] = [VIOLET, SKY, GREEN, AMBER, CORAL, PINK, TEAL, SLATE];

/// Wochentage für die Planer.
pub const PLANNER_LABELS: [&str; 8] = ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag", "Notizen"];
const DAYS_SHORT: [&str; 7] = ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"];
const HOURS: [&str; 17] = ["06:00", "07:00", "08:00", "09:00", "10:00", "11:00", "12:00", "13:00", "14:00", "15:00", "16:00", "17:00", "18:00", "19:00", "20:00", "21:00", "22:00"];
const NUMS: [&str; 31] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16", "17", "18", "19", "20", "21", "22", "23", "24", "25", "26", "27", "28", "29", "30", "31"];
const SHOP: [(&str, Rgba); 8] = [
    ("Obst & Gemüse", GREEN),
    ("Brot & Backwaren", AMBER),
    ("Milch & Käse", SKY),
    ("Fleisch & Fisch", CORAL),
    ("Vorrat & Konserven", VIOLET),
    ("Getränke", TEAL),
    ("Drogerie & Haushalt", PINK),
    ("Sonstiges", SLATE),
];
const MEALS: [(&str, Rgba); 4] = [("Frühstück", AMBER), ("Mittagessen", CORAL), ("Abendessen", VIOLET), ("Snacks", TEAL)];

fn tint(c: Rgba, a: u8) -> Rgba {
    [c[0], c[1], c[2], a]
}

/// Grobe Textbreite (Helvetica/Inter-ähnlich) – für Ausrichtungen in Vorlagen.
pub fn approx_text_w(text: &str, size: f32, bold: bool) -> f32 {
    text.chars().count() as f32 * size * if bold { 0.58 } else { 0.53 }
}

/// Sammelt Grundformen und abhakbare Zeilen einer Vorlage.
struct Builder {
    v: Vec<Prim>,
    rows: Vec<CheckRow>,
    s: f32,
}

impl Builder {
    fn hline(&mut self, y: f32, x0: f32, x1: f32, strong: bool) {
        self.v.push(Prim::Line { x0, y0: y, x1, y1: y, accent: false, strong });
    }

    fn vline(&mut self, x: f32, y0: f32, y1: f32, strong: bool) {
        self.v.push(Prim::Line { x0: x, y0, x1: x, y1, accent: false, strong });
    }

    fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, color: Rgba) {
        self.v.push(Prim::Fill { x, y, w, h, r, color });
    }

    fn label(&mut self, x: f32, y: f32, size: f32, text: &'static str, color: Option<Rgba>, bold: bool) {
        self.v.push(Prim::Label { x, y, size, text, color, bold });
    }

    /// Überschrift oben links + optional Feld rechts („KW ____“).
    fn title(&mut self, w: f32, text: &'static str, color: Rgba, field: Option<&'static str>) {
        let s = self.s;
        let m = s;
        self.label(m, s * 1.95, s * 0.95, text, Some(color), true);
        if let Some(f) = field {
            let fw = (w * 0.3).max(s * 5.0);
            let x = w - m - fw;
            self.label(x, s * 1.95, s * 0.5, f, None, true);
            let lx = x + approx_text_w(f, s * 0.5, true) + s * 0.6;
            self.hline(s * 2.05, lx, w - m, false);
        }
    }

    /// Farbige Kopfzeile (Pille) mit weißer Schrift.
    fn pill(&mut self, x: f32, y: f32, w: f32, text: &'static str, color: Rgba) {
        let s = self.s;
        let h = s * 1.1;
        self.fill(x, y, w, h, s * 0.4, tint(color, 235));
        let size = (s * 0.5).min(w / (text.chars().count().max(1) as f32 * 0.62));
        self.label(x + s * 0.42, y + h * 0.5 + size * 0.36, size, text, Some(WHITE), true);
    }

    /// Abschnitt: getönte Fläche mit Kopfzeile. Gibt den Beginn des Inhalts zurück.
    fn section(&mut self, x: f32, y: f32, w: f32, h: f32, text: &'static str, color: Rgba) -> f32 {
        let s = self.s;
        self.fill(x, y, w, h, s * 0.5, tint(color, 24));
        self.pill(x + s * 0.25, y + s * 0.25, w - s * 0.5, text, color);
        y + s * 1.6
    }

    /// Linierte Fläche.
    fn lines(&mut self, x0: f32, x1: f32, mut y: f32, bottom: f32, step: f32) {
        while y < bottom {
            self.hline(y, x0, x1, false);
            y += step;
        }
    }

    /// Abhakbare Zeilen von y bis bottom im Bereich x0..x1.
    fn check_rows(&mut self, x0: f32, x1: f32, mut y: f32, bottom: f32, max: usize) {
        let s = self.s;
        let rh = s * 0.95;
        let size = s * 0.46;
        let mut k = 0;
        while y + rh <= bottom + 0.01 && k < max {
            let bx = x0 + s * 0.35;
            let by = y + (rh - size) * 0.5 + s * 0.05;
            let text_x = bx + size + s * 0.35;
            self.rows.push(CheckRow { y0: y, y1: y + rh, bx, by, size, text_x, x1 });
            self.v.push(Prim::Box { x: bx, y: by, size });
            self.hline(y + rh, text_x - s * 0.1, x1, false);
            y += rh;
            k += 1;
        }
    }

    /// Raster aus `cols` × `rows` Zellen in einem Bereich.
    fn grid(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, cols: &[f32], rows: usize) {
        let rh = (y1 - y0) / rows as f32;
        for r in 0..=rows {
            self.hline(y0 + rh * r as f32, x0, x1, r == 0 || r == rows);
        }
        self.vline(x0, y0, y1, true);
        for &x in cols {
            self.vline(x, y0, y1, false);
        }
        self.vline(x1, y0, y1, true);
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

    /// Abhakbare Zeilen (leer für Vorlagen ohne Kästchen).
    pub fn check_rows(&self, w: f32, h: f32) -> Vec<CheckRow> {
        if !self.kind.has_checks() {
            return Vec::new();
        }
        self.build(w, h).rows
    }

    pub fn primitives(&self, w: f32, h: f32) -> Vec<Prim> {
        self.build(w, h).v
    }

    /// Lernkarten-Vorlage: (Frage-Bereich, Antwort-Bereich) als (x0, y0, x1, y1).
    pub fn flashcard_halves(w: f32, h: f32) -> (Area, Area) {
        if w > h * 1.15 {
            ((0.0, 0.0, w * 0.5, h), (w * 0.5, 0.0, w, h))
        } else {
            ((0.0, 0.0, w, h * 0.5), (0.0, h * 0.5, w, h))
        }
    }

    fn build(&self, w: f32, h: f32) -> Builder {
        let s = self.unit(w, h);
        let mut b = Builder { v: Vec::new(), rows: Vec::new(), s };
        let landscape = w > h * 1.1;
        let m = s;
        let top = s * 2.8;
        let g = s * 0.4;
        match self.kind {
            TemplateKind::Blank => {}
            TemplateKind::Lined => {
                b.lines(0.0, w, s * 2.5, h - s * 0.5, s);
                if w > s * 12.0 {
                    b.v.push(Prim::Line { x0: s * 2.2, y0: 0.0, x1: s * 2.2, y1: h, accent: true, strong: false });
                }
            }
            TemplateKind::Grid => {
                let gs = s * 0.62;
                let (ox, oy) = ((w % gs) * 0.5, (h % gs) * 0.5);
                let mut x = ox;
                while x <= w {
                    b.vline(x, 0.0, h, false);
                    x += gs;
                }
                let mut y = oy;
                while y <= h {
                    b.hline(y, 0.0, w, false);
                    y += gs;
                }
            }
            TemplateKind::Dots => {
                let gs = s * 0.62;
                let (ox, oy) = ((w % gs) * 0.5, (h % gs) * 0.5);
                let r = (gs / 18.0).max(0.5);
                let mut y = oy;
                while y <= h {
                    let mut x = ox;
                    while x <= w {
                        b.v.push(Prim::Dot { x, y, r });
                        x += gs;
                    }
                    y += gs;
                }
            }
            TemplateKind::Cornell => {
                let top = s * 3.0;
                let bottom = h - s * 6.0;
                let cue = w * 0.28;
                b.hline(top, 0.0, w, true);
                b.hline(bottom, 0.0, w, true);
                b.lines(cue, w, top + s, bottom - s * 0.3, s);
                b.vline(cue, top, bottom, true);
            }
            TemplateKind::Music => {
                let gap = s * 0.28;
                let mut top = s * 2.5;
                let margin = s * 1.5;
                while top + gap * 4.0 < h - s {
                    for k in 0..5 {
                        b.hline(top + gap * k as f32, margin, w - margin, false);
                    }
                    top += s * 2.6;
                }
            }
            TemplateKind::Checklist => {
                let size = s * 0.52;
                let bx = s * 0.9;
                let mut y0 = s * 2.5;
                b.hline(y0, 0.0, w, false);
                while y0 + s <= h - s * 0.5 {
                    let y1 = y0 + s;
                    let by = y0 + (s - size) * 0.5 + s * 0.08;
                    b.rows.push(CheckRow { y0, y1, bx, by, size, text_x: bx + size + s * 0.45, x1: w });
                    b.hline(y1, 0.0, w, false);
                    b.v.push(Prim::Box { x: bx, y: by, size });
                    y0 = y1;
                }
            }
            TemplateKind::Planner => {
                // 2 Spalten × 4 Felder (Montag–Sonntag + Notizen)
                let m = s * 1.2;
                let head = s * 2.6;
                let (cw, ch) = ((w - m * 2.0) / 2.0, (h - head - m) / 4.0);
                for row in 0..=4 {
                    b.hline(head + ch * row as f32, m, w - m, true);
                }
                for col in 0..=2 {
                    b.vline(m + cw * col as f32, head, h - m, true);
                }
                for row in 0..4 {
                    let y0 = head + ch * row as f32;
                    b.lines(m, w - m, y0 + s * 1.6, y0 + ch - s * 0.4, s * 0.85);
                }
                for (i, l) in PLANNER_LABELS.iter().enumerate() {
                    let (col, row) = (i / 4, i % 4);
                    b.label(m + cw * col as f32 + s * 0.4, head + ch * row as f32 + s * 1.1, s * 0.55, l, None, true);
                }
            }
            TemplateKind::WeekColor => {
                b.title(w, "Meine Woche", VIOLET, Some("KW"));
                let (cols, rows) = if landscape { (4, 2) } else { (2, 4) };
                let cw = (w - m * 2.0 - g * (cols - 1) as f32) / cols as f32;
                let ch = (h - top - m - g * (rows - 1) as f32) / rows as f32;
                for (i, l) in PLANNER_LABELS.iter().enumerate() {
                    let (col, row) = (i % cols, i / cols);
                    let (x, y) = (m + (cw + g) * col as f32, top + (ch + g) * row as f32);
                    let c0 = b.section(x, y, cw, ch, l, DAY_COLORS[i]);
                    b.lines(x + s * 0.4, x + cw - s * 0.4, c0 + s * 0.6, y + ch - s * 0.3, s * 0.8);
                }
            }
            TemplateKind::Shopping => {
                b.title(w, "Einkaufsliste", CORAL, Some("Datum"));
                let (cols, rows) = if landscape { (4, 2) } else { (2, 4) };
                let cw = (w - m * 2.0 - g * (cols - 1) as f32) / cols as f32;
                let ch = (h - top - m - g * (rows - 1) as f32) / rows as f32;
                for (i, (l, c)) in SHOP.iter().enumerate() {
                    let (col, row) = (i % cols, i / cols);
                    let (x, y) = (m + (cw + g) * col as f32, top + (ch + g) * row as f32);
                    let c0 = b.section(x, y, cw, ch, l, *c);
                    b.check_rows(x + s * 0.1, x + cw - s * 0.35, c0, y + ch - s * 0.25, 40);
                }
            }
            TemplateKind::Daily => {
                b.title(w, "Mein Tag", AMBER, Some("Datum"));
                let lw = (w - m * 2.0 - g) * 0.55;
                let (rx, rw) = (m + lw + g, w - m * 2.0 - g - lw);
                let bottom = h - m;
                // Zeitplan
                let c0 = b.section(m, top, lw, bottom - top, "Zeitplan", SKY);
                let slot = (bottom - s * 0.3 - c0) / HOURS.len() as f32;
                let tx = m + s * 2.6;
                for (i, l) in HOURS.iter().enumerate() {
                    let y = c0 + slot * i as f32;
                    b.label(m + s * 0.45, y + slot * 0.5 + s * 0.15, s * 0.42, l, None, true);
                    b.hline(y + slot, m + s * 0.3, m + lw - s * 0.3, false);
                }
                b.vline(tx, c0, c0 + slot * HOURS.len() as f32, false);
                // Rechte Spalte: Top 3, To-do, Notizen, Wasser
                let rh = s * 0.95;
                let top3_h = s * 1.6 + rh * 3.0 + s * 0.35;
                let c1 = b.section(rx, top, rw, top3_h, "Top 3", CORAL);
                b.check_rows(rx + s * 0.1, rx + rw - s * 0.35, c1, c1 + rh * 3.0, 3);
                let water_h = s * 2.9;
                let rest = bottom - (top + top3_h + g) - water_h - g * 2.0;
                let todo_h = rest * 0.58;
                let ty = top + top3_h + g;
                let c2 = b.section(rx, ty, rw, todo_h, "To-do", VIOLET);
                b.check_rows(rx + s * 0.1, rx + rw - s * 0.35, c2, ty + todo_h - s * 0.25, 30);
                let ny = ty + todo_h + g;
                let notes_h = rest - todo_h;
                let c3 = b.section(rx, ny, rw, notes_h, "Notizen", TEAL);
                b.lines(rx + s * 0.4, rx + rw - s * 0.4, c3 + s * 0.6, ny + notes_h - s * 0.3, s * 0.8);
                let wy = ny + notes_h + g;
                let c4 = b.section(rx, wy, rw, water_h, "Wasser", SKY);
                let bs = s * 0.62;
                let gap = ((rw - s * 0.8) - bs * 8.0) / 7.0;
                for k in 0..8 {
                    b.v.push(Prim::Box { x: rx + s * 0.4 + (bs + gap.max(s * 0.2)) * k as f32, y: c4 + (wy + water_h - c4 - bs) * 0.5 - s * 0.05, size: bs });
                }
            }
            TemplateKind::Month => {
                b.title(w, "Monat", VIOLET, Some("Jahr"));
                // Monatsname schreiben: Linie hinter der Überschrift
                b.hline(s * 2.05, m + approx_text_w("Monat", s * 0.95, true) + s * 0.4, w * 0.5, false);
                let notes_h = if landscape { 0.0 } else { (h - top) * 0.2 };
                let gy0 = top + s * 1.35;
                let gy1 = h - m - if notes_h > 0.0 { notes_h + g } else { 0.0 };
                let cw = (w - m * 2.0) / 7.0;
                for d in 0..7 {
                    let x = m + cw * d as f32;
                    b.pill(x + s * 0.08, top, cw - s * 0.16, DAYS_SHORT[d], DAY_COLORS[d]);
                    b.fill(x, gy0, cw, gy1 - gy0, 0.0, tint(DAY_COLORS[d], if d >= 5 { 30 } else { 12 }));
                }
                let cols: Vec<f32> = (1..7).map(|d| m + cw * d as f32).collect();
                b.grid(m, gy0, w - m, gy1, &cols, 6);
                if notes_h > 0.0 {
                    let ny = gy1 + g;
                    let c = b.section(m, ny, w - m * 2.0, h - m - ny, "Notizen", SLATE);
                    b.lines(m + s * 0.4, w - m - s * 0.4, c + s * 0.6, h - m - s * 0.3, s * 0.8);
                }
            }
            TemplateKind::Timetable => {
                b.title(w, "Stundenplan", SKY, Some("Klasse"));
                let (gx1, gy1) = if landscape { (m + (w - m * 2.0) * 0.7, h - m) } else { (w - m, top + (h - top - m) * 0.68) };
                let tw = s * 1.6;
                let cw = (gx1 - m - tw) / 5.0;
                for d in 0..5 {
                    b.pill(m + tw + cw * d as f32 + s * 0.08, top, cw - s * 0.16, PLANNER_LABELS[d], DAY_COLORS[d]);
                }
                let gy0 = top + s * 1.35;
                let lessons = 10;
                let rh = (gy1 - gy0) / lessons as f32;
                for (r, l) in NUMS.iter().enumerate().take(lessons) {
                    let y = gy0 + rh * r as f32;
                    if r % 2 == 1 {
                        b.fill(m, y, gx1 - m, rh, 0.0, tint(SLATE, 14));
                    }
                    b.label(m + tw * 0.5 - approx_text_w(l, s * 0.5, true) * 0.5, y + rh * 0.5 + s * 0.18, s * 0.5, l, None, true);
                }
                let cols: Vec<f32> = (0..5).map(|d| m + tw + cw * d as f32).collect();
                b.grid(m, gy0, gx1, gy1, &cols, lessons);
                let (hx, hy) = if landscape { (gx1 + g, top) } else { (m, gy1 + g) };
                let (hw, hh) = (w - m - hx, h - m - hy);
                let c = b.section(hx, hy, hw, hh, "Hausaufgaben", CORAL);
                b.check_rows(hx + s * 0.1, hx + hw - s * 0.35, c, hy + hh - s * 0.25, 40);
            }
            TemplateKind::Meals => {
                b.title(w, "Essensplan", GREEN, Some("Woche"));
                let (gx1, gy1) = if landscape { (m + (w - m * 2.0) * 0.72, h - m) } else { (w - m, top + (h - top - m) * 0.66) };
                let dw = s * 2.6;
                let cw = (gx1 - m - dw) / MEALS.len() as f32;
                for (k, (l, c)) in MEALS.iter().enumerate() {
                    b.pill(m + dw + cw * k as f32 + s * 0.08, top, cw - s * 0.16, l, *c);
                }
                let gy0 = top + s * 1.35;
                let rh = (gy1 - gy0) / 7.0;
                for d in 0..7 {
                    let y = gy0 + rh * d as f32;
                    b.fill(m, y, gx1 - m, rh, 0.0, tint(DAY_COLORS[d], 16));
                    b.fill(m + s * 0.15, y + s * 0.15, dw - s * 0.3, rh - s * 0.3, s * 0.35, tint(DAY_COLORS[d], 225));
                    b.label(m + s * 0.45, y + rh * 0.5 + s * 0.18, s * 0.5, DAYS_SHORT[d], Some(WHITE), true);
                }
                let cols: Vec<f32> = (0..MEALS.len()).map(|k| m + dw + cw * k as f32).collect();
                b.grid(m, gy0, gx1, gy1, &cols, 7);
                let (hx, hy) = if landscape { (gx1 + g, top) } else { (m, gy1 + g) };
                let (hw, hh) = (w - m - hx, h - m - hy);
                let c = b.section(hx, hy, hw, hh, "Einkaufen", CORAL);
                if landscape {
                    b.check_rows(hx + s * 0.1, hx + hw - s * 0.35, c, hy + hh - s * 0.25, 40);
                } else {
                    let half = hw * 0.5;
                    b.check_rows(hx + s * 0.1, hx + half - s * 0.2, c, hy + hh - s * 0.25, 40);
                    b.check_rows(hx + half, hx + hw - s * 0.35, c, hy + hh - s * 0.25, 40);
                }
            }
            TemplateKind::Habits => {
                b.title(w, "Gewohnheiten", TEAL, Some("Monat"));
                let nw = if landscape { s * 6.5 } else { s * 5.0 };
                let x0 = m + nw;
                let dw = (w - m - x0) / 31.0;
                let hy = top + s * 0.9;
                let size = (s * 0.36).min(dw * 0.7);
                for (d, l) in NUMS.iter().enumerate() {
                    let x = x0 + dw * d as f32 + dw * 0.5 - approx_text_w(l, size, true) * 0.5;
                    b.label(x, top + s * 0.55, size, l, None, true);
                }
                let rh = s * 1.15;
                let rows = (((h - m - hy) / rh).floor() as usize).min(24);
                for r in 0..rows {
                    let y = hy + rh * r as f32;
                    let c = DAY_COLORS[r % 8];
                    b.fill(m, y + s * 0.08, nw - s * 0.2, rh - s * 0.16, s * 0.3, tint(c, 40));
                    if r % 2 == 0 {
                        b.fill(x0, y, w - m - x0, rh, 0.0, tint(c, 12));
                    }
                }
                let y1 = hy + rh * rows as f32;
                let cols: Vec<f32> = (1..31).map(|d| x0 + dw * d as f32).collect();
                for r in 0..=rows {
                    b.hline(hy + rh * r as f32, x0, w - m, r == 0 || r == rows);
                }
                b.vline(x0, hy, y1, true);
                for x in cols {
                    b.vline(x, hy, y1, false);
                }
                b.vline(w - m, hy, y1, true);
            }
            TemplateKind::Flashcard => {
                let ((ax0, ay0, ax1, ay1), (bx0, by0, bx1, by1)) = Template::flashcard_halves(w, h);
                b.fill(ax0, ay0, ax1 - ax0, ay1 - ay0, 0.0, tint(VIOLET, 14));
                b.fill(bx0, by0, bx1 - bx0, by1 - by0, 0.0, tint(GREEN, 14));
                let pw = s * 3.4;
                b.pill(ax0 + s * 0.6, ay0 + s * 0.6, pw, "Frage", VIOLET);
                b.pill(bx0 + s * 0.6, by0 + s * 0.6, pw * 1.15, "Antwort", GREEN);
                if bx0 > 0.0 {
                    b.vline(bx0, 0.0, h, true);
                } else {
                    b.hline(by0, 0.0, w, true);
                }
            }
        }
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_produce_primitives() {
        for k in TemplateKind::ALL {
            for (w, h) in [(595.0, 842.0), (842.0, 595.0), (700.0, 700.0), (420.0, 595.0)] {
                let t = Template { kind: k, spacing: 1 };
                let p = t.primitives(w, h);
                if k == TemplateKind::Blank {
                    assert!(p.is_empty());
                } else {
                    assert!(p.len() > 5, "{:?}", k);
                }
                // alles liegt auf der Seite
                for pr in &p {
                    if let Prim::Fill { x, y, w: fw, h: fh, .. } = pr {
                        assert!(*x >= -0.5 && *y >= -0.5 && x + fw <= w + 0.5 && y + fh <= h + 0.5, "{:?} {:?}", k, pr);
                    }
                }
                assert_eq!(t.kind.has_checks(), !t.check_rows(w, h).is_empty(), "{:?} {}x{}", k, w, h);
            }
            assert_eq!(TemplateKind::from_code(k.code()), k);
            assert_eq!(TemplateKind::from_u8(k as u8), k);
        }
        let c = Template { kind: TemplateKind::Checklist, spacing: 1 };
        let rows = c.check_rows(595.0, 842.0);
        assert!(rows.len() > 20);
        assert!(rows[0].box_contains(rows[0].bx + 1.0, rows[0].by + 1.0, 0.0));
        assert!(c.primitives(595.0, 842.0).iter().any(|p| matches!(p, Prim::Box { .. })));
        let shop = Template { kind: TemplateKind::Shopping, spacing: 1 }.check_rows(595.0, 842.0);
        assert!(shop.len() >= 24, "{}", shop.len());
        let week = Template { kind: TemplateKind::WeekColor, spacing: 1 }.primitives(595.0, 842.0);
        assert_eq!(week.iter().filter(|p| matches!(p, Prim::Label { text, .. } if PLANNER_LABELS.contains(text))).count(), 8);
        // mittel ≈ 8 mm
        let u = Template::default().unit(595.0, 842.0);
        assert!((u - 22.75).abs() < 0.1);
    }
}
