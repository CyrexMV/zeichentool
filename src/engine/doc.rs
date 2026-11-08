use super::color::Color;
use super::layer::{BlendMode, Layer, LayerProps};
use super::mask::Mask;
use super::rect::IRect;

/// Was gerade „über“ einer Ebene liegt und live angezeigt werden soll.
pub enum Overlay<'a> {
    None,
    /// Ein laufender Mal-Strich auf `layer`.
    Paint { layer: usize, mask: &'a Mask, color: Color, opacity: f32 },
    /// Ein laufender Radierer-Strich.
    Erase { layer: usize, mask: &'a Mask, opacity: f32 },
    /// Verschieben einer Ebene.
    Offset { layer: usize, dx: i32, dy: i32 },
}

pub struct Document {
    pub w: usize,
    pub h: usize,
    pub layers: Vec<Layer>,
    pub active: usize,
    /// Papierfarbe, `None` = transparent.
    pub paper: Option<Color>,
    pub next_id: u32,
}

#[inline]
fn px(data: &[u8], i: usize) -> [f32; 4] {
    const K: f32 = 1.0 / 255.0;
    [data[i] as f32 * K, data[i + 1] as f32 * K, data[i + 2] as f32 * K, data[i + 3] as f32 * K]
}

#[inline]
fn store(data: &mut [u8], i: usize, p: [f32; 4]) {
    let a = p[3].clamp(0.0, 1.0);
    data[i + 3] = (a * 255.0 + 0.5) as u8;
    // Vormultipliziert: Farbe darf Alpha nicht übersteigen
    for c in 0..3 {
        data[i + c] = (p[c].clamp(0.0, a) * 255.0 + 0.5) as u8;
    }
}

#[inline(always)]
fn blend_into(a: &mut [f32; 4], s: [f32; 4], op: f32, mode: BlendMode) {
    let s = if op < 1.0 { [s[0] * op, s[1] * op, s[2] * op, s[3] * op] } else { s };
    if s[3] <= 0.0 {
        return;
    }
    if mode == BlendMode::Normal {
        let k = 1.0 - s[3];
        *a = [s[0] + a[0] * k, s[1] + a[1] * k, s[2] + a[2] * k, s[3] + a[3] * k];
    } else {
        *a = mode.blend(s, *a);
    }
}

/// Wendet Farbe mit Deckung `m` auf ein vormultipliziertes Pixel an.
#[inline]
pub fn paint_pixel(p: [f32; 4], c: &Color, m: f32, alpha_lock: bool) -> [f32; 4] {
    let k = 1.0 - m;
    if alpha_lock {
        let a = p[3];
        [p[0] * k + c.r * a * m, p[1] * k + c.g * a * m, p[2] * k + c.b * a * m, a]
    } else {
        [p[0] * k + c.r * m, p[1] * k + c.g * m, p[2] * k + c.b * m, p[3] * k + m]
    }
}

impl Document {
    pub fn new(w: usize, h: usize) -> Self {
        let mut d = Document { w, h, layers: Vec::new(), active: 0, paper: Some(Color::WHITE), next_id: 1 };
        d.add_layer("Ebene 1", 0);
        d
    }

    pub fn bounds(&self) -> IRect {
        IRect::full(self.w, self.h)
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Fügt eine leere Ebene an Position `index` ein (0 = unten).
    pub fn add_layer(&mut self, name: &str, index: usize) -> usize {
        let id = self.alloc_id();
        let idx = index.min(self.layers.len());
        self.layers.insert(idx, Layer::new(id, name, self.w, self.h));
        self.active = idx;
        idx
    }

    pub fn next_layer_name(&self) -> String {
        let mut n = self.layers.len() + 1;
        loop {
            let name = format!("Ebene {}", n);
            if !self.layers.iter().any(|l| l.props.name == name) {
                return name;
            }
            n += 1;
        }
    }

    pub fn index_of(&self, id: u32) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    pub fn active_layer(&self) -> &Layer {
        &self.layers[self.active]
    }

    pub fn active_layer_mut(&mut self) -> &mut Layer {
        &mut self.layers[self.active]
    }

    /// Kopiert einen Bereich einer Ebene (RGBA8).
    pub fn read_region(&self, layer: usize, r: IRect) -> Vec<u8> {
        let r = r.clamp(self.w, self.h);
        let mut out = Vec::with_capacity(r.area() * 4);
        let data = &self.layers[layer].data;
        for y in r.y0..r.y1 {
            let s = (y as usize * self.w + r.x0 as usize) * 4;
            out.extend_from_slice(&data[s..s + r.width() * 4]);
        }
        out
    }

    pub fn write_region(&mut self, layer: usize, r: IRect, bytes: &[u8]) {
        let r = r.clamp(self.w, self.h);
        let w = self.w;
        let rw = r.width() * 4;
        let l = &mut self.layers[layer];
        for (row, y) in (r.y0..r.y1).enumerate() {
            let s = (y as usize * w + r.x0 as usize) * 4;
            l.data[s..s + rw].copy_from_slice(&bytes[row * rw..row * rw + rw]);
        }
        l.touch();
    }

    /// Übernimmt einen Mal- oder Radierstrich endgültig in die Ebene.
    pub fn commit_overlay(&mut self, overlay: &Overlay) {
        match *overlay {
            Overlay::Paint { layer, mask, color, opacity } => {
                let r = mask.dirty.clamp(self.w, self.h);
                let w = self.w;
                let l = &mut self.layers[layer];
                let lock = l.props.alpha_lock;
                for y in r.y0..r.y1 {
                    for x in r.x0..r.x1 {
                        let m = mask.data[y as usize * w + x as usize];
                        if m == 0 {
                            continue;
                        }
                        let i = (y as usize * w + x as usize) * 4;
                        let p = paint_pixel(px(&l.data, i), &color, m as f32 / 65535.0 * opacity, lock);
                        store(&mut l.data, i, p);
                    }
                }
                l.touch();
            }
            Overlay::Erase { layer, mask, opacity } => {
                let r = mask.dirty.clamp(self.w, self.h);
                let w = self.w;
                let l = &mut self.layers[layer];
                for y in r.y0..r.y1 {
                    for x in r.x0..r.x1 {
                        let m = mask.data[y as usize * w + x as usize];
                        if m == 0 {
                            continue;
                        }
                        let i = (y as usize * w + x as usize) * 4;
                        let k = 1.0 - m as f32 / 65535.0 * opacity;
                        let p = px(&l.data, i);
                        store(&mut l.data, i, [p[0] * k, p[1] * k, p[2] * k, p[3] * k]);
                    }
                }
                l.touch();
            }
            Overlay::Offset { layer, dx, dy } => {
                let (w, h) = (self.w as i32, self.h as i32);
                let l = &mut self.layers[layer];
                let src = l.data.clone();
                l.data.fill(0);
                for y in 0..h {
                    let sy = y - dy;
                    if sy < 0 || sy >= h {
                        continue;
                    }
                    let x0 = dx.max(0);
                    let x1 = (w + dx).min(w);
                    if x1 <= x0 {
                        continue;
                    }
                    let d = ((y * w + x0) * 4) as usize;
                    let s = ((sy * w + x0 - dx) * 4) as usize;
                    let n = ((x1 - x0) * 4) as usize;
                    l.data[d..d + n].copy_from_slice(&src[s..s + n]);
                }
                l.touch();
            }
            Overlay::None => {}
        }
    }

    /// Setzt die Ebenen im Rechteck `r` zusammen und schreibt nicht-vormultipliziertes RGBA8 in `out`.
    pub fn composite(&self, r: IRect, overlay: &Overlay, with_paper: bool, out: &mut Vec<u8>) {
        let r = r.clamp(self.w, self.h);
        let rw = r.width();
        out.clear();
        out.resize(rw * r.height() * 4, 0);
        if rw == 0 {
            return;
        }
        let mut acc = vec![[0f32; 4]; rw];
        let paper = if with_paper { self.paper } else { None };
        for (row, y) in (r.y0..r.y1).enumerate() {
            let base = match paper {
                Some(c) => [c.r, c.g, c.b, 1.0],
                None => [0.0; 4],
            };
            acc.fill(base);
            let rowi = y as usize * self.w;
            let x0 = r.x0 as usize;
            for (li, layer) in self.layers.iter().enumerate() {
                if !layer.props.visible || layer.props.opacity <= 0.0 {
                    continue;
                }
                let op = layer.props.opacity;
                let mode = layer.props.blend;
                let data = &layer.data[..];
                // Overlay nur einmal pro Zeile und Ebene auswerten (schnelle innere Schleife)
                match overlay {
                    Overlay::Paint { layer: ol, mask, color, opacity } if *ol == li => {
                        let md = &mask.data[rowi + x0..rowi + x0 + rw];
                        let lock = layer.props.alpha_lock;
                        for (cx, a) in acc.iter_mut().enumerate() {
                            let i = (rowi + x0 + cx) * 4;
                            let m = md[cx];
                            if m == 0 && data[i + 3] == 0 {
                                continue;
                            }
                            let mut s = px(data, i);
                            if m > 0 {
                                s = paint_pixel(s, color, m as f32 * (1.0 / 65535.0) * opacity, lock);
                            }
                            blend_into(a, s, op, mode);
                        }
                    }
                    Overlay::Erase { layer: ol, mask, opacity } if *ol == li => {
                        let md = &mask.data[rowi + x0..rowi + x0 + rw];
                        for (cx, a) in acc.iter_mut().enumerate() {
                            let i = (rowi + x0 + cx) * 4;
                            if data[i + 3] == 0 {
                                continue;
                            }
                            let mut s = px(data, i);
                            let m = md[cx];
                            if m > 0 {
                                let k = 1.0 - m as f32 * (1.0 / 65535.0) * opacity;
                                s = [s[0] * k, s[1] * k, s[2] * k, s[3] * k];
                            }
                            blend_into(a, s, op, mode);
                        }
                    }
                    Overlay::Offset { layer: ol, dx, dy } if *ol == li => {
                        let sy = y - dy;
                        if sy < 0 || sy >= self.h as i32 {
                            continue;
                        }
                        let srow = sy as usize * self.w;
                        for (cx, a) in acc.iter_mut().enumerate() {
                            let sx = (x0 + cx) as i32 - dx;
                            if sx < 0 || sx >= self.w as i32 {
                                continue;
                            }
                            let i = (srow + sx as usize) * 4;
                            if data[i + 3] == 0 {
                                continue;
                            }
                            blend_into(a, px(data, i), op, mode);
                        }
                    }
                    _ => {
                        let row = &data[(rowi + x0) * 4..(rowi + x0 + rw) * 4];
                        for (p, a) in row.chunks_exact(4).zip(acc.iter_mut()) {
                            if p[3] == 0 {
                                continue;
                            }
                            const K: f32 = 1.0 / 255.0;
                            let s = [p[0] as f32 * K, p[1] as f32 * K, p[2] as f32 * K, p[3] as f32 * K];
                            blend_into(a, s, op, mode);
                        }
                    }
                }
            }
            let o = row * rw * 4;
            for (cx, a) in acc.iter().enumerate() {
                let i = o + cx * 4;
                let al = a[3].clamp(0.0, 1.0);
                if al <= 0.0 {
                    continue;
                }
                let inv = if al >= 0.999 { 1.0 } else { 1.0 / al };
                out[i] = ((a[0] * inv).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                out[i + 1] = ((a[1] * inv).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                out[i + 2] = ((a[2] * inv).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                out[i + 3] = (al * 255.0 + 0.5) as u8;
            }
        }
    }

    /// Begrenzungsrechteck aller nicht-transparenten Pixel einer Ebene.
    pub fn content_bounds(&self, layer: usize) -> IRect {
        let data = &self.layers[layer].data;
        let mut r = IRect::EMPTY;
        for y in 0..self.h {
            let row = &data[y * self.w * 4..(y + 1) * self.w * 4];
            let first = row.chunks_exact(4).position(|p| p[3] != 0);
            if let Some(x0) = first {
                let x1 = self.w - row.chunks_exact(4).rev().position(|p| p[3] != 0).unwrap_or(0);
                r = r.union(&IRect::new(x0 as i32, y as i32, x1 as i32, y as i32 + 1));
            }
        }
        r
    }

    /// Farbe des sichtbaren Bildes an einer Stelle (Pipette).
    pub fn sample(&self, x: i32, y: i32) -> Option<Color> {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return None;
        }
        let mut out = Vec::new();
        self.composite(IRect::new(x, y, x + 1, y + 1), &Overlay::None, true, &mut out);
        if out[3] == 0 {
            return None;
        }
        Some(Color::from_u8(out[0], out[1], out[2]))
    }

    /// Fügt die aktive Ebene in die darunterliegende ein. Liefert die Indizes (oben, unten).
    pub fn merge_down(&mut self, upper: usize) -> Option<usize> {
        if upper == 0 || upper >= self.layers.len() {
            return None;
        }
        let up = self.layers.remove(upper);
        let lower = upper - 1;
        let l = &mut self.layers[lower];
        let op = up.props.opacity;
        let mode = up.props.blend;
        if up.props.visible {
            for i in (0..l.data.len()).step_by(4) {
                let mut s = px(&up.data, i);
                if s[3] <= 0.0 {
                    continue;
                }
                s = [s[0] * op, s[1] * op, s[2] * op, s[3] * op];
                let d = px(&l.data, i);
                store(&mut l.data, i, mode.blend(s, d));
            }
        }
        l.touch();
        self.active = lower;
        Some(lower)
    }

    pub fn flip_layer(&mut self, layer: usize, horizontal: bool) {
        let (w, h) = (self.w, self.h);
        let l = &mut self.layers[layer];
        if horizontal {
            for y in 0..h {
                let row = &mut l.data[y * w * 4..(y + 1) * w * 4];
                for x in 0..w / 2 {
                    for c in 0..4 {
                        row.swap(x * 4 + c, (w - 1 - x) * 4 + c);
                    }
                }
            }
        } else {
            for y in 0..h / 2 {
                let (a, b) = l.data.split_at_mut((h - 1 - y) * w * 4);
                a[y * w * 4..(y + 1) * w * 4].swap_with_slice(&mut b[..w * 4]);
            }
        }
        l.touch();
    }

    /// Verkleinertes Vorschaubild einer Ebene (nicht vormultipliziertes RGBA8).
    pub fn thumbnail(&self, layer: usize, tw: usize, th: usize) -> Vec<u8> {
        let mut out = vec![0u8; tw * th * 4];
        let data = &self.layers[layer].data;
        let sx = self.w as f32 / tw as f32;
        let sy = self.h as f32 / th as f32;
        // Pro Zielpixel 3x3 Stichproben – schnell und zeigt auch dünne Linien.
        for ty in 0..th {
            for tx in 0..tw {
                let mut acc = [0f32; 4];
                for j in 0..3 {
                    for i in 0..3 {
                        let x = (((tx as f32) + (i as f32 + 0.5) / 3.0) * sx) as usize;
                        let y = (((ty as f32) + (j as f32 + 0.5) / 3.0) * sy) as usize;
                        let p = px(data, (y.min(self.h - 1) * self.w + x.min(self.w - 1)) * 4);
                        for c in 0..4 {
                            acc[c] += p[c];
                        }
                    }
                }
                let a = acc[3] / 9.0;
                let o = (ty * tw + tx) * 4;
                if a > 0.0 {
                    for c in 0..3 {
                        out[o + c] = ((acc[c] / 9.0 / a).clamp(0.0, 1.0) * 255.0) as u8;
                    }
                    out[o + 3] = (a.min(1.0) * 255.0) as u8;
                }
            }
        }
        out
    }

    /// Fügt ein Bild (nicht vormultipliziertes RGBA8) als neue Ebene ein.
    pub fn insert_image_layer(&mut self, name: &str, rgba: &[u8], index: usize) -> usize {
        let idx = self.add_layer(name, index);
        let l = &mut self.layers[idx];
        for (d, s) in l.data.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
            let a = s[3] as u32;
            d[0] = ((s[0] as u32 * a + 127) / 255) as u8;
            d[1] = ((s[1] as u32 * a + 127) / 255) as u8;
            d[2] = ((s[2] as u32 * a + 127) / 255) as u8;
            d[3] = s[3];
        }
        l.touch();
        idx
    }

    pub fn duplicate_layer(&mut self, index: usize) -> usize {
        let id = self.alloc_id();
        let mut l = self.layers[index].clone();
        l.id = id;
        l.props = LayerProps { name: format!("{} Kopie", l.props.name), ..l.props };
        l.touch();
        self.layers.insert(index + 1, l);
        self.active = index + 1;
        index + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::brush::{render_dab, Dab};
    use crate::engine::noise::Noise;

    #[test]
    fn paint_commit_and_composite() {
        let mut d = Document::new(20, 20);
        d.paper = None;
        let mut m = Mask::new(20, 20);
        let dab = Dab { x: 10.0, y: 10.0, radius: 4.0, alpha: 1.0, hardness: 1.0, aspect: 1.0, angle: 0.0, grain: 0.0, pixel: false, sym: 0 };
        render_dab(&mut m, &dab, false, &Noise::new());
        let red = Color::rgb(1.0, 0.0, 0.0);
        let mut out = Vec::new();
        d.composite(d.bounds(), &Overlay::Paint { layer: 0, mask: &m, color: red, opacity: 1.0 }, false, &mut out);
        let i = (10 * 20 + 10) * 4;
        assert_eq!(&out[i..i + 4], &[255, 0, 0, 255]);
        d.commit_overlay(&Overlay::Paint { layer: 0, mask: &m, color: red, opacity: 1.0 });
        assert_eq!(d.sample(10, 10), Some(red));
        assert_eq!(d.sample(0, 0), None);
    }

    #[test]
    fn offset_moves_pixels() {
        let mut d = Document::new(10, 10);
        let i = (2 * 10 + 2) * 4;
        d.layers[0].data[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
        d.commit_overlay(&Overlay::Offset { layer: 0, dx: 3, dy: 1 });
        let j = (3 * 10 + 5) * 4;
        assert_eq!(d.layers[0].data[j + 3], 255);
        assert_eq!(d.layers[0].data[i + 3], 0);
    }

    #[test]
    fn flip_vertical() {
        let mut d = Document::new(4, 3);
        d.layers[0].data[3] = 255; // (0,0)
        d.flip_layer(0, false);
        assert_eq!(d.layers[0].data[(2 * 4) * 4 + 3], 255);
        d.flip_layer(0, true);
        assert_eq!(d.layers[0].data[(2 * 4 + 3) * 4 + 3], 255);
    }
}
