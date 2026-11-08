use super::mask::Mask;
use super::noise::Noise;
use super::rect::IRect;

/// Beschreibung eines Pinsels. Alle Werte sind normalisiert (0..1), sofern nicht anders angegeben.
#[derive(Clone, Debug, PartialEq)]
pub struct BrushPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    /// Kantenhärte: 1 = harte, geglättete Kante; 0 = sehr weich.
    pub hardness: f32,
    /// Abstand der Stempel relativ zum Durchmesser.
    pub spacing: f32,
    /// Deckkraft pro Stempel.
    pub flow: f32,
    /// true = Farbe baut sich innerhalb eines Strichs auf (Airbrush, Bleistift).
    pub buildup: bool,
    /// Einfluss des Stiftdrucks auf die Größe.
    pub pressure_size: f32,
    /// Einfluss des Stiftdrucks auf die Deckkraft.
    pub pressure_opacity: f32,
    /// Minimale Größe bei sehr leichtem Druck (relativ).
    pub min_size: f32,
    /// Papierkörnung.
    pub grain: f32,
    /// Seitenverhältnis der Pinselspitze (1 = rund).
    pub aspect: f32,
    /// Fester Winkel der Spitze in Radiant.
    pub angle: f32,
    /// Neigung des S-Pens erzeugt breitere, weichere Schattierung.
    pub tilt_shading: f32,
    /// Zufällige Streuung relativ zum Radius.
    pub jitter: f32,
    /// Pixel-Pinsel ohne Kantenglättung.
    pub pixel: bool,
    pub default_size: f32,
    pub default_opacity: f32,
}

impl BrushPreset {
    const BASE: BrushPreset = BrushPreset {
        id: "",
        name: "",
        group: "",
        hardness: 1.0,
        spacing: 0.08,
        flow: 1.0,
        buildup: false,
        pressure_size: 0.5,
        pressure_opacity: 0.0,
        min_size: 0.1,
        grain: 0.0,
        aspect: 1.0,
        angle: 0.0,
        tilt_shading: 0.0,
        jitter: 0.0,
        pixel: false,
        default_size: 10.0,
        default_opacity: 1.0,
    };
}

/// Alle eingebauten Pinsel.
pub fn presets() -> Vec<BrushPreset> {
    let b = BrushPreset::BASE;
    vec![
        BrushPreset { id: "fineliner", name: "Fineliner", group: "Tinte", pressure_size: 0.3, min_size: 0.5, default_size: 6.0, ..b },
        BrushPreset { id: "tusche", name: "Tusche", group: "Tinte", hardness: 0.96, spacing: 0.05, pressure_size: 1.0, min_size: 0.04, default_size: 16.0, ..b },
        BrushPreset { id: "kalligraphie", name: "Kalligraphie", group: "Tinte", hardness: 0.98, spacing: 0.04, aspect: 0.16, angle: 0.785, pressure_size: 0.6, min_size: 0.3, default_size: 28.0, ..b },
        BrushPreset { id: "bleistift", name: "Bleistift", group: "Skizze", hardness: 0.55, spacing: 0.14, flow: 0.4, buildup: true, pressure_size: 0.25, pressure_opacity: 0.9, min_size: 0.6, grain: 0.85, tilt_shading: 1.0, default_size: 5.0, default_opacity: 0.9, ..b },
        BrushPreset { id: "kohle", name: "Kohle", group: "Skizze", hardness: 0.4, spacing: 0.1, flow: 0.28, buildup: true, pressure_size: 0.45, pressure_opacity: 0.8, min_size: 0.4, grain: 0.75, aspect: 0.7, angle: 0.6, tilt_shading: 0.8, jitter: 0.06, default_size: 30.0, ..b },
        BrushPreset { id: "pinsel", name: "Rundpinsel", group: "Malen", hardness: 0.78, spacing: 0.04, pressure_size: 0.85, pressure_opacity: 0.25, min_size: 0.12, default_size: 26.0, ..b },
        BrushPreset { id: "flachpinsel", name: "Flachpinsel", group: "Malen", hardness: 0.75, spacing: 0.04, flow: 0.55, buildup: true, aspect: 0.3, angle: 0.785, pressure_size: 0.4, pressure_opacity: 0.5, min_size: 0.4, grain: 0.25, default_size: 44.0, ..b },
        BrushPreset { id: "aquarell", name: "Aquarell", group: "Malen", hardness: 0.3, spacing: 0.07, flow: 0.12, buildup: true, pressure_size: 0.6, pressure_opacity: 0.6, min_size: 0.3, grain: 0.45, jitter: 0.05, default_size: 60.0, default_opacity: 0.75, ..b },
        BrushPreset { id: "weich", name: "Weicher Pinsel", group: "Malen", hardness: 0.15, spacing: 0.06, flow: 0.25, buildup: true, pressure_size: 0.5, pressure_opacity: 0.8, min_size: 0.3, default_size: 70.0, ..b },
        BrushPreset { id: "airbrush", name: "Airbrush", group: "Malen", hardness: 0.0, spacing: 0.04, flow: 0.06, buildup: true, pressure_size: 0.2, pressure_opacity: 1.0, min_size: 0.6, default_size: 140.0, ..b },
        BrushPreset { id: "marker", name: "Marker", group: "Marker", hardness: 0.9, spacing: 0.04, pressure_size: 0.1, min_size: 0.8, aspect: 0.85, default_size: 24.0, default_opacity: 0.6, ..b },
        BrushPreset { id: "textmarker", name: "Textmarker", group: "Marker", hardness: 0.95, spacing: 0.04, pressure_size: 0.0, aspect: 0.3, angle: 0.0, default_size: 32.0, default_opacity: 0.4, ..b },
        BrushPreset { id: "pixel", name: "Pixel", group: "Spezial", hardness: 1.0, spacing: 0.3, pressure_size: 0.0, pixel: true, default_size: 1.0, ..b },
        BrushPreset { id: "spray", name: "Sprühdose", group: "Spezial", hardness: 0.7, spacing: 0.2, flow: 0.7, buildup: true, pressure_size: 0.1, pressure_opacity: 0.6, min_size: 0.15, jitter: 1.4, grain: 0.2, default_size: 40.0, ..b },
    ]
}

/// Ein einzelner Pinselstempel.
#[derive(Clone, Copy, Debug)]
pub struct Dab {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub alpha: f32,
    pub hardness: f32,
    pub aspect: f32,
    pub angle: f32,
    pub grain: f32,
    pub pixel: bool,
    /// Index der Symmetrie-Kopie.
    pub sym: u8,
}

impl Dab {
    pub fn bounds(&self) -> IRect {
        IRect::around(self.x, self.y, self.radius.max(0.5) + 1.0)
    }
}

/// Liefert die Deckung eines Stempels an Pixel `(px, py)` (0..1) – ohne Körnung und Fluss.
#[inline]
pub fn dab_coverage(d: &Dab, px: i32, py: i32, cos: f32, sin: f32, inv_aspect: f32) -> f32 {
    let r = d.radius.max(0.5);
    let dx = px as f32 + 0.5 - d.x;
    let dy = py as f32 + 0.5 - d.y;
    let u = dx * cos + dy * sin;
    let v = (-dx * sin + dy * cos) * inv_aspect;
    let f2 = u * u + v * v;
    if d.pixel {
        return if f2 <= (r + 0.01) * (r + 0.01) { 1.0 } else { 0.0 };
    }
    // Außerhalb der Ellipse plus Glättungsrand → keine Deckung.
    let lim = r + inv_aspect;
    if f2 >= lim * lim {
        return 0.0;
    }
    let f = f2.sqrt();
    // Abstand zur Kante in echten Pixeln (Gradientennäherung, wichtig für schmale Spitzen)
    let g = if f > 1e-4 {
        (u * u + v * v * inv_aspect * inv_aspect).sqrt() / f
    } else {
        1.0
    };
    let edge = ((r - f) / g + 0.5).clamp(0.0, 1.0);
    if edge <= 0.0 {
        return 0.0;
    }
    if d.hardness >= 0.99 {
        return edge;
    }
    let t = ((f / r - d.hardness) / (1.0 - d.hardness)).clamp(0.0, 1.0);
    (1.0 - t * t * (3.0 - 2.0 * t)) * edge
}

/// Stempelt `d` in die Maske.
pub fn render_dab(mask: &mut Mask, d: &Dab, buildup: bool, noise: &Noise) {
    let mut d = *d;
    if d.pixel {
        d.x = d.x.floor() + 0.5;
        d.y = d.y.floor() + 0.5;
    }
    let rect = d.bounds().intersect(&mask.bounds());
    if rect.is_empty() {
        return;
    }
    let alpha_scale = if d.radius < 0.5 && !d.pixel {
        (d.radius / 0.5).powi(2).max(0.05)
    } else {
        1.0
    };
    let (sin, cos) = d.angle.sin_cos();
    let inv_aspect = 1.0 / d.aspect.clamp(0.05, 1.0);
    let base = d.alpha * alpha_scale;
    let w = mask.w;
    for py in rect.y0..rect.y1 {
        let row = py as usize * w;
        for px in rect.x0..rect.x1 {
            let mut a = dab_coverage(&d, px, py, cos, sin, inv_aspect);
            if a <= 0.0 {
                continue;
            }
            if d.grain > 0.0 {
                let n = noise.at(px, py);
                // Körnung: helle Stellen der Papierstruktur nehmen weniger Farbe an
                a *= ((1.0 - d.grain) + d.grain * (n * 1.6 - 0.3)).clamp(0.0, 1.0);
            }
            a *= base;
            if a <= 0.0 {
                continue;
            }
            let i = row + px as usize;
            let m = mask.data[i] as f32 * (1.0 / 65535.0);
            let n = if buildup { m + a * (1.0 - m) } else { m.max(a) };
            mask.data[i] = (n.min(1.0) * 65535.0 + 0.5) as u16;
        }
    }
    mask.dirty = mask.dirty.union(&rect);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dab_fills_center_and_respects_bounds() {
        let mut m = Mask::new(32, 32);
        let noise = Noise::new();
        let d = Dab { x: 16.0, y: 16.0, radius: 5.0, alpha: 1.0, hardness: 1.0, aspect: 1.0, angle: 0.0, grain: 0.0, pixel: false, sym: 0 };
        render_dab(&mut m, &d, false, &noise);
        assert!(m.get(16, 16) > 0.99);
        assert_eq!(m.get(0, 0), 0.0);
        assert!(m.get(16, 23) == 0.0);
        assert!(!m.dirty.is_empty());
    }

    #[test]
    fn presets_have_unique_ids() {
        let p = presets();
        let mut ids: Vec<_> = p.iter().map(|p| p.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), p.len());
    }
}
