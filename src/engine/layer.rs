/// Mischmodus einer Ebene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    Normal = 0,
    Multiply = 1,
    Screen = 2,
    Overlay = 3,
    Darken = 4,
    Lighten = 5,
    Add = 6,
    ColorDodge = 7,
    Difference = 8,
}

impl BlendMode {
    pub const ALL: [BlendMode; 9] = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::Add,
        BlendMode::ColorDodge,
        BlendMode::Difference,
    ];

    pub fn name_de(&self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Multiply => "Multiplizieren",
            BlendMode::Screen => "Negativ multiplizieren",
            BlendMode::Overlay => "Überlagern",
            BlendMode::Darken => "Abdunkeln",
            BlendMode::Lighten => "Aufhellen",
            BlendMode::Add => "Addieren (Leuchten)",
            BlendMode::ColorDodge => "Farbig abwedeln",
            BlendMode::Difference => "Differenz",
        }
    }

    pub fn from_u8(v: u8) -> BlendMode {
        BlendMode::ALL
            .get(v as usize)
            .copied()
            .unwrap_or(BlendMode::Normal)
    }

    #[inline]
    fn channel(&self, cs: f32, cd: f32) -> f32 {
        match self {
            BlendMode::Normal => cs,
            BlendMode::Multiply => cs * cd,
            BlendMode::Screen => cs + cd - cs * cd,
            BlendMode::Overlay => {
                if cd <= 0.5 {
                    2.0 * cs * cd
                } else {
                    1.0 - 2.0 * (1.0 - cs) * (1.0 - cd)
                }
            }
            BlendMode::Darken => cs.min(cd),
            BlendMode::Lighten => cs.max(cd),
            BlendMode::Add => (cs + cd).min(1.0),
            BlendMode::ColorDodge => {
                if cd <= 0.0 {
                    0.0
                } else if cs >= 1.0 {
                    1.0
                } else {
                    (cd / (1.0 - cs)).min(1.0)
                }
            }
            BlendMode::Difference => (cs - cd).abs(),
        }
    }

    /// Mischt die vormultiplizierte Quellfarbe `s` auf das vormultiplizierte Ziel `d`.
    #[inline]
    pub fn blend(&self, s: [f32; 4], d: [f32; 4]) -> [f32; 4] {
        let sa = s[3];
        if sa <= 0.0 {
            return d;
        }
        let da = d[3];
        if *self == BlendMode::Normal || da <= 0.0 {
            let k = 1.0 - sa;
            return [s[0] + d[0] * k, s[1] + d[1] * k, s[2] + d[2] * k, sa + da * k];
        }
        let ra = sa + da - sa * da;
        let mut out = [0.0; 4];
        for i in 0..3 {
            let cs = s[i] / sa;
            let cd = d[i] / da;
            out[i] = s[i] * (1.0 - da) + d[i] * (1.0 - sa) + sa * da * self.channel(cs, cd);
        }
        out[3] = ra;
        out
    }
}

/// Eigenschaften einer Ebene (ohne Pixeldaten) – wird auch im Verlauf gespeichert.
#[derive(Clone, Debug, PartialEq)]
pub struct LayerProps {
    pub name: String,
    pub opacity: f32,
    pub visible: bool,
    pub blend: BlendMode,
    pub locked: bool,
    pub alpha_lock: bool,
}

impl LayerProps {
    pub fn named(name: &str) -> Self {
        LayerProps {
            name: name.to_string(),
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
            locked: false,
            alpha_lock: false,
        }
    }
}

/// Eine Ebene mit vormultiplizierten RGBA8-Pixeln.
#[derive(Clone, Debug)]
pub struct Layer {
    pub id: u32,
    pub props: LayerProps,
    pub data: Vec<u8>,
    /// Wird bei jeder Pixeländerung erhöht (für inkrementelles Autospeichern).
    pub rev: u64,
}

impl Layer {
    pub fn new(id: u32, name: &str, w: usize, h: usize) -> Self {
        Layer {
            id,
            props: LayerProps::named(name),
            data: vec![0; w * h * 4],
            rev: 0,
        }
    }

    pub fn touch(&mut self) {
        self.rev = self.rev.wrapping_add(1);
    }

    pub fn is_empty(&self) -> bool {
        self.data.chunks_exact(4).all(|p| p[3] == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_blend_over() {
        let s = [0.5, 0.0, 0.0, 0.5];
        let d = [0.0, 0.0, 1.0, 1.0];
        let r = BlendMode::Normal.blend(s, d);
        assert!((r[0] - 0.5).abs() < 1e-6);
        assert!((r[2] - 0.5).abs() < 1e-6);
        assert!((r[3] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn multiply_white_is_identity() {
        let s = [1.0, 1.0, 1.0, 1.0];
        let d = [0.2, 0.4, 0.6, 1.0];
        let r = BlendMode::Multiply.blend(s, d);
        for i in 0..4 {
            assert!((r[i] - d[i]).abs() < 1e-5);
        }
    }
}
