//! Vektor-Tinte: Striche mit Druck, Umrissberechnung (variable Breite), Treffertests.

use super::geom::{dist, dist_point_seg, dist_seg_seg, point_in_polygon, Rect, Xf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PenKind {
    /// Füller: Breite folgt stark dem Druck
    Fountain = 0,
    /// Kugelschreiber: gleichmäßige Linie
    Ball = 1,
    /// Pinselstift: sehr druckempfindlich, spitze Enden
    Brush = 2,
    /// Bleistift: leicht transparent, dünn
    Pencil = 3,
    /// Textmarker: breit, transparent, liegt unter der Schrift
    Highlighter = 4,
    /// Füllung einer geschlossenen Form (Punkte = Polygon)
    Fill = 5,
}

impl PenKind {
    pub const PENS: [PenKind; 4] = [PenKind::Fountain, PenKind::Ball, PenKind::Brush, PenKind::Pencil];

    pub fn from_u8(v: u8) -> PenKind {
        match v {
            1 => PenKind::Ball,
            2 => PenKind::Brush,
            3 => PenKind::Pencil,
            4 => PenKind::Highlighter,
            5 => PenKind::Fill,
            _ => PenKind::Fountain,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            PenKind::Fountain => "fountain",
            PenKind::Ball => "ball",
            PenKind::Brush => "brush",
            PenKind::Pencil => "pencil",
            PenKind::Highlighter => "highlighter",
            PenKind::Fill => "fill",
        }
    }

    pub fn from_code(s: &str) -> PenKind {
        match s {
            "ball" => PenKind::Ball,
            "brush" => PenKind::Brush,
            "pencil" => PenKind::Pencil,
            "highlighter" => PenKind::Highlighter,
            _ => PenKind::Fountain,
        }
    }

    pub fn name_de(&self) -> &'static str {
        match self {
            PenKind::Fountain => "Füller",
            PenKind::Ball => "Kugelschreiber",
            PenKind::Brush => "Pinselstift",
            PenKind::Pencil => "Bleistift",
            PenKind::Highlighter => "Textmarker",
            PenKind::Fill => "Füllung",
        }
    }

    /// Breitenfaktor für einen Druckwert (0..1).
    fn width_factor(&self, p: f32) -> f32 {
        let p = p.clamp(0.0, 1.0);
        match self {
            PenKind::Fountain => 0.35 + 0.95 * p.powf(0.8),
            PenKind::Ball => 0.85 + 0.25 * p,
            PenKind::Brush => 0.08 + 1.6 * p.powf(1.2),
            PenKind::Pencil => 0.6 + 0.6 * p,
            PenKind::Highlighter | PenKind::Fill => 1.0,
        }
    }

    /// Länge der Verjüngung an Anfang und Ende (in Strichbreiten).
    fn taper(&self) -> (f32, f32) {
        match self {
            PenKind::Fountain => (1.2, 2.0),
            PenKind::Brush => (2.5, 4.0),
            _ => (0.0, 0.0),
        }
    }

    pub fn round_caps(&self) -> bool {
        !matches!(self, PenKind::Highlighter | PenKind::Fill)
    }
}

/// Ein Stiftpunkt (pt) mit Druck 0..1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
    pub p: f32,
}

impl Pt {
    pub fn new(x: f32, y: f32, p: f32) -> Pt {
        Pt { x, y, p }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub pen: PenKind,
    /// RGBA, Alpha für Textmarker/Bleistift
    pub color: [u8; 4],
    /// Grundbreite in pt
    pub width: f32,
    pub pts: Vec<Pt>,
}

fn catmull(p0: Pt, p1: Pt, p2: Pt, p3: Pt, t: f32) -> Pt {
    let t2 = t * t;
    let t3 = t2 * t;
    let f = |a: f32, b: f32, c: f32, d: f32| 0.5 * ((2.0 * b) + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3);
    Pt { x: f(p0.x, p1.x, p2.x, p3.x), y: f(p0.y, p1.y, p2.y, p3.y), p: p1.p + (p2.p - p1.p) * t }
}

impl Stroke {
    pub fn bbox(&self) -> Rect {
        let mut r = Rect::EMPTY;
        for p in &self.pts {
            r.add(p.x, p.y);
        }
        r.expand(self.width * 1.0 + 1.0)
    }

    pub fn length(&self) -> f32 {
        self.pts.windows(2).map(|w| dist(w[0].x, w[0].y, w[1].x, w[1].y)).sum()
    }

    /// Weich interpolierte Punkte (Catmull-Rom), Abstand ~`step`.
    fn resample(&self, step: f32) -> Vec<Pt> {
        let p = &self.pts;
        if p.len() < 3 {
            return p.clone();
        }
        let mut out = Vec::with_capacity(p.len() * 3);
        out.push(p[0]);
        for i in 0..p.len() - 1 {
            let p0 = if i == 0 { p[0] } else { p[i - 1] };
            let p1 = p[i];
            let p2 = p[i + 1];
            let p3 = if i + 2 < p.len() { p[i + 2] } else { p[i + 1] };
            let seg = dist(p1.x, p1.y, p2.x, p2.y);
            let n = ((seg / step).ceil() as usize).clamp(1, 32);
            for k in 1..=n {
                out.push(catmull(p0, p1, p2, p3, k as f32 / n as f32));
            }
        }
        out
    }

    /// Umriss als geschlossenes Polygon (gefüllt mit Nonzero-Regel).
    pub fn outline(&self) -> Vec<(f32, f32)> {
        if self.pen == PenKind::Fill {
            return self.pts.iter().map(|p| (p.x, p.y)).collect();
        }
        let w = self.width.max(0.1);
        if self.pts.is_empty() {
            return Vec::new();
        }
        let pts = self.resample((w * 0.35).clamp(0.3, 3.0));
        let total: f32 = pts.windows(2).map(|q| dist(q[0].x, q[0].y, q[1].x, q[1].y)).sum();
        if pts.len() < 2 || total < 0.05 {
            let p = pts[0];
            let r = w * 0.5 * self.pen.width_factor(p.p).max(0.5);
            return circle(p.x, p.y, r, 16);
        }
        // Breiten mit Verjüngung und Glättung
        let (t0, t1) = self.pen.taper();
        let mut widths: Vec<f32> = Vec::with_capacity(pts.len());
        let mut acc = 0.0;
        let mut along = Vec::with_capacity(pts.len());
        for i in 0..pts.len() {
            if i > 0 {
                acc += dist(pts[i - 1].x, pts[i - 1].y, pts[i].x, pts[i].y);
            }
            along.push(acc);
        }
        for (i, p) in pts.iter().enumerate() {
            let mut f = self.pen.width_factor(p.p);
            if t0 > 0.0 {
                let ts = (along[i] / (w * t0)).min(1.0);
                f *= 0.35 + 0.65 * ts.sqrt();
            }
            if t1 > 0.0 {
                let te = ((total - along[i]) / (w * t1)).min(1.0);
                f *= 0.25 + 0.75 * te.sqrt();
            }
            widths.push(w * 0.5 * f);
        }
        for _ in 0..2 {
            let prev = widths.clone();
            for i in 1..widths.len() - 1 {
                widths[i] = (prev[i - 1] + prev[i] * 2.0 + prev[i + 1]) * 0.25;
            }
        }
        let n = pts.len();
        let mut left = Vec::with_capacity(n);
        let mut right = Vec::with_capacity(n);
        let mut dir = (1.0f32, 0.0f32);
        for i in 0..n {
            let a = pts[i.saturating_sub(1)];
            let b = pts[(i + 1).min(n - 1)];
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let l = (dx * dx + dy * dy).sqrt();
            if l > 1e-6 {
                dir = (dx / l, dy / l);
            }
            let (nx, ny) = (-dir.1, dir.0);
            let r = widths[i];
            left.push((pts[i].x + nx * r, pts[i].y + ny * r));
            right.push((pts[i].x - nx * r, pts[i].y - ny * r));
        }
        let mut out = Vec::with_capacity(n * 2 + 24);
        out.extend_from_slice(&left);
        let end = pts[n - 1];
        let start = pts[0];
        if self.pen.round_caps() {
            let ang = (left[n - 1].1 - end.y).atan2(left[n - 1].0 - end.x);
            arc(&mut out, end.x, end.y, widths[n - 1], ang, ang - std::f32::consts::PI, 8);
        }
        out.extend(right.iter().rev());
        if self.pen.round_caps() {
            let ang = (right[0].1 - start.y).atan2(right[0].0 - start.x);
            arc(&mut out, start.x, start.y, widths[0], ang, ang - std::f32::consts::PI, 8);
        }
        out
    }

    /// Berührt ein Radierer (Strecke a→b mit Radius r) diesen Strich?
    pub fn hit(&self, a: (f32, f32), b: (f32, f32), r: f32) -> bool {
        let rr = r + self.width * 0.5;
        if !self.bbox().expand(r).intersects(&Rect::new(a.0, a.1, b.0, b.1)) {
            return false;
        }
        if self.pen == PenKind::Fill {
            let poly: Vec<(f32, f32)> = self.pts.iter().map(|p| (p.x, p.y)).collect();
            return point_in_polygon(a.0, a.1, &poly) || point_in_polygon(b.0, b.1, &poly);
        }
        if self.pts.len() == 1 {
            return dist_point_seg(self.pts[0].x, self.pts[0].y, a.0, a.1, b.0, b.1) <= rr;
        }
        self.pts.windows(2).any(|w| dist_seg_seg((w[0].x, w[0].y), (w[1].x, w[1].y), a, b) <= rr)
    }

    /// Präzisionsradierer: entfernt die Teile innerhalb des Radius, liefert die verbleibenden Stücke.
    /// `None`, wenn der Strich nicht betroffen ist.
    pub fn erase_part(&self, a: (f32, f32), b: (f32, f32), r: f32) -> Option<Vec<Stroke>> {
        if !self.hit(a, b, r) {
            return None;
        }
        if self.pen == PenKind::Fill {
            return Some(Vec::new());
        }
        let rr = r + self.width * 0.35;
        // dichter abtasten, damit Schnitte sauber werden
        let dense = self.resample((self.width * 0.5).clamp(0.4, 2.0));
        let mut pieces = Vec::new();
        let mut cur: Vec<Pt> = Vec::new();
        for p in dense {
            if dist_point_seg(p.x, p.y, a.0, a.1, b.0, b.1) <= rr {
                if cur.len() >= 2 {
                    pieces.push(std::mem::take(&mut cur));
                } else {
                    cur.clear();
                }
            } else {
                cur.push(p);
            }
        }
        if cur.len() >= 2 {
            pieces.push(cur);
        }
        Some(
            pieces
                .into_iter()
                .filter(|p| p.windows(2).map(|w| dist(w[0].x, w[0].y, w[1].x, w[1].y)).sum::<f32>() > self.width * 0.3)
                .map(|pts| Stroke { pts, ..self.clone_meta() })
                .collect(),
        )
    }

    fn clone_meta(&self) -> Stroke {
        Stroke { pen: self.pen, color: self.color, width: self.width, pts: Vec::new() }
    }

    pub fn transformed(&self, xf: &Xf) -> Stroke {
        Stroke {
            pen: self.pen,
            color: self.color,
            width: self.width * xf.scale(),
            pts: self
                .pts
                .iter()
                .map(|p| {
                    let (x, y) = xf.apply(p.x, p.y);
                    Pt { x, y, p: p.p }
                })
                .collect(),
        }
    }
}

pub fn circle(cx: f32, cy: f32, r: f32, n: usize) -> Vec<(f32, f32)> {
    (0..n)
        .map(|i| {
            let t = std::f32::consts::TAU * i as f32 / n as f32;
            (cx + r * t.cos(), cy + r * t.sin())
        })
        .collect()
}

fn arc(out: &mut Vec<(f32, f32)>, cx: f32, cy: f32, r: f32, from: f32, to: f32, n: usize) {
    for i in 1..n {
        let t = from + (to - from) * i as f32 / n as f32;
        out.push((cx + r * t.cos(), cy + r * t.sin()));
    }
}

/// Glättet Eingabepunkte (Stabilisator), `amount` 0..1.
pub struct Smoother {
    amount: f32,
    last: Option<Pt>,
}

impl Smoother {
    pub fn new(amount: f32) -> Smoother {
        Smoother { amount: amount.clamp(0.0, 0.95), last: None }
    }

    pub fn push(&mut self, p: Pt) -> Pt {
        let k = (1.0 - self.amount).powf(1.5).max(0.05);
        let n = match self.last {
            None => p,
            Some(l) => Pt { x: l.x + (p.x - l.x) * k, y: l.y + (p.y - l.y) * k, p: l.p + (p.p - l.p) * k.max(0.4) },
        };
        self.last = Some(n);
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(n: usize) -> Stroke {
        Stroke {
            pen: PenKind::Ball,
            color: [0, 0, 0, 255],
            width: 2.0,
            pts: (0..n).map(|i| Pt::new(i as f32 * 2.0, 0.0, 0.6)).collect(),
        }
    }

    #[test]
    fn outline_is_closed_and_wide_enough() {
        let s = line(30);
        let o = s.outline();
        assert!(o.len() > 40);
        let ys: Vec<f32> = o.iter().map(|p| p.1).collect();
        let (lo, hi) = ys.iter().fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(*y), b.max(*y)));
        assert!(hi - lo > 1.5 && hi - lo < 3.0, "Breite {}", hi - lo);
    }

    #[test]
    fn dot_stroke() {
        let s = Stroke { pts: vec![Pt::new(5.0, 5.0, 1.0)], ..line(0) };
        assert_eq!(s.outline().len(), 16);
    }

    #[test]
    fn eraser_hits_and_splits() {
        let s = line(51); // 0..100
        assert!(s.hit((50.0, -5.0), (50.0, 5.0), 1.0));
        assert!(!s.hit((50.0, 10.0), (60.0, 10.0), 1.0));
        let parts = s.erase_part((50.0, -5.0), (50.0, 5.0), 2.0).unwrap();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].pts.last().unwrap().x < 49.0);
        assert!(parts[1].pts[0].x > 51.0);
    }

    #[test]
    fn transform_scales_width() {
        let s = line(5).transformed(&Xf::scale_about(2.0, 2.0, 0.0, 0.0));
        assert_eq!(s.width, 4.0);
        assert_eq!(s.pts[1].x, 4.0);
    }
}
