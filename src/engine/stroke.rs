use super::brush::{BrushPreset, Dab};

/// Ein Eingabepunkt in Dokumentkoordinaten.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputPoint {
    pub x: f32,
    pub y: f32,
    /// Stiftdruck 0..1 (Maus/Finger: 1)
    pub pressure: f32,
    /// S-Pen-Neigung in Grad (-90..90)
    pub tilt_x: f32,
    pub tilt_y: f32,
}

impl InputPoint {
    pub fn new(x: f32, y: f32, pressure: f32) -> Self {
        InputPoint { x, y, pressure, tilt_x: 0.0, tilt_y: 0.0 }
    }

    fn lerp(&self, o: &InputPoint, t: f32) -> InputPoint {
        InputPoint {
            x: self.x + (o.x - self.x) * t,
            y: self.y + (o.y - self.y) * t,
            pressure: self.pressure + (o.pressure - self.pressure) * t,
            tilt_x: self.tilt_x + (o.tilt_x - self.tilt_x) * t,
            tilt_y: self.tilt_y + (o.tilt_y - self.tilt_y) * t,
        }
    }

    pub fn dist(&self, o: &InputPoint) -> f32 {
        ((o.x - self.x).powi(2) + (o.y - self.y).powi(2)).sqrt()
    }
}

/// Symmetriemodus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symmetry {
    Off,
    Vertical,
    Horizontal,
    Quad,
    Radial(u8),
}

impl Symmetry {
    pub fn code(&self) -> String {
        match self {
            Symmetry::Off => "off".into(),
            Symmetry::Vertical => "vertical".into(),
            Symmetry::Horizontal => "horizontal".into(),
            Symmetry::Quad => "quad".into(),
            Symmetry::Radial(n) => format!("radial{}", n),
        }
    }

    pub fn from_code(s: &str) -> Symmetry {
        match s {
            "vertical" => Symmetry::Vertical,
            "horizontal" => Symmetry::Horizontal,
            "quad" => Symmetry::Quad,
            _ if s.starts_with("radial") => {
                Symmetry::Radial(s[6..].parse::<u8>().unwrap_or(6).clamp(2, 32))
            }
            _ => Symmetry::Off,
        }
    }

    /// Affine Abbildungen (inkl. Identität) für alle Symmetriekopien um `(cx, cy)`.
    pub fn transforms(&self, cx: f32, cy: f32) -> Vec<Xf> {
        let id = Xf::IDENTITY;
        let mx = Xf { a: -1.0, b: 0.0, c: 0.0, d: 1.0, e: 2.0 * cx, f: 0.0 };
        let my = Xf { a: 1.0, b: 0.0, c: 0.0, d: -1.0, e: 0.0, f: 2.0 * cy };
        match *self {
            Symmetry::Off => vec![id],
            Symmetry::Vertical => vec![id, mx],
            Symmetry::Horizontal => vec![id, my],
            Symmetry::Quad => vec![id, mx, my, mx.then(&my)],
            Symmetry::Radial(n) => (0..n.max(1))
                .map(|k| Xf::rotation_about(std::f32::consts::TAU * k as f32 / n as f32, cx, cy))
                .collect(),
        }
    }
}

/// 2D-Affintransformation: x' = a·x + c·y + e, y' = b·x + d·y + f
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

    pub fn rotation_about(theta: f32, cx: f32, cy: f32) -> Xf {
        let (s, c) = theta.sin_cos();
        Xf { a: c, b: s, c: -s, d: c, e: cx - c * cx + s * cy, f: cy - s * cx - c * cy }
    }

    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }

    pub fn apply_vec(&self, x: f32, y: f32) -> (f32, f32) {
        (self.a * x + self.c * y, self.b * x + self.d * y)
    }

    /// Zuerst `self`, dann `o`.
    pub fn then(&self, o: &Xf) -> Xf {
        Xf {
            a: o.a * self.a + o.c * self.b,
            b: o.b * self.a + o.d * self.b,
            c: o.a * self.c + o.c * self.d,
            d: o.b * self.c + o.d * self.d,
            e: o.a * self.e + o.c * self.f + o.e,
            f: o.b * self.e + o.d * self.f + o.f,
        }
    }
}

/// Wandelt Eingabepunkte in gleichmäßig verteilte Pinselstempel um –
/// mit Glättung (Stabilisator), Druckkurve, Neigung und Symmetrie.
pub struct StrokeEngine {
    pub preset: BrushPreset,
    /// Durchmesser in Dokumentpixeln
    pub size: f32,
    pub pressure_gamma: f32,
    pub smoothing: f32,
    xfs: Vec<Xf>,
    smoothed: Option<InputPoint>,
    prev: Option<InputPoint>,
    last_raw: Option<InputPoint>,
    carry: f32,
    rng: u32,
    pub raw: Vec<InputPoint>,
    pub path_len: f32,
}

impl StrokeEngine {
    pub fn new(preset: BrushPreset, size: f32, pressure_gamma: f32, smoothing: f32, sym: Symmetry, cx: f32, cy: f32) -> Self {
        StrokeEngine {
            preset,
            size: size.max(0.5),
            pressure_gamma: pressure_gamma.clamp(0.2, 4.0),
            smoothing: smoothing.clamp(0.0, 0.98),
            xfs: sym.transforms(cx, cy),
            smoothed: None,
            prev: None,
            last_raw: None,
            carry: 0.0,
            rng: 0x1234_5678,
            raw: Vec::new(),
            path_len: 0.0,
        }
    }

    pub fn symmetry_count(&self) -> usize {
        self.xfs.len()
    }

    fn rand(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x & 0xffffff) as f32 / 16_777_215.0
    }

    fn pressure_curve(&self, p: f32) -> f32 {
        p.clamp(0.0, 1.0).powf(self.pressure_gamma)
    }

    fn radius_at(&self, pressure: f32) -> f32 {
        let pr = self.pressure_curve(pressure);
        let p = &self.preset;
        let target = p.min_size + (1.0 - p.min_size) * pr;
        let f = 1.0 + (target - 1.0) * p.pressure_size;
        (self.size * 0.5 * f).max(0.25)
    }

    fn spacing_at(&self, pressure: f32) -> f32 {
        let r = self.radius_at(pressure);
        (2.0 * r * self.preset.spacing).max(if self.preset.pixel { 0.5 } else { 0.3 })
    }

    pub fn begin(&mut self, p: InputPoint, out: &mut Vec<Dab>) {
        self.smoothed = Some(p);
        self.prev = Some(p);
        self.last_raw = Some(p);
        self.carry = 0.0;
        self.raw.clear();
        self.raw.push(p);
        self.path_len = 0.0;
        self.emit(p, out);
    }

    pub fn add(&mut self, p: InputPoint, out: &mut Vec<Dab>) {
        if let Some(lr) = self.last_raw {
            self.path_len += lr.dist(&p);
        }
        self.last_raw = Some(p);
        self.raw.push(p);
        let s = match self.smoothed {
            Some(s) => s,
            None => return self.begin(p, out),
        };
        // Exponentielle Glättung: je höher die Stabilisierung, desto ruhiger die Linie.
        let k = (1.0 - self.smoothing).powf(1.6).max(0.02);
        let kp = k.max(0.35);
        let n = InputPoint {
            x: s.x + (p.x - s.x) * k,
            y: s.y + (p.y - s.y) * k,
            pressure: s.pressure + (p.pressure - s.pressure) * kp,
            tilt_x: s.tilt_x + (p.tilt_x - s.tilt_x) * kp,
            tilt_y: s.tilt_y + (p.tilt_y - s.tilt_y) * kp,
        };
        self.smoothed = Some(n);
        self.segment(n, out);
    }

    /// Holt die Glättung zum letzten echten Punkt auf, damit Striche nicht zu kurz enden.
    pub fn end(&mut self, out: &mut Vec<Dab>) {
        if let (Some(s), Some(r)) = (self.smoothed, self.last_raw) {
            if s.dist(&r) > 0.5 {
                let mut t = r;
                t.pressure = s.pressure;
                self.segment(t, out);
            }
        }
    }

    /// Stempel entlang eines Pfades mit konstantem Druck (für Formen & QuickShape).
    pub fn path(&mut self, pts: &[(f32, f32)], closed: bool, pressure: f32, out: &mut Vec<Dab>) {
        if pts.is_empty() {
            return;
        }
        let mk = |p: &(f32, f32)| InputPoint::new(p.0, p.1, pressure);
        let first = mk(&pts[0]);
        self.prev = Some(first);
        self.carry = 0.0;
        self.emit(first, out);
        for p in &pts[1..] {
            self.segment(mk(p), out);
        }
        if closed && pts.len() > 2 {
            self.segment(first, out);
        }
    }

    fn segment(&mut self, to: InputPoint, out: &mut Vec<Dab>) {
        let from = match self.prev {
            Some(p) => p,
            None => {
                self.prev = Some(to);
                return;
            }
        };
        let len = from.dist(&to);
        if len < 1e-4 {
            self.prev = Some(InputPoint { x: from.x, y: from.y, ..to });
            return;
        }
        let mut travelled = 0.0;
        loop {
            let here = from.lerp(&to, (travelled / len).min(1.0));
            let spacing = self.spacing_at(here.pressure);
            let need = spacing - self.carry;
            if travelled + need > len {
                self.carry += len - travelled;
                break;
            }
            travelled += need;
            self.carry = 0.0;
            let q = from.lerp(&to, travelled / len);
            self.emit(q, out);
        }
        self.prev = Some(to);
    }

    fn emit(&mut self, p: InputPoint, out: &mut Vec<Dab>) {
        let preset = self.preset.clone();
        let pr = self.pressure_curve(p.pressure);
        let opac = 1.0 + (pr - 1.0) * preset.pressure_opacity;
        // Neigung: 0 = senkrecht, 1 = flach
        let tilt = ((p.tilt_x * p.tilt_x + p.tilt_y * p.tilt_y).sqrt() / 90.0).clamp(0.0, 1.0);
        let ts = preset.tilt_shading * tilt;
        let mut radius = self.radius_at(p.pressure) * (1.0 + ts * 1.8);
        let alpha = preset.flow * opac * (1.0 - ts * 0.55);
        let mut aspect = preset.aspect * (1.0 - ts * 0.55);
        let mut hardness = preset.hardness * (1.0 - ts * 0.6);
        let angle = if ts > 0.05 { p.tilt_y.atan2(p.tilt_x) + std::f32::consts::FRAC_PI_2 } else { preset.angle };

        let particles = preset.jitter > 1.0;
        let count = if particles { 6 } else { 1 };
        let scatter = if particles { radius * preset.jitter * 0.5 } else { radius * preset.jitter };
        if particles {
            radius = (radius * 0.12).max(0.6);
            aspect = 1.0;
            hardness = preset.hardness;
        }
        for _ in 0..count {
            let (mut x, mut y) = (p.x, p.y);
            if scatter > 0.0 {
                let a = self.rand() * std::f32::consts::TAU;
                let r = if particles { self.rand().sqrt() } else { self.rand() } * scatter;
                x += a.cos() * r;
                y += a.sin() * r;
            }
            let rr = if particles { radius * (0.5 + self.rand()) } else { radius };
            for (i, xf) in self.xfs.iter().enumerate() {
                let (tx, ty) = xf.apply(x, y);
                let (vx, vy) = xf.apply_vec(angle.cos(), angle.sin());
                out.push(Dab {
                    x: tx,
                    y: ty,
                    radius: rr,
                    alpha,
                    hardness,
                    aspect,
                    angle: vy.atan2(vx),
                    grain: preset.grain,
                    pixel: preset.pixel,
                    sym: i as u8,
                });
            }
        }
    }

    /// Transformiert ein Polygon in alle Symmetriekopien.
    pub fn symmetric_polygons(&self, pts: &[(f32, f32)]) -> Vec<Vec<(f32, f32)>> {
        self.xfs
            .iter()
            .map(|xf| pts.iter().map(|&(x, y)| xf.apply(x, y)).collect())
            .collect()
    }
}

/// Ergebnis der QuickShape-Erkennung.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum QuickShape {
    Line { x0: f32, y0: f32 },
    Ellipse { cx: f32, cy: f32, rx: f32, ry: f32 },
}

/// Erkennt, ob ein gehaltener Strich eine Linie oder eine Ellipse/ein Kreis werden soll.
pub fn detect_quickshape(raw: &[InputPoint]) -> Option<QuickShape> {
    if raw.len() < 3 {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in raw {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    let (bw, bh) = (x1 - x0, y1 - y0);
    let span = bw.max(bh);
    if span < 12.0 {
        return None;
    }
    let first = raw[0];
    let last = raw[raw.len() - 1];
    let mut len = 0.0;
    for w in raw.windows(2) {
        len += w[0].dist(&w[1]);
    }
    let closed = first.dist(&last) < span * 0.3 && len > (bw + bh) * 1.2;
    if closed {
        let (cx, cy) = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
        let (mut rx, mut ry) = (bw * 0.5, bh * 0.5);
        // fast gleich groß → perfekter Kreis
        if (rx - ry).abs() < rx.max(ry) * 0.15 {
            let r = (rx + ry) * 0.5;
            rx = r;
            ry = r;
        }
        Some(QuickShape::Ellipse { cx, cy, rx, ry })
    } else {
        Some(QuickShape::Line { x0: first.x, y0: first.y })
    }
}

/// Punkte einer Ellipse als Polygon.
pub fn ellipse_points(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<(f32, f32)> {
    let circ = std::f32::consts::TAU * ((rx * rx + ry * ry) * 0.5).sqrt();
    let n = (circ / 3.0).clamp(24.0, 720.0) as usize;
    (0..n)
        .map(|i| {
            let t = std::f32::consts::TAU * i as f32 / n as f32;
            (cx + rx * t.cos(), cy + ry * t.sin())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::brush::presets;

    #[test]
    fn evenly_spaced_dabs() {
        let p = presets()[0].clone();
        let mut e = StrokeEngine::new(p, 10.0, 1.0, 0.0, Symmetry::Off, 0.0, 0.0);
        let mut out = Vec::new();
        e.begin(InputPoint::new(0.0, 0.0, 1.0), &mut out);
        e.add(InputPoint::new(100.0, 0.0, 1.0), &mut out);
        assert!(out.len() > 50, "zu wenige Stempel: {}", out.len());
        assert!(out.iter().all(|d| d.y.abs() < 1e-3));
    }

    #[test]
    fn symmetry_duplicates_dabs() {
        let p = presets()[0].clone();
        let mut e = StrokeEngine::new(p, 10.0, 1.0, 0.0, Symmetry::Quad, 50.0, 50.0);
        let mut out = Vec::new();
        e.begin(InputPoint::new(10.0, 20.0, 1.0), &mut out);
        assert_eq!(out.len(), 4);
        assert!((out[1].x - 90.0).abs() < 1e-3);
        assert!((out[2].y - 80.0).abs() < 1e-3);
        let mut e = StrokeEngine::new(presets()[0].clone(), 10.0, 1.0, 0.0, Symmetry::Radial(6), 50.0, 50.0);
        out.clear();
        e.begin(InputPoint::new(10.0, 20.0, 1.0), &mut out);
        assert_eq!(out.len(), 6);
    }

    #[test]
    fn quickshape_detects_line_and_circle() {
        let line: Vec<_> = (0..50).map(|i| InputPoint::new(i as f32 * 4.0, i as f32 * 2.0, 1.0)).collect();
        assert!(matches!(detect_quickshape(&line), Some(QuickShape::Line { .. })));
        let circle: Vec<_> = (0..=60)
            .map(|i| {
                let t = std::f32::consts::TAU * i as f32 / 60.0;
                InputPoint::new(100.0 + 50.0 * t.cos(), 100.0 + 52.0 * t.sin(), 1.0)
            })
            .collect();
        match detect_quickshape(&circle) {
            Some(QuickShape::Ellipse { rx, ry, .. }) => assert_eq!(rx, ry),
            other => panic!("unerwartet: {:?}", other),
        }
    }
}
