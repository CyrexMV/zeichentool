//! Formerkennung (QuickShape): Linie, Winkel, Dreieck, Rechteck, Viereck, Kreis, Ellipse.

use super::geom::{dist, rdp, Rect};
use super::ink::Pt;

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Line,
    Polyline,
    Triangle,
    Rectangle,
    Polygon,
    Circle,
    Ellipse,
}

impl Shape {
    pub fn name_de(&self) -> &'static str {
        match self {
            Shape::Line => "Linie",
            Shape::Polyline => "Linienzug",
            Shape::Triangle => "Dreieck",
            Shape::Rectangle => "Rechteck",
            Shape::Polygon => "Vieleck",
            Shape::Circle => "Kreis",
            Shape::Ellipse => "Ellipse",
        }
    }
}

fn densify(pts: &[(f32, f32)], closed: bool, step: f32, p: f32) -> Vec<Pt> {
    let mut out = Vec::new();
    let n = pts.len();
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs {
        let (ax, ay) = pts[i];
        let (bx, by) = pts[(i + 1) % n];
        let k = ((dist(ax, ay, bx, by) / step).ceil() as usize).max(1);
        for j in 0..k {
            let t = j as f32 / k as f32;
            out.push(Pt::new(ax + (bx - ax) * t, ay + (by - ay) * t, p));
        }
    }
    let last = if closed { pts[0] } else { pts[n - 1] };
    out.push(Pt::new(last.0, last.1, p));
    if closed && out.len() > 2 {
        // ein Stück überlappen, damit keine Naht sichtbar ist
        let second = out[1];
        out.push(second);
    }
    out
}

/// Erkennt eine Form in einem Freihandstrich und liefert die neuen, sauberen Punkte.
pub fn recognize(raw: &[Pt]) -> Option<(Shape, Vec<Pt>)> {
    if raw.len() < 4 {
        return None;
    }
    let xy: Vec<(f32, f32)> = raw.iter().map(|p| (p.x, p.y)).collect();
    let mut bb = Rect::EMPTY;
    for &(x, y) in &xy {
        bb.add(x, y);
    }
    let diag = dist(bb.x0, bb.y0, bb.x1, bb.y1);
    if diag < 8.0 {
        return None;
    }
    let len: f32 = xy.windows(2).map(|w| dist(w[0].0, w[0].1, w[1].0, w[1].1)).sum();
    let p = (raw.iter().map(|q| q.p).sum::<f32>() / raw.len() as f32).clamp(0.3, 1.0);
    let step = (diag / 120.0).max(0.8);
    let (sx, sy) = xy[0];
    let (ex, ey) = xy[xy.len() - 1];
    let closed = dist(sx, sy, ex, ey) < diag * 0.22 && len > diag * 1.6;

    if !closed {
        let keep = rdp(&xy, diag * 0.06);
        let pts: Vec<(f32, f32)> = keep.iter().map(|&i| xy[i]).collect();
        return match pts.len() {
            2 => Some((Shape::Line, densify(&pts, false, step, p))),
            3 | 4 => Some((Shape::Polyline, densify(&pts, false, step, p))),
            _ => None,
        };
    }

    // Ellipsen-Passung über das Begrenzungsrechteck
    let (cx, cy) = bb.center();
    let (rx, ry) = (bb.w() * 0.5, bb.h() * 0.5);
    let err: f32 = xy
        .iter()
        .map(|&(x, y)| {
            let dx = (x - cx) / rx.max(1e-3);
            let dy = (y - cy) / ry.max(1e-3);
            ((dx * dx + dy * dy).sqrt() - 1.0).abs()
        })
        .sum::<f32>()
        / xy.len() as f32;

    // Ecken über RDP auf dem geschlossenen Pfad (ohne doppelten Endpunkt)
    let mut path = xy.clone();
    path.push(xy[0]);
    let keep = rdp(&path, diag * 0.075);
    let mut corners: Vec<(f32, f32)> = keep.iter().map(|&i| path[i]).collect();
    corners.pop();
    // nahe beieinanderliegende Ecken zusammenfassen
    corners.dedup_by(|a, b| dist(a.0, a.1, b.0, b.1) < diag * 0.08);

    if err < 0.09 || corners.len() > 6 {
        if err > 0.2 {
            return None;
        }
        if (rx - ry).abs() < rx.max(ry) * 0.15 {
            let r = (rx + ry) * 0.5;
            let pts = super::ink::circle(cx, cy, r, 96);
            return Some((Shape::Circle, densify(&pts, true, step, p)));
        }
        let n = 96;
        let pts: Vec<(f32, f32)> = (0..n)
            .map(|i| {
                let t = std::f32::consts::TAU * i as f32 / n as f32;
                (cx + rx * t.cos(), cy + ry * t.sin())
            })
            .collect();
        return Some((Shape::Ellipse, densify(&pts, true, step, p)));
    }
    match corners.len() {
        3 => Some((Shape::Triangle, densify(&corners, true, step, p))),
        4 => {
            // fast achsenparallel → sauberes Rechteck
            let axis = corners.iter().zip(corners.iter().cycle().skip(1)).all(|(a, b)| {
                let (dx, dy) = ((b.0 - a.0).abs(), (b.1 - a.1).abs());
                dx.min(dy) < dx.max(dy) * 0.25
            });
            if axis {
                let r = [(bb.x0, bb.y0), (bb.x1, bb.y0), (bb.x1, bb.y1), (bb.x0, bb.y1)];
                Some((Shape::Rectangle, densify(&r, true, step, p)))
            } else {
                Some((Shape::Polygon, densify(&corners, true, step, p)))
            }
        }
        5 | 6 => Some((Shape::Polygon, densify(&corners, true, step, p))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(v: &[(f32, f32)]) -> Vec<Pt> {
        v.iter().map(|&(x, y)| Pt::new(x, y, 0.7)).collect()
    }

    fn path(corners: &[(f32, f32)], closed: bool) -> Vec<Pt> {
        let mut v = Vec::new();
        let n = corners.len();
        let segs = if closed { n } else { n - 1 };
        for i in 0..segs {
            let a = corners[i];
            let b = corners[(i + 1) % n];
            for k in 0..20 {
                let t = k as f32 / 20.0;
                // leichtes Zittern
                let j = ((i * 20 + k) as f32 * 1.7).sin() * 0.6;
                v.push((a.0 + (b.0 - a.0) * t + j, a.1 + (b.1 - a.1) * t - j));
            }
        }
        if closed {
            v.push(corners[0]);
        } else {
            v.push(corners[n - 1]);
        }
        pts(&v)
    }

    #[test]
    fn recognizes_basic_shapes() {
        assert_eq!(recognize(&path(&[(0.0, 0.0), (100.0, 40.0)], false)).unwrap().0, Shape::Line);
        assert_eq!(recognize(&path(&[(0.0, 0.0), (100.0, 0.0), (50.0, 80.0)], true)).unwrap().0, Shape::Triangle);
        assert_eq!(recognize(&path(&[(0.0, 0.0), (120.0, 0.0), (120.0, 80.0), (0.0, 80.0)], true)).unwrap().0, Shape::Rectangle);
        let circle: Vec<(f32, f32)> = (0..=80)
            .map(|i| {
                let t = std::f32::consts::TAU * i as f32 / 80.0;
                (50.0 + 40.0 * t.cos(), 50.0 + 41.0 * t.sin())
            })
            .collect();
        assert_eq!(recognize(&pts(&circle)).unwrap().0, Shape::Circle);
    }
}
