//! Buchcover: Farbverläufe und gemusterte Designs (Leder, Leinen, Marmor …) als Vektor-Grundformen.
//! Dieselben Formen zeichnen die Bibliothek, das Deckblatt im Notizbuch und der PDF-Export.

use super::paper::{Prim, Rgba};

/// Muster eines Covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    Gradient,
    Leather,
    Linen,
    Kraft,
    Marble,
    Dots,
    Stripes,
    Plaid,
    Stars,
    Waves,
    Terrazzo,
    Rainbow,
    Sunset,
}

/// Muster je Cover-Index (passend zu `note::COVERS`).
pub const PATTERNS: [Pattern; 23] = [
    Pattern::Gradient,
    Pattern::Gradient,
    Pattern::Gradient,
    Pattern::Gradient,
    Pattern::Gradient,
    Pattern::Gradient,
    Pattern::Gradient,
    Pattern::Gradient,
    Pattern::Gradient,
    Pattern::Gradient,
    Pattern::Leather,
    Pattern::Leather,
    Pattern::Linen,
    Pattern::Kraft,
    Pattern::Marble,
    Pattern::Dots,
    Pattern::Stripes,
    Pattern::Plaid,
    Pattern::Stars,
    Pattern::Waves,
    Pattern::Terrazzo,
    Pattern::Rainbow,
    Pattern::Sunset,
];

pub fn pattern(idx: usize) -> Pattern {
    PATTERNS.get(idx).copied().unwrap_or(Pattern::Gradient)
}

/// "#rrggbb" → RGBA.
pub fn hex(s: &str) -> Rgba {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0x777777);
    [(v >> 16) as u8, (v >> 8) as u8, v as u8, 255]
}

fn a(c: Rgba, alpha: u8) -> Rgba {
    [c[0], c[1], c[2], alpha]
}

fn lerp(c1: Rgba, c2: Rgba, t: f32) -> Rgba {
    let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    [m(c1[0], c2[0]), m(c1[1], c2[1]), m(c1[2], c2[2]), 255]
}

/// Kleiner, fester Zufallsgenerator (damit Muster immer gleich aussehen).
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 % 100_000) as f32 / 100_000.0
    }
}

/// Grundformen des Covers `idx` für die Fläche w × h (ohne Titel).
pub fn cover_prims(idx: usize, w: f32, h: f32) -> Vec<Prim> {
    let (_, c1, c2) = super::note::COVERS[idx % super::note::COVERS.len()];
    let (c1, c2) = (hex(c1), hex(c2));
    let s = w.min(h) / 20.0;
    let mut v = Vec::new();
    let mut rng = Rng(0x9e37_79b9 ^ (idx as u32 * 7919 + 1));
    let full_grad = |v: &mut Vec<Prim>| v.push(Prim::Grad { x: 0.0, y: 0.0, w, h, c1, c2 });
    let seg = |v: &mut Vec<Prim>, x0: f32, y0: f32, x1: f32, y1: f32, lw: f32, color: Rgba| v.push(Prim::Seg { x0, y0, x1, y1, w: lw, color, dash: false });
    match pattern(idx) {
        Pattern::Gradient => full_grad(&mut v),
        Pattern::Leather => {
            full_grad(&mut v);
            // Narbung
            for _ in 0..(w * h / (s * s) * 3.0) as usize {
                let (x, y, r) = (rng.next() * w, rng.next() * h, s * (0.04 + rng.next() * 0.1));
                let dark = rng.next() > 0.5;
                v.push(Prim::Circle { x, y, r, color: if dark { [0, 0, 0, 26] } else { [255, 255, 255, 16] } });
            }
            // Ziernaht
            let m = s * 0.7;
            let stitch = [255, 230, 200, 150];
            for (x0, y0, x1, y1) in [(m, m, w - m, m), (w - m, m, w - m, h - m), (w - m, h - m, m, h - m), (m, h - m, m, m)] {
                v.push(Prim::Seg { x0, y0, x1, y1, w: s * 0.08, color: stitch, dash: true });
            }
        }
        Pattern::Linen => {
            full_grad(&mut v);
            let step = s * 0.22;
            let mut x = 0.0;
            while x < w {
                seg(&mut v, x, 0.0, x, h, step * 0.35, if (x / step) as i32 % 3 == 0 { [255, 255, 255, 22] } else { [0, 0, 0, 18] });
                x += step;
            }
            let mut y = 0.0;
            while y < h {
                seg(&mut v, 0.0, y, w, y, step * 0.35, if (y / step) as i32 % 3 == 0 { [255, 255, 255, 18] } else { [0, 0, 0, 16] });
                y += step;
            }
        }
        Pattern::Kraft => {
            full_grad(&mut v);
            for _ in 0..(w * h / (s * s) * 4.0) as usize {
                let (x, y, r) = (rng.next() * w, rng.next() * h, s * (0.02 + rng.next() * 0.06));
                v.push(Prim::Circle { x, y, r, color: if rng.next() > 0.3 { [90, 60, 25, 60] } else { [255, 245, 220, 70] } });
            }
            // Gummiband
            v.push(Prim::Fill { x: w * 0.82, y: 0.0, w: s * 0.55, h, r: 0.0, color: [40, 32, 28, 235] });
        }
        Pattern::Marble => {
            full_grad(&mut v);
            // Adern: sanfte Zufallswege als durchgehende Linienzüge
            for k in 0..9 {
                let gold = k % 4 == 3;
                let color: Rgba = if gold { [196, 160, 80, 170] } else { [125, 130, 140, 45 + (k * 8) as u8] };
                let mut x = rng.next() * w * 1.2 - w * 0.1;
                let mut y = -s;
                let drift = (rng.next() - 0.5) * 1.4;
                let mut dx = drift;
                let lw = s * (0.04 + rng.next() * 0.1);
                let mut pts = vec![(x, y)];
                while y < h + s {
                    dx = (dx * 0.7 + drift * 0.3 + (rng.next() - 0.5) * 0.9).clamp(-1.6, 1.6);
                    x += dx * s * 0.35;
                    y += s * 0.35;
                    pts.push((x, y));
                }
                v.push(Prim::Poly { pts, w: lw, color });
            }
        }
        Pattern::Dots => {
            v.push(Prim::Fill { x: 0.0, y: 0.0, w, h, r: 0.0, color: c1 });
            let step = s * 1.6;
            let mut row = 0;
            let mut y = step * 0.5;
            while y < h + step {
                let off = if row % 2 == 0 { 0.0 } else { step * 0.5 };
                let mut x = off;
                while x < w + step {
                    v.push(Prim::Circle { x, y, r: s * 0.32, color: c2 });
                    x += step;
                }
                y += step * 0.87;
                row += 1;
            }
        }
        Pattern::Stripes => {
            v.push(Prim::Fill { x: 0.0, y: 0.0, w, h, r: 0.0, color: c1 });
            let step = s * 1.4;
            let mut x = 0.0;
            while x < w {
                v.push(Prim::Fill { x, y: 0.0, w: step * 0.5, h, r: 0.0, color: c2 });
                x += step;
            }
        }
        Pattern::Plaid => {
            v.push(Prim::Fill { x: 0.0, y: 0.0, w, h, r: 0.0, color: c1 });
            let step = s * 4.0;
            let dark = a(c2, 120);
            let mut p = 0.0;
            while p < w.max(h) {
                v.push(Prim::Fill { x: p, y: 0.0, w: step * 0.4, h, r: 0.0, color: dark });
                v.push(Prim::Fill { x: 0.0, y: p, w, h: step * 0.4, r: 0.0, color: dark });
                seg(&mut v, p + step * 0.7, 0.0, p + step * 0.7, h, s * 0.08, [255, 236, 160, 170]);
                seg(&mut v, 0.0, p + step * 0.7, w, p + step * 0.7, s * 0.08, [255, 236, 160, 170]);
                p += step;
            }
        }
        Pattern::Stars => {
            full_grad(&mut v);
            for _ in 0..(w * h / (s * s) * 0.9) as usize {
                let (x, y) = (rng.next() * w, rng.next() * h);
                let r = s * (0.03 + rng.next().powi(3) * 0.14);
                v.push(Prim::Circle { x, y, r, color: [255, 255, 255, (120.0 + rng.next() * 135.0) as u8] });
            }
            v.push(Prim::Circle { x: w * 0.74, y: h * 0.16, r: s * 1.6, color: [255, 244, 214, 245] });
            // Sichel: Scheibe in der Farbe des Verlaufs an dieser Stelle
            let (mx, my) = (w * 0.74 + s * 0.6, h * 0.16 - s * 0.4);
            v.push(Prim::Circle { x: mx, y: my, r: s * 1.4, color: lerp(c1, c2, (mx * w + my * h) / (w * w + h * h)) });
        }
        Pattern::Waves => {
            full_grad(&mut v);
            let mut y = s * 1.2;
            let mut k = 0;
            while y < h + s {
                let amp = s * 0.45;
                let len = s * 3.0;
                let mut x = -len;
                let mut pts = Vec::new();
                while x < w + len {
                    pts.push((x, y + amp * ((x / len) * std::f32::consts::TAU + k as f32).sin()));
                    x += len * 0.08;
                }
                v.push(Prim::Poly { pts, w: s * 0.16, color: [255, 255, 255, 60] });
                y += s * 1.3;
                k += 1;
            }
        }
        Pattern::Terrazzo => {
            v.push(Prim::Fill { x: 0.0, y: 0.0, w, h, r: 0.0, color: c1 });
            let chips: [Rgba; 6] = [[231, 111, 81, 255], [244, 162, 97, 255], [42, 157, 143, 255], [38, 70, 83, 255], [233, 196, 106, 255], [214, 140, 160, 255]];
            for _ in 0..(w * h / (s * s) * 0.55) as usize {
                let (x, y) = (rng.next() * w, rng.next() * h);
                let c = chips[(rng.next() * chips.len() as f32) as usize % chips.len()];
                v.push(Prim::Circle { x, y, r: s * (0.12 + rng.next() * 0.35), color: c });
            }
        }
        Pattern::Rainbow => {
            let bands: [Rgba; 7] = [[255, 173, 173, 255], [255, 214, 165, 255], [253, 255, 182, 255], [202, 255, 191, 255], [155, 246, 255, 255], [160, 196, 255, 255], [189, 178, 255, 255]];
            let bh = h / bands.len() as f32;
            for (i, c) in bands.iter().enumerate() {
                v.push(Prim::Fill { x: 0.0, y: bh * i as f32, w, h: bh + 0.5, r: 0.0, color: *c });
            }
        }
        Pattern::Sunset => {
            full_grad(&mut v);
            let r = (w * 0.32).min(h * 0.3);
            let (cx, cy) = (w * 0.5, h * 0.72);
            v.push(Prim::Circle { x: cx, y: cy, r, color: [255, 214, 102, 255] });
            // Streifen durch die Sonne
            for k in 0..5 {
                let y = cy - r * 0.1 + k as f32 * r * 0.22;
                v.push(Prim::Fill { x: 0.0, y, w, h: r * (0.04 + k as f32 * 0.025), r: 0.0, color: a(c2, 255) });
            }
            if h - cy - r * 0.9 > 0.0 {
                v.push(Prim::Fill { x: 0.0, y: cy + r * 0.9, w, h: h - cy - r * 0.9, r: 0.0, color: a(c2, 255) });
            }
        }
    }
    v
}

/// Deckblatt: Cover + Buchrücken + Titelschild. Liefert außerdem die Fläche des Titelschilds.
pub fn cover_page_prims(idx: usize, w: f32, h: f32) -> Vec<Prim> {
    let mut v = cover_prims(idx, w, h);
    let s = w.min(h) / 20.0;
    // Buchrücken
    v.push(Prim::Fill { x: 0.0, y: 0.0, w: s * 0.9, h, r: 0.0, color: [0, 0, 0, 40] });
    v.push(Prim::Seg { x0: s * 0.9, y0: 0.0, x1: s * 0.9, y1: h, w: s * 0.06, color: [255, 255, 255, 40], dash: false });
    // Titelschild
    let (x, y, pw, ph) = title_plate(w, h);
    v.push(Prim::Fill { x: x + s * 0.08, y: y + s * 0.14, w: pw, h: ph, r: s * 0.5, color: [0, 0, 0, 45] });
    v.push(Prim::Fill { x, y, w: pw, h: ph, r: s * 0.5, color: [255, 255, 255, 238] });
    v.push(Prim::Seg { x0: x + s * 0.6, y0: y + ph - s * 0.55, x1: x + pw * 0.45, y1: y + ph - s * 0.55, w: s * 0.1, color: [109, 92, 255, 200], dash: false });
    v
}

/// Titelschild (x, y, Breite, Höhe).
pub fn title_plate(w: f32, h: f32) -> (f32, f32, f32, f32) {
    let pw = w * 0.62;
    let ph = (h * 0.15).max(w.min(h) * 0.16);
    (w * 0.22, h * 0.24, pw, ph)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_covers_draw() {
        assert_eq!(PATTERNS.len(), super::super::note::COVERS.len());
        for i in 0..PATTERNS.len() {
            let p = cover_prims(i, 595.0, 842.0);
            assert!(!p.is_empty());
            assert!(p.len() < 6000, "zu viele Formen: {} bei {}", p.len(), i);
            for (cw, ch) in [(300.0, 300.0), (300.0, 200.0), (300.0, 424.0)] {
                for q in cover_prims(i, cw, ch) {
                    if let Prim::Fill { w, h, .. } | Prim::Grad { w, h, .. } = q {
                        assert!(w >= 0.0 && h >= 0.0, "negative Fläche bei Cover {}", i);
                    }
                }
            }
        }
        assert_eq!(hex("#ff8000"), [255, 128, 0, 255]);
    }
}
