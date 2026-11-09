//! Geometrie-Helfer: Rechtecke, Transformationen, Abstände, Polygone.

/// Achsenparalleles Rechteck in Seitenkoordinaten (pt).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Rect {
    pub const EMPTY: Rect = Rect { x0: f32::MAX, y0: f32::MAX, x1: f32::MIN, y1: f32::MIN };

    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect { x0: x0.min(x1), y0: y0.min(y1), x1: x0.max(x1), y1: y0.max(y1) }
    }

    pub fn is_empty(&self) -> bool {
        self.x1 < self.x0 || self.y1 < self.y0
    }

    pub fn w(&self) -> f32 {
        (self.x1 - self.x0).max(0.0)
    }

    pub fn h(&self) -> f32 {
        (self.y1 - self.y0).max(0.0)
    }

    pub fn add(&mut self, x: f32, y: f32) {
        self.x0 = self.x0.min(x);
        self.y0 = self.y0.min(y);
        self.x1 = self.x1.max(x);
        self.y1 = self.y1.max(y);
    }

    pub fn union(&self, o: &Rect) -> Rect {
        Rect { x0: self.x0.min(o.x0), y0: self.y0.min(o.y0), x1: self.x1.max(o.x1), y1: self.y1.max(o.y1) }
    }

    pub fn expand(&self, d: f32) -> Rect {
        Rect { x0: self.x0 - d, y0: self.y0 - d, x1: self.x1 + d, y1: self.y1 + d }
    }

    pub fn intersects(&self, o: &Rect) -> bool {
        !(o.x0 > self.x1 || o.x1 < self.x0 || o.y0 > self.y1 || o.y1 < self.y0)
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1
    }

    pub fn center(&self) -> (f32, f32) {
        ((self.x0 + self.x1) * 0.5, (self.y0 + self.y1) * 0.5)
    }
}

/// Affine Transformation: x' = a·x + c·y + e, y' = b·x + d·y + f
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl Xf {
    pub const IDENTITY: Xf = Xf { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    pub fn translate(dx: f32, dy: f32) -> Xf {
        Xf { e: dx, f: dy, ..Xf::IDENTITY }
    }

    /// Skalierung um einen festen Punkt.
    pub fn scale_about(sx: f32, sy: f32, cx: f32, cy: f32) -> Xf {
        Xf { a: sx, b: 0.0, c: 0.0, d: sy, e: cx - sx * cx, f: cy - sy * cy }
    }

    /// Drehung um 90° im Uhrzeigersinn (Bildschirm: y nach unten) um (cx, cy).
    pub fn rot90_about(cx: f32, cy: f32) -> Xf {
        Xf { a: 0.0, b: 1.0, c: -1.0, d: 0.0, e: cx + cy, f: cy - cx }
    }

    /// Spiegelung an der senkrechten (`horizontal`) bzw. waagrechten Achse durch (cx, cy).
    pub fn flip_about(horizontal: bool, cx: f32, cy: f32) -> Xf {
        if horizontal {
            Xf { a: -1.0, b: 0.0, c: 0.0, d: 1.0, e: 2.0 * cx, f: 0.0 }
        } else {
            Xf { a: 1.0, b: 0.0, c: 0.0, d: -1.0, e: 0.0, f: 2.0 * cy }
        }
    }

    /// Nur Verschiebung/positive Skalierung (keine Drehung oder Spiegelung)?
    pub fn is_axis_scale(&self) -> bool {
        self.b == 0.0 && self.c == 0.0 && self.a > 0.0 && self.d > 0.0
    }

    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }

    /// Mittlerer Skalierungsfaktor (für Strichbreiten).
    pub fn scale(&self) -> f32 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }
}

#[inline]
pub fn dist(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt()
}

/// Abstand Punkt–Strecke.
pub fn dist_point_seg(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (dx, dy) = (bx - ax, by - ay);
    let l2 = dx * dx + dy * dy;
    let t = if l2 <= 1e-12 { 0.0 } else { (((px - ax) * dx + (py - ay) * dy) / l2).clamp(0.0, 1.0) };
    dist(px, py, ax + t * dx, ay + t * dy)
}

fn orient(ax: f32, ay: f32, bx: f32, by: f32, cx: f32, cy: f32) -> f32 {
    (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
}

/// Abstand zweier Strecken (0, wenn sie sich schneiden).
pub fn dist_seg_seg(a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) -> f32 {
    let o1 = orient(a.0, a.1, b.0, b.1, c.0, c.1);
    let o2 = orient(a.0, a.1, b.0, b.1, d.0, d.1);
    let o3 = orient(c.0, c.1, d.0, d.1, a.0, a.1);
    let o4 = orient(c.0, c.1, d.0, d.1, b.0, b.1);
    if o1 * o2 < 0.0 && o3 * o4 < 0.0 {
        return 0.0;
    }
    dist_point_seg(a.0, a.1, c.0, c.1, d.0, d.1)
        .min(dist_point_seg(b.0, b.1, c.0, c.1, d.0, d.1))
        .min(dist_point_seg(c.0, c.1, a.0, a.1, b.0, b.1))
        .min(dist_point_seg(d.0, d.1, a.0, a.1, b.0, b.1))
}

/// Punkt in Polygon (Even-Odd).
pub fn point_in_polygon(x: f32, y: f32, poly: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Ramer–Douglas–Peucker-Vereinfachung, liefert Indizes der behaltenen Punkte.
pub fn rdp(pts: &[(f32, f32)], eps: f32) -> Vec<usize> {
    if pts.len() < 3 {
        return (0..pts.len()).collect();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0usize, pts.len() - 1)];
    while let Some((s, e)) = stack.pop() {
        let (ax, ay) = pts[s];
        let (bx, by) = pts[e];
        let mut best = (0.0f32, 0usize);
        for (i, &(px, py)) in pts.iter().enumerate().take(e).skip(s + 1) {
            let d = dist_point_seg(px, py, ax, ay, bx, by);
            if d > best.0 {
                best = (d, i);
            }
        }
        if best.0 > eps {
            keep[best.1] = true;
            stack.push((s, best.1));
            stack.push((best.1, e));
        }
    }
    keep.iter().enumerate().filter(|(_, k)| **k).map(|(i, _)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments() {
        assert_eq!(dist_seg_seg((0.0, 0.0), (10.0, 10.0), (0.0, 10.0), (10.0, 0.0)), 0.0);
        assert!((dist_seg_seg((0.0, 0.0), (10.0, 0.0), (0.0, 3.0), (10.0, 3.0)) - 3.0).abs() < 1e-5);
        assert!((dist_point_seg(5.0, 5.0, 0.0, 0.0, 10.0, 0.0) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn polygon_and_rdp() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        assert!(point_in_polygon(5.0, 5.0, &sq));
        assert!(!point_in_polygon(15.0, 5.0, &sq));
        let line: Vec<_> = (0..20).map(|i| (i as f32, (i as f32 * 0.01).sin() * 0.01)).collect();
        assert_eq!(rdp(&line, 0.5), vec![0, 19]);
    }

    #[test]
    fn transform_scale() {
        let x = Xf::scale_about(2.0, 2.0, 10.0, 10.0);
        assert_eq!(x.apply(10.0, 10.0), (10.0, 10.0));
        assert_eq!(x.apply(11.0, 10.0), (12.0, 10.0));
        assert!((x.scale() - 2.0).abs() < 1e-6);
    }
}
