//! Papiervorlagen (wie in Notiz-Apps): liniert, kariert, gepunktet, Cornell, Notenlinien.
//! Die Linien gehören zum Papier – sie liegen unter allen Ebenen und lassen sich nicht wegradieren.

use super::color::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateKind {
    Blank = 0,
    Lined = 1,
    Grid = 2,
    Dots = 3,
    Cornell = 4,
    Music = 5,
}

impl TemplateKind {
    pub const ALL: [TemplateKind; 6] = [
        TemplateKind::Blank,
        TemplateKind::Lined,
        TemplateKind::Grid,
        TemplateKind::Dots,
        TemplateKind::Cornell,
        TemplateKind::Music,
    ];

    pub fn code(&self) -> &'static str {
        match self {
            TemplateKind::Blank => "blank",
            TemplateKind::Lined => "lined",
            TemplateKind::Grid => "grid",
            TemplateKind::Dots => "dots",
            TemplateKind::Cornell => "cornell",
            TemplateKind::Music => "music",
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

/// Vorberechnete Werte, damit die Abfrage pro Pixel billig bleibt.
pub struct TemplateCtx {
    kind: TemplateKind,
    w: f32,
    h: f32,
    s: f32,
    half: f32,
    line: [f32; 4],
    accent: [f32; 4],
}

#[inline]
fn line_cov(d: f32, half: f32) -> f32 {
    (half + 0.5 - d).clamp(0.0, 1.0)
}

/// Abstand zur nächsten Linie einer periodischen Linienschar.
#[inline]
fn periodic(p: f32, off: f32, period: f32) -> f32 {
    let m = (p - off).rem_euclid(period);
    m.min(period - m)
}

impl Template {
    /// Grundabstand in Pixeln, relativ zur Seitengröße (A4 bei 300 dpi → ca. 8 mm „mittel“).
    pub fn unit(&self, w: usize, h: usize) -> f32 {
        let base = w.max(h) as f32;
        let div = match self.spacing {
            0 => 48.0,
            2 => 28.0,
            _ => 37.0,
        };
        (base / div).max(8.0)
    }

    pub fn ctx(&self, w: usize, h: usize, paper: Color) -> Option<TemplateCtx> {
        if self.kind == TemplateKind::Blank {
            return None;
        }
        let s = self.unit(w, h);
        let light = paper.luminance() > 0.5;
        let (line, accent) = if light {
            ([0.52, 0.64, 0.80, 0.55], [0.92, 0.42, 0.42, 0.65])
        } else {
            ([1.0, 1.0, 1.0, 0.17], [0.95, 0.45, 0.45, 0.5])
        };
        Some(TemplateCtx { kind: self.kind, w: w as f32, h: h as f32, s, half: (s / 55.0).max(0.7), line, accent })
    }
}

impl TemplateCtx {
    /// Papierfarbe an Pixel (x, y) inklusive Vorlagenlinien (vormultipliziert, deckend).
    #[inline]
    pub fn shade(&self, x: usize, y: usize, paper: [f32; 4]) -> [f32; 4] {
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        let s = self.s;
        let (mut cl, mut ca) = (0.0f32, 0.0f32);
        match self.kind {
            TemplateKind::Blank => {}
            TemplateKind::Lined => {
                let top = s * 2.5;
                if py > top - s * 0.5 {
                    cl = line_cov(periodic(py, top, s), self.half);
                }
                if self.w > s * 12.0 {
                    ca = line_cov((px - s * 2.2).abs(), self.half);
                }
            }
            TemplateKind::Grid => {
                let g = s * 0.62;
                let ox = (self.w % g) * 0.5;
                let oy = (self.h % g) * 0.5;
                let h = self.half * 0.9;
                cl = line_cov(periodic(px, ox, g), h).max(line_cov(periodic(py, oy, g), h));
            }
            TemplateKind::Dots => {
                let g = s * 0.62;
                let ox = (self.w % g) * 0.5;
                let oy = (self.h % g) * 0.5;
                let dx = periodic(px, ox, g);
                let dy = periodic(py, oy, g);
                let r = (g / 16.0).max(1.3);
                cl = (r + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0) * 1.4;
            }
            TemplateKind::Cornell => {
                let top = s * 3.0;
                let bottom = self.h - s * 6.0;
                let cue = self.w * 0.28;
                let strong = self.half * 1.6;
                cl = line_cov((py - top).abs(), strong).max(line_cov((py - bottom).abs(), strong));
                if py > top && py < bottom {
                    cl = cl.max(line_cov(periodic(py, top, s), self.half));
                    cl = cl.max(line_cov((px - cue).abs(), strong));
                }
            }
            TemplateKind::Music => {
                let gap = s * 0.28;
                let period = s * 2.6;
                let top = s * 2.5;
                if py > top - gap {
                    let local = (py - top).rem_euclid(period);
                    let staff = gap * 4.0;
                    let d = if local <= staff + gap * 0.5 {
                        let k = (local / gap).round().clamp(0.0, 4.0);
                        (local - k * gap).abs()
                    } else {
                        period - local
                    };
                    cl = line_cov(d, self.half * 0.9);
                }
            }
        }
        let mut c = paper;
        for (cov, col) in [(cl, self.line), (ca, self.accent)] {
            let a = (cov * col[3]).min(1.0);
            if a > 0.0 {
                for i in 0..3 {
                    c[i] = c[i] * (1.0 - a) + col[i] * a;
                }
            }
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lined_has_lines_and_gaps() {
        let t = Template { kind: TemplateKind::Lined, spacing: 1 };
        let ctx = t.ctx(2480, 3508, Color::WHITE).unwrap();
        let s = t.unit(2480, 3508);
        let white = [1.0, 1.0, 1.0, 1.0];
        // Pixel genau auf einer Linie vs. zwischen zwei Linien
        let on = ctx.shade(1000, (s * 5.5) as usize, white);
        let off = ctx.shade(1000, (s * 6.0) as usize, white);
        assert!(on[0] < 0.9, "Linie fehlt: {:?}", on);
        assert_eq!(off, white);
    }

    #[test]
    fn blank_has_no_ctx() {
        assert!(Template::default().ctx(100, 100, Color::WHITE).is_none());
        for k in TemplateKind::ALL {
            assert_eq!(TemplateKind::from_code(k.code()), k);
            assert_eq!(TemplateKind::from_u8(k as u8), k);
        }
    }
}
