//! Funktionsplotter (Formelparser + Koordinatensystem) und Tabellen als Vektorelemente.

use super::ink::{PenKind, Pt, Stroke};
use super::note::{Content, TextBox};

// ---------------------------------------------------------------------------
// Formeln
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(f64),
    X,
    Neg(Box<Expr>),
    Bin(char, Box<Expr>, Box<Expr>),
    Call(String, Box<Expr>),
}

impl Expr {
    pub fn eval(&self, x: f64) -> f64 {
        match self {
            Expr::Num(v) => *v,
            Expr::X => x,
            Expr::Neg(a) => -a.eval(x),
            Expr::Bin(op, a, b) => {
                let (a, b) = (a.eval(x), b.eval(x));
                match op {
                    '+' => a + b,
                    '-' => a - b,
                    '*' => a * b,
                    '/' => a / b,
                    _ => a.powf(b),
                }
            }
            Expr::Call(f, a) => {
                let v = a.eval(x);
                match f.as_str() {
                    "sin" => v.sin(),
                    "cos" => v.cos(),
                    "tan" => v.tan(),
                    "sqrt" | "wurzel" => v.sqrt(),
                    "abs" | "betrag" => v.abs(),
                    "ln" => v.ln(),
                    "log" | "lg" => v.log10(),
                    "exp" => v.exp(),
                    "arcsin" | "asin" => v.asin(),
                    "arccos" | "acos" => v.acos(),
                    "arctan" | "atan" => v.atan(),
                    _ => f64::NAN,
                }
            }
        }
    }
}

const FUNCS: [&str; 15] = ["sin", "cos", "tan", "sqrt", "wurzel", "abs", "betrag", "ln", "log", "lg", "exp", "arcsin", "asin", "arccos", "acos"];

struct Parser {
    t: Vec<char>,
    p: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.t.get(self.p).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.p += 1;
            true
        } else {
            false
        }
    }

    // Summe := Produkt (('+'|'-') Produkt)*
    fn sum(&mut self) -> Result<Expr, String> {
        let mut a = self.product()?;
        loop {
            if self.eat('+') {
                a = Expr::Bin('+', Box::new(a), Box::new(self.product()?));
            } else if self.eat('-') {
                a = Expr::Bin('-', Box::new(a), Box::new(self.product()?));
            } else {
                return Ok(a);
            }
        }
    }

    // Produkt := Potenz (('*'|'/'|implizit) Potenz)*
    fn product(&mut self) -> Result<Expr, String> {
        let mut a = self.unary()?;
        loop {
            if self.eat('*') {
                a = Expr::Bin('*', Box::new(a), Box::new(self.unary()?));
            } else if self.eat('/') {
                a = Expr::Bin('/', Box::new(a), Box::new(self.unary()?));
            } else if matches!(self.peek(), Some(c) if c == '(' || c.is_ascii_alphabetic() || c == 'π' || c.is_ascii_digit()) {
                // implizite Multiplikation: 2x, 3(x+1), x sin(x)
                a = Expr::Bin('*', Box::new(a), Box::new(self.power()?));
            } else {
                return Ok(a);
            }
        }
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.eat('-') {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        if self.eat('+') {
            return self.unary();
        }
        self.power()
    }

    // Potenz := Atom ('^' Unär)?   (rechtsassoziativ)
    fn power(&mut self) -> Result<Expr, String> {
        let a = self.atom()?;
        if self.eat('^') {
            let b = self.unary()?;
            return Ok(Expr::Bin('^', Box::new(a), Box::new(b)));
        }
        Ok(a)
    }

    fn atom(&mut self) -> Result<Expr, String> {
        let Some(c) = self.peek() else { return Err("Formel ist unvollständig".into()) };
        if c == '(' {
            self.p += 1;
            let e = self.sum()?;
            if !self.eat(')') {
                return Err("Klammer „)“ fehlt".into());
            }
            return Ok(e);
        }
        if c.is_ascii_digit() || c == '.' {
            let start = self.p;
            while matches!(self.peek(), Some(d) if d.is_ascii_digit() || d == '.') {
                self.p += 1;
            }
            let s: String = self.t[start..self.p].iter().collect();
            return s.parse::<f64>().map(Expr::Num).map_err(|_| format!("Zahl „{}“ verstehe ich nicht", s));
        }
        if c == 'π' {
            self.p += 1;
            return Ok(Expr::Num(std::f64::consts::PI));
        }
        if c.is_ascii_alphabetic() {
            let start = self.p;
            while matches!(self.peek(), Some(d) if d.is_ascii_alphabetic()) {
                self.p += 1;
            }
            let word: String = self.t[start..self.p].iter().collect::<String>().to_lowercase();
            // längsten bekannten Namen am Wortanfang nehmen (z. B. „sinx“ → sin · x, „xx“ → x · x)
            let known = FUNCS.iter().chain(["arctan", "atan", "pi", "x", "e"].iter());
            let Some(name) = known.filter(|n| word.starts_with(**n)).max_by_key(|n| n.len()) else {
                return Err(format!("„{}“ kenne ich nicht", word));
            };
            self.p = start + name.chars().count();
            match *name {
                "x" => return Ok(Expr::X),
                "pi" => return Ok(Expr::Num(std::f64::consts::PI)),
                "e" => return Ok(Expr::Num(std::f64::consts::E)),
                f => {
                    let arg = if self.peek() == Some('(') { self.atom()? } else { self.power()? };
                    return Ok(Expr::Call(f.to_string(), Box::new(arg)));
                }
            }
        }
        Err(format!("Unerwartetes Zeichen „{}“", c))
    }
}

/// Formel wie „x^2 - 2“, „2sin(x)“, „f(x) = 1/x“ einlesen.
pub fn parse(src: &str) -> Result<Expr, String> {
    let mut s = src.trim().to_string();
    if let Some((l, r)) = s.split_once('=') {
        if l.contains('x') || l.trim().len() <= 4 {
            s = r.to_string();
        }
    }
    let t: Vec<char> = s.replace(',', ".").replace(['·', '×'], "*").replace('÷', "/").replace('²', "^2").replace('³', "^3").replace('√', "sqrt").chars().filter(|c| !c.is_whitespace()).collect();
    if t.is_empty() {
        return Err("Bitte eine Funktion eingeben".into());
    }
    let mut p = Parser { t, p: 0 };
    let e = p.sum()?;
    if p.p < p.t.len() {
        return Err(format!("Unerwartetes Zeichen „{}“", p.t[p.p]));
    }
    Ok(e)
}

// ---------------------------------------------------------------------------
// Koordinatensystem
// ---------------------------------------------------------------------------

pub struct PlotSpec {
    pub funcs: Vec<Expr>,
    pub xmin: f64,
    pub xmax: f64,
    /// `None` = automatisch
    pub y: Option<(f64, f64)>,
    pub grid: bool,
}

/// Linien einer Funktion (getrennt an Sprungstellen) in Datenkoordinaten.
pub fn sample(f: &Expr, xmin: f64, xmax: f64, ymin: f64, ymax: f64, n: usize) -> Vec<Vec<(f64, f64)>> {
    let mut out = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    let span = ymax - ymin;
    for i in 0..=n {
        let x = xmin + (xmax - xmin) * i as f64 / n as f64;
        let y = f.eval(x);
        let ok = y.is_finite() && y > ymin - span * 2.0 && y < ymax + span * 2.0;
        let jump = cur.last().map(|&(_, py)| (y - py).abs() > span * 1.5).unwrap_or(false);
        if !ok || jump {
            if cur.len() > 1 {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
            if !ok {
                continue;
            }
        }
        cur.push((x, y.clamp(ymin - span, ymax + span)));
    }
    if cur.len() > 1 {
        out.push(cur);
    }
    out
}

/// Automatischer y-Bereich (robust gegen Polstellen).
pub fn auto_y(funcs: &[Expr], xmin: f64, xmax: f64) -> (f64, f64) {
    let mut v: Vec<f64> = Vec::new();
    for f in funcs {
        for i in 0..=200 {
            let y = f.eval(xmin + (xmax - xmin) * i as f64 / 200.0);
            if y.is_finite() {
                v.push(y);
            }
        }
    }
    if v.is_empty() {
        return (-5.0, 5.0);
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let lo = v[v.len() * 5 / 100];
    let hi = v[(v.len() * 95 / 100).min(v.len() - 1)];
    let (mut lo, mut hi) = (lo.min(0.0), hi.max(0.0));
    if hi - lo < 1e-6 {
        lo -= 1.0;
        hi += 1.0;
    }
    let pad = (hi - lo) * 0.1;
    (lo - pad, hi + pad)
}

/// „Schöne“ Teilung (1, 2, 5 · 10^k) für etwa `target` Abschnitte.
pub fn nice_step(span: f64, target: f64) -> f64 {
    let raw = span / target;
    let mag = 10f64.powf(raw.log10().floor());
    let r = raw / mag;
    mag * if r < 1.5 {
        1.0
    } else if r < 3.5 {
        2.0
    } else if r < 7.5 {
        5.0
    } else {
        10.0
    }
}

fn fmt_num(v: f64) -> String {
    let s = if v.abs() >= 1000.0 || (v.fract().abs() < 1e-9) { format!("{:.0}", v) } else { format!("{:.2}", v).trim_end_matches('0').trim_end_matches('.').to_string() };
    s.replace('.', ",").replace("-0", "0").replace('-', "−")
}

fn line(pts: &[(f32, f32)], color: [u8; 4], width: f32) -> Content {
    Content::Stroke(Stroke { pen: PenKind::Ball, color, width, pts: pts.iter().map(|&(x, y)| Pt::new(x, y, 0.5)).collect() })
}

/// Kurvenfarben
pub const PLOT_COLORS: [[u8; 4]; 5] = [[30, 90, 230, 255], [225, 45, 60, 255], [31, 178, 110, 255], [240, 152, 25, 255], [142, 92, 255, 255]];

/// Koordinatensystem mit Funktionen als Elemente in einem Kasten (x0, y0, w, h) auf der Seite.
pub fn plot_items(spec: &PlotSpec, x0: f32, y0: f32, w: f32, h: f32, ink: [u8; 4]) -> Vec<Content> {
    let (xmin, xmax) = (spec.xmin.min(spec.xmax - 1e-6), spec.xmax);
    let (ymin, ymax) = spec.y.unwrap_or_else(|| auto_y(&spec.funcs, xmin, xmax));
    let sx = |x: f64| x0 + ((x - xmin) / (xmax - xmin)) as f32 * w;
    let sy = |y: f64| y0 + h - ((y - ymin) / (ymax - ymin)) as f32 * h;
    let mut out = Vec::new();
    let grid_c = [ink[0], ink[1], ink[2], 40];
    let (stx, sty) = (nice_step(xmax - xmin, (w / 40.0) as f64), nice_step(ymax - ymin, (h / 40.0) as f64));
    let (ax, ay) = (sx(0.0f64.clamp(xmin, xmax)), sy(0.0f64.clamp(ymin, ymax)));
    let fs = (w.min(h) / 32.0).clamp(6.0, 11.0);
    // Gitter + Beschriftung
    let mut k = (xmin / stx).ceil() as i64;
    while (k as f64) * stx <= xmax + 1e-9 {
        let v = k as f64 * stx;
        let x = sx(v);
        if spec.grid {
            out.push(line(&[(x, y0), (x, y0 + h)], grid_c, 0.5));
        }
        out.push(line(&[(x, ay - 2.5), (x, ay + 2.5)], ink, 0.8));
        if v.abs() > 1e-9 {
            let t = fmt_num(v);
            out.push(Content::Text(TextBox { x: x - fs * 0.3 * t.chars().count() as f32, y: (ay + 4.0).min(y0 + h - fs * 1.3), w: fs * 4.0, size: fs, color: [ink[0], ink[1], ink[2], 200], bold: false, text: t }));
        }
        k += 1;
    }
    let mut k = (ymin / sty).ceil() as i64;
    while (k as f64) * sty <= ymax + 1e-9 {
        let v = k as f64 * sty;
        let y = sy(v);
        if spec.grid {
            out.push(line(&[(x0, y), (x0 + w, y)], grid_c, 0.5));
        }
        out.push(line(&[(ax - 2.5, y), (ax + 2.5, y)], ink, 0.8));
        if v.abs() > 1e-9 {
            let t = fmt_num(v);
            let tx = if ax - fs * 0.6 * t.chars().count() as f32 - 5.0 < x0 { ax + 5.0 } else { ax - fs * 0.6 * t.chars().count() as f32 - 5.0 };
            out.push(Content::Text(TextBox { x: tx, y: y - fs * 0.65, w: fs * 4.0, size: fs, color: [ink[0], ink[1], ink[2], 200], bold: false, text: t }));
        }
        k += 1;
    }
    // Achsen mit Pfeilen
    let a = 6.0;
    out.push(line(&[(x0, ay), (x0 + w, ay)], ink, 1.1));
    out.push(line(&[(x0 + w - a, ay - a * 0.6), (x0 + w, ay), (x0 + w - a, ay + a * 0.6)], ink, 1.1));
    out.push(line(&[(ax, y0 + h), (ax, y0)], ink, 1.1));
    out.push(line(&[(ax - a * 0.6, y0 + a), (ax, y0), (ax + a * 0.6, y0 + a)], ink, 1.1));
    out.push(Content::Text(TextBox { x: x0 + w - fs, y: ay - fs * 1.9, w: fs * 2.0, size: fs * 1.1, color: ink, bold: true, text: "x".into() }));
    out.push(Content::Text(TextBox { x: ax + 6.0, y: y0 - fs * 0.2, w: fs * 2.0, size: fs * 1.1, color: ink, bold: true, text: "y".into() }));
    // Kurven
    for (i, f) in spec.funcs.iter().enumerate() {
        let color = PLOT_COLORS[i % PLOT_COLORS.len()];
        for seg in sample(f, xmin, xmax, ymin, ymax, (w as usize * 2).clamp(200, 1200)) {
            for part in clip_y(&seg, ymin, ymax) {
                let pts: Vec<(f32, f32)> = part.iter().map(|&(x, y)| (sx(x), sy(y))).collect();
                out.push(line(&pts, color, 1.7));
            }
        }
    }
    out
}

/// Linienzug am y-Bereich abschneiden (Kurve endet sauber am Rand statt flach weiterzulaufen).
pub fn clip_y(seg: &[(f64, f64)], ymin: f64, ymax: f64) -> Vec<Vec<(f64, f64)>> {
    let inside = |y: f64| y >= ymin && y <= ymax;
    let edge = |a: (f64, f64), b: (f64, f64)| {
        let yb = if b.1 > ymax || a.1 > ymax { ymax } else { ymin };
        let t = ((yb - a.1) / (b.1 - a.1)).clamp(0.0, 1.0);
        (a.0 + (b.0 - a.0) * t, yb)
    };
    let mut out = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    for i in 0..seg.len() {
        let p = seg[i];
        if inside(p.1) {
            if cur.is_empty() && i > 0 && !inside(seg[i - 1].1) {
                cur.push(edge(seg[i - 1], p));
            }
            cur.push(p);
        } else if !cur.is_empty() {
            cur.push(edge(*cur.last().unwrap(), p));
            out.push(std::mem::take(&mut cur));
        }
    }
    if cur.len() > 1 {
        out.push(cur);
    }
    out.retain(|c| c.len() > 1);
    out
}

// ---------------------------------------------------------------------------
// Tabellen
// ---------------------------------------------------------------------------

/// Tabelle mit `cols` × `rows` Zellen ab (x, y); `header` = farbige Kopfzeile.
#[allow(clippy::too_many_arguments)]
pub fn table_items(x: f32, y: f32, cols: usize, rows: usize, cw: f32, rh: f32, header: Option<[u8; 4]>, ink: [u8; 4]) -> Vec<Content> {
    let (w, h) = (cw * cols as f32, rh * rows as f32);
    let mut out = Vec::new();
    if let Some(c) = header {
        out.push(Content::Stroke(Stroke {
            pen: PenKind::Fill,
            color: c,
            width: 0.0,
            pts: [(x, y), (x + w, y), (x + w, y + rh), (x, y + rh)].iter().map(|&(a, b)| Pt::new(a, b, 1.0)).collect(),
        }));
    }
    for r in 1..rows {
        let yy = y + rh * r as f32;
        out.push(line(&[(x, yy), (x + w, yy)], ink, if r == 1 && header.is_some() { 1.3 } else { 0.8 }));
    }
    for c in 1..cols {
        let xx = x + cw * c as f32;
        out.push(line(&[(xx, y), (xx, y + h)], ink, 0.8));
    }
    out.push(line(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h), (x, y)], ink, 1.4));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_formulas() {
        let f = |s: &str, x: f64| parse(s).unwrap().eval(x);
        assert_eq!(f("x^2 - 2", 3.0), 7.0);
        assert_eq!(f("2x+1", 2.0), 5.0);
        assert_eq!(f("-x^2", 3.0), -9.0);
        assert_eq!(f("2^3^2", 0.0), 512.0);
        assert!((f("sin(pi/2)", 0.0) - 1.0).abs() < 1e-9);
        assert!((f("f(x) = 3(x+1)", 1.0) - 6.0).abs() < 1e-9);
        assert!((f("0,5x²", 2.0) - 2.0).abs() < 1e-9);
        assert!((f("2 sin x", std::f64::consts::FRAC_PI_2) - 2.0).abs() < 1e-9);
        assert!((f("sqrt(x)+e", 4.0) - (2.0 + std::f64::consts::E)).abs() < 1e-9);
        assert!(parse("x +").is_err());
        assert!(parse("foo(x)").is_err());
        assert!(parse("(x+1").is_err());
    }

    #[test]
    fn plot_has_axes_and_curves() {
        let spec = PlotSpec { funcs: vec![parse("1/x").unwrap()], xmin: -5.0, xmax: 5.0, y: None, grid: true };
        let items = plot_items(&spec, 0.0, 0.0, 300.0, 200.0, [0, 0, 0, 255]);
        let curves = items.iter().filter(|c| matches!(c, Content::Stroke(s) if s.color == PLOT_COLORS[0])).count();
        assert!(curves >= 2, "1/x ist an der Polstelle getrennt: {}", curves);
        assert_eq!(nice_step(10.0, 5.0), 2.0);
        let c = clip_y(&[(0.0, 0.0), (1.0, 2.0), (2.0, 4.0), (3.0, 2.0)], -1.0, 3.0);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].last().unwrap().1, 3.0);
        assert_eq!(c[1][0].1, 3.0);
        let t = table_items(0.0, 0.0, 3, 4, 50.0, 20.0, Some([1, 2, 3, 40]), [0, 0, 0, 255]);
        assert_eq!(t.len(), 1 + 3 + 2 + 1);
    }
}
