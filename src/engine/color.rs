/// sRGB-Farbe mit Kanälen im Bereich 0..1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color {
    pub const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0 };
    pub const WHITE: Color = Color { r: 1.0, g: 1.0, b: 1.0 };

    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Color {
            r: r.clamp(0.0, 1.0),
            g: g.clamp(0.0, 1.0),
            b: b.clamp(0.0, 1.0),
        }
    }

    pub fn from_u8(r: u8, g: u8, b: u8) -> Self {
        Color::rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }

    pub fn to_u8(&self) -> [u8; 3] {
        [
            (self.r * 255.0).round() as u8,
            (self.g * 255.0).round() as u8,
            (self.b * 255.0).round() as u8,
        ]
    }

    pub fn from_hex(s: &str) -> Option<Color> {
        let s = s.trim().trim_start_matches('#');
        let parse = |h: &str| u8::from_str_radix(h, 16).ok();
        match s.len() {
            3 => {
                let r = parse(&s[0..1])?;
                let g = parse(&s[1..2])?;
                let b = parse(&s[2..3])?;
                Some(Color::from_u8(r * 17, g * 17, b * 17))
            }
            6 => Some(Color::from_u8(
                parse(&s[0..2])?,
                parse(&s[2..4])?,
                parse(&s[4..6])?,
            )),
            _ => None,
        }
    }

    pub fn to_hex(&self) -> String {
        let [r, g, b] = self.to_u8();
        format!("#{:02x}{:02x}{:02x}", r, g, b)
    }

    /// `h` in Grad (0..360), `s` und `v` in 0..1.
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Color {
        let h = ((h % 360.0) + 360.0) % 360.0 / 60.0;
        let c = v * s;
        let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
        let m = v - c;
        let (r, g, b) = match h as i32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        Color::rgb(r + m, g + m, b + m)
    }

    pub fn to_hsv(&self) -> (f32, f32, f32) {
        let max = self.r.max(self.g).max(self.b);
        let min = self.r.min(self.g).min(self.b);
        let d = max - min;
        let h = if d < 1e-6 {
            0.0
        } else if max == self.r {
            60.0 * (((self.g - self.b) / d) % 6.0)
        } else if max == self.g {
            60.0 * ((self.b - self.r) / d + 2.0)
        } else {
            60.0 * ((self.r - self.g) / d + 4.0)
        };
        let h = if h < 0.0 { h + 360.0 } else { h };
        let s = if max <= 0.0 { 0.0 } else { d / max };
        (h, s, max)
    }

    /// Wahrgenommene Helligkeit (für Kontrastentscheidungen in der Oberfläche).
    pub fn luminance(&self) -> f32 {
        0.2126 * self.r + 0.7152 * self.g + 0.0722 * self.b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let c = Color::from_hex("#3a7bff").unwrap();
        assert_eq!(c.to_hex(), "#3a7bff");
        assert_eq!(Color::from_hex("fff").unwrap(), Color::WHITE);
        assert!(Color::from_hex("zz").is_none());
    }

    #[test]
    fn hsv_roundtrip() {
        for hex in ["#ff0000", "#00ff00", "#0000ff", "#12abef", "#808080"] {
            let c = Color::from_hex(hex).unwrap();
            let (h, s, v) = c.to_hsv();
            assert_eq!(Color::from_hsv(h, s, v).to_hex(), hex);
        }
    }
}
