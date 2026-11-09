//! Lineal und Geodreieck: liegen über der Seite (Bildschirmkoordinaten), lassen sich mit
//! Fingern verschieben und drehen; der Stift zeichnet an den Kanten entlang gerade Linien.

use web_sys::CanvasRenderingContext2d;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RulerKind {
    Ruler,
    Triangle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ruler {
    pub kind: RulerKind,
    /// Mittelpunkt (Lineal) bzw. Mitte der langen Kante (Geodreieck), CSS-Pixel
    pub cx: f64,
    pub cy: f64,
    /// Drehung in Radiant
    pub angle: f64,
}

/// Linealmaße (CSS-Pixel)
const LEN: f64 = 860.0;
const WIDTH: f64 = 92.0;
/// Geodreieck: halbe lange Kante = Höhe
const HALF: f64 = 300.0;

pub type Seg = ((f64, f64), (f64, f64));

impl Ruler {
    pub fn new(kind: RulerKind, vw: f64, vh: f64) -> Ruler {
        let cy = if kind == RulerKind::Triangle { vh * 0.62 } else { vh * 0.5 };
        Ruler { kind, cx: vw * 0.5, cy, angle: if kind == RulerKind::Ruler { -0.12 } else { 0.0 } }
    }

    fn rot(&self) -> (f64, f64) {
        let (s, c) = self.angle.sin_cos();
        (s, c)
    }

    /// Bildschirm → lokale Koordinaten (u entlang, v quer).
    pub fn local(&self, x: f64, y: f64) -> (f64, f64) {
        let (s, c) = self.rot();
        let (dx, dy) = (x - self.cx, y - self.cy);
        (dx * c + dy * s, -dx * s + dy * c)
    }

    pub fn screen(&self, u: f64, v: f64) -> (f64, f64) {
        let (s, c) = self.rot();
        (self.cx + u * c - v * s, self.cy + u * s + v * c)
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        let (u, v) = self.local(x, y);
        match self.kind {
            RulerKind::Ruler => u.abs() <= LEN * 0.5 && v.abs() <= WIDTH * 0.5,
            RulerKind::Triangle => (-HALF..=0.0).contains(&v) && u.abs() <= HALF + v,
        }
    }

    /// Kanten in Bildschirmkoordinaten.
    pub fn edges(&self) -> Vec<Seg> {
        let p = |u: f64, v: f64| self.screen(u, v);
        match self.kind {
            RulerKind::Ruler => vec![(p(-LEN * 0.5, -WIDTH * 0.5), p(LEN * 0.5, -WIDTH * 0.5)), (p(-LEN * 0.5, WIDTH * 0.5), p(LEN * 0.5, WIDTH * 0.5))],
            RulerKind::Triangle => vec![(p(-HALF, 0.0), p(HALF, 0.0)), (p(-HALF, 0.0), p(0.0, -HALF)), (p(HALF, 0.0), p(0.0, -HALF))],
        }
    }

    /// Kante, an der ein Strich beginnend bei (x, y) einrasten soll – verschoben um `offset`
    /// (halbe Strichbreite) auf die Seite, auf der der Stift liegt.
    pub fn snap(&self, x: f64, y: f64, max: f64, offset: f64) -> Option<Seg> {
        let mut best: Option<(f64, Seg)> = None;
        for (a, b) in self.edges() {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let len = (dx * dx + dy * dy).sqrt().max(1e-6);
            let t = ((x - a.0) * dx + (y - a.1) * dy) / (len * len);
            if !(-0.05..=1.05).contains(&t) {
                continue;
            }
            // vorzeichenbehafteter Abstand (Normale)
            let (nx, ny) = (-dy / len, dx / len);
            let d = (x - a.0) * nx + (y - a.1) * ny;
            if d.abs() <= max && best.map(|b| d.abs() < b.0).unwrap_or(true) {
                let o = if d >= 0.0 { offset } else { -offset };
                best = Some((d.abs(), ((a.0 + nx * o, a.1 + ny * o), (b.0 + nx * o, b.1 + ny * o))));
            }
        }
        best.map(|b| b.1)
    }

    /// Winkel in Grad (0…180) für die Anzeige.
    /// Winkel zur Waagerechten in Grad (0…90) für die Anzeige.
    pub fn degrees(&self) -> f64 {
        let d = self.angle.to_degrees().rem_euclid(180.0);
        let d = if d > 90.0 { 180.0 - d } else { d };
        d.round().abs()
    }

    /// Drehung auf 0°/45°/90° … einrasten, wenn nahe dran.
    pub fn snap_angle(a: f64) -> f64 {
        let q = std::f64::consts::FRAC_PI_4;
        let s = (a / q).round() * q;
        if (a - s).abs() < 0.035 {
            s
        } else {
            a
        }
    }

    /// Rechteck um das Werkzeug (für den Handballen-Filter der Android-App).
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        let pts: Vec<(f64, f64)> = match self.kind {
            RulerKind::Ruler => vec![self.screen(-LEN * 0.5, -WIDTH * 0.5), self.screen(LEN * 0.5, -WIDTH * 0.5), self.screen(LEN * 0.5, WIDTH * 0.5), self.screen(-LEN * 0.5, WIDTH * 0.5)],
            RulerKind::Triangle => vec![self.screen(-HALF, 0.0), self.screen(HALF, 0.0), self.screen(0.0, -HALF)],
        };
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for (x, y) in pts {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        (x0, y0, x1 - x0, y1 - y0)
    }

    /// Zeichnet das Werkzeug. `mm_px` = Bildschirmpixel pro Millimeter auf der Seite.
    pub fn draw(&self, ctx: &CanvasRenderingContext2d, dpr: f64, mm_px: f64, dark: bool) {
        let (s, c) = self.rot();
        ctx.save();
        ctx.set_transform(dpr * c, dpr * s, -dpr * s, dpr * c, dpr * self.cx, dpr * self.cy).ok();
        let (fill, edge, ink) = if dark {
            ("rgba(40,42,54,0.72)", "rgba(255,255,255,0.35)", "rgba(235,236,245,0.85)")
        } else {
            ("rgba(255,255,255,0.62)", "rgba(30,35,60,0.25)", "rgba(25,28,45,0.8)")
        };
        ctx.set_shadow_color("rgba(20,22,40,0.25)");
        ctx.set_shadow_blur(18.0);
        ctx.set_shadow_offset_y(6.0);
        ctx.begin_path();
        match self.kind {
            RulerKind::Ruler => {
                let _ = ctx.round_rect_with_f64(-LEN * 0.5, -WIDTH * 0.5, LEN, WIDTH, 10.0);
            }
            RulerKind::Triangle => {
                ctx.move_to(-HALF, 0.0);
                ctx.line_to(HALF, 0.0);
                ctx.line_to(0.0, -HALF);
                ctx.close_path();
            }
        }
        ctx.set_fill_style_str(fill);
        ctx.fill();
        ctx.set_shadow_blur(0.0);
        ctx.set_shadow_offset_y(0.0);
        ctx.set_stroke_style_str(edge);
        ctx.set_line_width(1.2);
        ctx.stroke();

        // Skala: Millimeter / halbe / ganze Zentimeter
        let step_mm = if mm_px >= 5.0 { 1.0 } else if mm_px >= 1.6 { 5.0 } else { 10.0 };
        ctx.set_stroke_style_str(ink);
        ctx.set_fill_style_str(ink);
        ctx.set_font("600 11px Inter, system-ui, sans-serif");
        ctx.set_text_align("center");
        let label_every = if mm_px * 10.0 >= 26.0 { 1 } else { 5 };
        let tick = |ctx: &CanvasRenderingContext2d, u: f64, v0: f64, dir: f64, mm: i64| {
            let len = if mm % 10 == 0 { 16.0 } else if mm % 5 == 0 { 11.0 } else { 6.0 };
            ctx.begin_path();
            ctx.move_to(u, v0);
            ctx.line_to(u, v0 + dir * len);
            ctx.set_line_width(if mm % 10 == 0 { 1.3 } else { 0.9 });
            ctx.stroke();
            if mm % 10 == 0 && (mm / 10) % label_every == 0 {
                let _ = ctx.fill_text(&format!("{}", mm.abs() / 10), u, v0 + dir * 28.0 + if dir > 0.0 { 0.0 } else { 9.0 });
            }
        };
        match self.kind {
            RulerKind::Ruler => {
                let start = -LEN * 0.5 + 14.0;
                let mut mm = 0i64;
                loop {
                    let u = start + mm as f64 * mm_px;
                    if u > LEN * 0.5 - 8.0 {
                        break;
                    }
                    tick(ctx, u, -WIDTH * 0.5, 1.0, mm);
                    mm += step_mm as i64;
                }
            }
            RulerKind::Triangle => {
                // Skala entlang der langen Kante, 0 in der Mitte
                let mut mm = 0i64;
                loop {
                    let u = mm as f64 * mm_px;
                    if u > HALF - 20.0 {
                        break;
                    }
                    tick(ctx, u, 0.0, -1.0, mm);
                    if mm != 0 {
                        tick(ctx, -u, 0.0, -1.0, -mm);
                    }
                    mm += step_mm as i64;
                }
                // Winkelmesser
                let r = HALF * 0.62;
                ctx.begin_path();
                let _ = ctx.arc(0.0, 0.0, r, std::f64::consts::PI, std::f64::consts::TAU);
                ctx.set_line_width(1.0);
                ctx.stroke();
                ctx.set_font("600 10px Inter, system-ui, sans-serif");
                for deg in (0..=180).step_by(5) {
                    let a = std::f64::consts::PI + (deg as f64).to_radians();
                    let len = if deg % 10 == 0 { 12.0 } else { 6.0 };
                    let (ca, sa) = (a.cos(), a.sin());
                    ctx.begin_path();
                    ctx.move_to(ca * r, sa * r);
                    ctx.line_to(ca * (r - len), sa * (r - len));
                    ctx.stroke();
                    if deg % 30 == 0 {
                        let _ = ctx.fill_text(&deg.to_string(), ca * (r - 24.0), sa * (r - 24.0) + 4.0);
                    }
                }
                ctx.begin_path();
                let _ = ctx.arc(0.0, 0.0, 3.0, 0.0, std::f64::consts::TAU);
                ctx.fill();
            }
        }
        // Winkelanzeige
        let (lx, ly) = match self.kind {
            RulerKind::Ruler => (0.0, 14.0),
            RulerKind::Triangle => (0.0, -HALF * 0.3),
        };
        let label = format!("{:.0}°", self.degrees());
        ctx.set_font("700 13px Inter, system-ui, sans-serif");
        ctx.set_fill_style_str("#6d5cff");
        ctx.begin_path();
        let _ = ctx.round_rect_with_f64(lx - 26.0, ly - 12.0, 52.0, 24.0, 12.0);
        ctx.fill();
        ctx.set_fill_style_str("#ffffff");
        let _ = ctx.fill_text(&label, lx, ly + 4.5);
        ctx.set_text_align("start");
        ctx.restore();
    }
}

/// Punkt auf die Gerade durch `seg` projizieren.
pub fn project(seg: &Seg, x: f64, y: f64) -> (f64, f64) {
    let ((ax, ay), (bx, by)) = *seg;
    let (dx, dy) = (bx - ax, by - ay);
    let l2 = (dx * dx + dy * dy).max(1e-9);
    let t = ((x - ax) * dx + (y - ay) * dy) / l2;
    (ax + dx * t, ay + dy * t)
}
