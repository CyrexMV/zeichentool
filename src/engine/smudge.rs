use super::brush::{dab_coverage, Dab};
use super::rect::IRect;

/// Wischfinger: zieht vorhandene Farbe in Strichrichtung mit.
pub struct Smudger {
    prev: Vec<Option<(f32, f32)>>,
    tmp: Vec<f32>,
    pub touched: IRect,
}

impl Smudger {
    pub fn new(copies: usize) -> Self {
        Smudger { prev: vec![None; copies.max(1)], tmp: Vec::new(), touched: IRect::EMPTY }
    }

    /// `data` ist die vormultiplizierte Ebene (w×h), `strength` 0..1.
    pub fn dab(&mut self, data: &mut [u8], w: usize, h: usize, d: &Dab, strength: f32) {
        let slot = (d.sym as usize).min(self.prev.len() - 1);
        let (px0, py0) = match self.prev[slot] {
            Some(p) => p,
            None => {
                self.prev[slot] = Some((d.x, d.y));
                return;
            }
        };
        self.prev[slot] = Some((d.x, d.y));
        let (ox, oy) = (d.x - px0, d.y - py0);
        let rect = d.bounds().clamp(w, h);
        if rect.is_empty() {
            return;
        }
        // Quellbereich (verschoben) zwischenspeichern, weil Lesen und Schreiben überlappen
        let src = IRect::new(
            (rect.x0 as f32 - ox).floor() as i32 - 1,
            (rect.y0 as f32 - oy).floor() as i32 - 1,
            (rect.x1 as f32 - ox).ceil() as i32 + 1,
            (rect.y1 as f32 - oy).ceil() as i32 + 1,
        );
        let sw = src.width();
        self.tmp.clear();
        self.tmp.resize(sw * src.height() * 4, 0.0);
        for (j, y) in (src.y0..src.y1).enumerate() {
            if y < 0 || y >= h as i32 {
                continue;
            }
            for (i, x) in (src.x0..src.x1).enumerate() {
                if x < 0 || x >= w as i32 {
                    continue;
                }
                let s = (y as usize * w + x as usize) * 4;
                let t = (j * sw + i) * 4;
                for c in 0..4 {
                    self.tmp[t + c] = data[s + c] as f32;
                }
            }
        }
        let sample = |tmp: &[f32], fx: f32, fy: f32| -> [f32; 4] {
            let lx = fx - src.x0 as f32 - 0.5;
            let ly = fy - src.y0 as f32 - 0.5;
            let ix = lx.floor().clamp(0.0, (sw - 2) as f32) as usize;
            let iy = ly.floor().clamp(0.0, (src.height() - 2) as f32) as usize;
            let tx = (lx - ix as f32).clamp(0.0, 1.0);
            let ty = (ly - iy as f32).clamp(0.0, 1.0);
            let mut out = [0f32; 4];
            for (c, o) in out.iter_mut().enumerate() {
                let p = |x: usize, y: usize| tmp[(y * sw + x) * 4 + c];
                let a = p(ix, iy) + (p(ix + 1, iy) - p(ix, iy)) * tx;
                let b = p(ix, iy + 1) + (p(ix + 1, iy + 1) - p(ix, iy + 1)) * tx;
                *o = a + (b - a) * ty;
            }
            out
        };
        let (sin, cos) = d.angle.sin_cos();
        let inv_aspect = 1.0 / d.aspect.clamp(0.05, 1.0);
        for y in rect.y0..rect.y1 {
            for x in rect.x0..rect.x1 {
                let k = dab_coverage(d, x, y, cos, sin, inv_aspect) * strength;
                if k <= 0.0 {
                    continue;
                }
                let s = sample(&self.tmp, x as f32 + 0.5 - ox, y as f32 + 0.5 - oy);
                let i = (y as usize * w + x as usize) * 4;
                let a = data[i + 3] as f32 + (s[3] - data[i + 3] as f32) * k;
                data[i + 3] = a.round().clamp(0.0, 255.0) as u8;
                for c in 0..3 {
                    let v = data[i + c] as f32 + (s[c] - data[i + c] as f32) * k;
                    data[i + c] = v.round().clamp(0.0, a.round().clamp(0.0, 255.0)) as u8;
                }
            }
        }
        self.touched = self.touched.union(&rect);
    }
}
