//! Vektor-PDF-Export: Papier, Vorlagen, Tinte, Textmarker, Text (auswählbar) und Bilder.

use super::format::decode_png;
use super::ink::PenKind;
use super::note::{Content, Notebook, Page, TextBox};
use super::paper::{line_colors, planner_label_pos, Prim, TemplateKind};
use miniz_oxide::deflate::compress_to_vec_zlib;
use std::collections::BTreeMap;
use std::fmt::Write as _;

fn n(v: f32) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.to_string()
    }
}

/// Text in WinAnsi (cp1252) für die Standardschrift Helvetica, als PDF-String-Literal.
pub fn pdf_text(s: &str) -> String {
    let mut out = String::from("(");
    for ch in s.chars() {
        let b: u8 = match ch {
            '€' => 0x80,
            '‚' => 0x82,
            '„' => 0x84,
            '…' => 0x85,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            '•' => 0x95,
            '–' => 0x96,
            '—' => 0x97,
            '™' => 0x99,
            c if (c as u32) >= 0x20 && (c as u32) < 0x7f => c as u8,
            c if (c as u32) >= 0xa0 && (c as u32) <= 0xff => c as u32 as u8,
            '\t' => b' ',
            _ => b'?',
        };
        match b {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(b as char);
            }
            0x20..=0x7e => out.push(b as char),
            _ => {
                let _ = write!(out, "\\{:03o}", b);
            }
        }
    }
    out.push(')');
    out
}

struct Pdf {
    buf: Vec<u8>,
    offsets: Vec<usize>,
}

impl Pdf {
    fn new() -> Pdf {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");
        Pdf { buf, offsets: Vec::new() }
    }

    /// Reserviert eine Objektnummer.
    fn reserve(&mut self) -> usize {
        self.offsets.push(0);
        self.offsets.len()
    }

    fn obj(&mut self, id: usize, body: &str) {
        self.offsets[id - 1] = self.buf.len();
        self.buf.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", id, body).as_bytes());
    }

    fn stream(&mut self, id: usize, dict: &str, data: &[u8]) {
        let z = compress_to_vec_zlib(data, 6);
        self.offsets[id - 1] = self.buf.len();
        self.buf.extend_from_slice(format!("{} 0 obj\n<< {} /Filter /FlateDecode /Length {} >>\nstream\n", id, dict, z.len()).as_bytes());
        self.buf.extend_from_slice(&z);
        self.buf.extend_from_slice(b"\nendstream\nendobj\n");
    }

    fn finish(mut self, root: usize) -> Vec<u8> {
        let xref = self.buf.len();
        let mut s = format!("xref\n0 {}\n0000000000 65535 f \n", self.offsets.len() + 1);
        for o in &self.offsets {
            let _ = writeln!(s, "{:010} 00000 n ", o);
        }
        let _ = write!(s, "trailer\n<< /Size {} /Root {} 0 R >>\nstartxref\n{}\n%%EOF\n", self.offsets.len() + 1, root, xref);
        self.buf.extend_from_slice(s.as_bytes());
        self.buf
    }
}

fn rgb(c: [f32; 4]) -> String {
    format!("{} {} {}", n(c[0]), n(c[1]), n(c[2]))
}

fn rgb8(c: [u8; 4]) -> String {
    format!("{} {} {}", n(c[0] as f32 / 255.0), n(c[1] as f32 / 255.0), n(c[2] as f32 / 255.0))
}

/// Exportiert die Seiten `pages` (Indizes) als PDF. `wrap` bricht Textfelder in Zeilen um.
pub fn export_pdf(nb: &Notebook, pages: &[usize], wrap: &dyn Fn(&TextBox) -> Vec<String>) -> Vec<u8> {
    let mut pdf = Pdf::new();
    let catalog = pdf.reserve();
    let pages_id = pdf.reserve();
    let font = pdf.reserve();
    let font_b = pdf.reserve();
    pdf.obj(font, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>");
    pdf.obj(font_b, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>");

    // Transparenzstufen (Alpha in 5%-Schritten), mit/ohne Multiplizieren
    let mut gstates: BTreeMap<(u8, bool), String> = BTreeMap::new();
    let mut gs_name = |alpha: f32, multiply: bool| -> String {
        let a = ((alpha.clamp(0.0, 1.0) * 20.0).round() as u8).min(20);
        let len = gstates.len();
        gstates.entry((a, multiply)).or_insert_with(|| format!("G{}", len)).clone()
    };
    // Bilder: (Schlüssel → Name, Objekt-ID)
    let mut images: BTreeMap<u64, (String, usize)> = BTreeMap::new();
    let mut page_ids = Vec::new();
    let (w, h) = (nb.format.w, nb.format.h);

    let mut contents: Vec<String> = Vec::new();
    for &pi in pages {
        let Some(page) = nb.pages.get(pi) else { continue };
        let mut c = String::new();
        let _ = writeln!(c, "q 1 0 0 -1 0 {} cm", n(h));
        page_background(&mut c, page, w, h, &mut gs_name);
        // Reihenfolge: Bilder, Textmarker, Tinte, Text
        for pass in 0..4 {
            for it in &page.items {
                match (&it.content, pass) {
                    (Content::Image(img), 0) => {
                        let id = if let Some((name, _)) = images.get(&img.key) {
                            name.clone()
                        } else if let Some((iw, ih, rgba)) = decode_png(&img.png) {
                            let name = format!("Im{}", images.len());
                            let oid = pdf.reserve();
                            let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
                            let has_alpha = rgba.chunks_exact(4).any(|p| p[3] < 255);
                            let smask = if has_alpha {
                                let sid = pdf.reserve();
                                let a: Vec<u8> = rgba.chunks_exact(4).map(|p| p[3]).collect();
                                pdf.stream(sid, &format!("/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray /BitsPerComponent 8", iw, ih), &a);
                                format!(" /SMask {} 0 R", sid)
                            } else {
                                String::new()
                            };
                            pdf.stream(oid, &format!("/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8{}", iw, ih, smask), &rgb);
                            images.insert(img.key, (name.clone(), oid));
                            name
                        } else {
                            continue;
                        };
                        let _ = writeln!(c, "q {} 0 0 {} {} {} cm /{} Do Q", n(img.w), n(-img.h), n(img.x), n(img.y + img.h), id);
                    }
                    (Content::Stroke(s), 1 | 2) => {
                        let hl = s.pen == PenKind::Highlighter;
                        if hl != (pass == 1) {
                            continue;
                        }
                        let alpha = s.color[3] as f32 / 255.0;
                        let o = s.outline();
                        if o.len() < 3 {
                            continue;
                        }
                        let _ = write!(c, "q /{} gs {} rg ", gs_name(alpha, hl), rgb8(s.color));
                        let _ = write!(c, "{} {} m ", n(o[0].0), n(o[0].1));
                        for p in &o[1..] {
                            let _ = write!(c, "{} {} l ", n(p.0), n(p.1));
                        }
                        c.push_str("h f Q\n");
                    }
                    (Content::Text(t), 3) => {
                        let lines = wrap(t);
                        let f = if t.bold { "F2" } else { "F1" };
                        let _ = write!(c, "BT /{} {} Tf {} rg ", f, n(t.size), rgb8(t.color));
                        for (i, line) in lines.iter().enumerate() {
                            let y = t.y + t.size * (0.95 + 1.3 * i as f32);
                            let _ = write!(c, "1 0 0 -1 {} {} Tm {} Tj ", n(t.x), n(y), pdf_text(line));
                        }
                        c.push_str("ET\n");
                    }
                    _ => {}
                }
            }
        }
        c.push_str("Q\n");
        contents.push(c);
    }

    // Gemeinsame Ressourcen
    let mut res = String::from("<< /Font << /F1 ");
    let _ = write!(res, "{} 0 R /F2 {} 0 R >> /ExtGState <<", font, font_b);
    for ((a, m), name) in &gstates {
        let _ = write!(res, " /{} << /ca {} /CA {}{} >>", name, n(*a as f32 / 20.0), n(*a as f32 / 20.0), if *m { " /BM /Multiply" } else { "" });
    }
    res.push_str(" >> /XObject <<");
    for (name, oid) in images.values() {
        let _ = write!(res, " /{} {} 0 R", name, oid);
    }
    res.push_str(" >> >>");
    let res_id = pdf.reserve();
    pdf.obj(res_id, &res);

    for c in contents {
        let cid = pdf.reserve();
        pdf.stream(cid, "", c.as_bytes());
        let pid = pdf.reserve();
        pdf.obj(pid, &format!("<< /Type /Page /Parent {} 0 R /MediaBox [0 0 {} {}] /Resources {} 0 R /Contents {} 0 R >>", pages_id, n(w), n(h), res_id, cid));
        page_ids.push(pid);
    }
    let kids: Vec<String> = page_ids.iter().map(|i| format!("{} 0 R", i)).collect();
    pdf.obj(pages_id, &format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), page_ids.len()));
    pdf.obj(catalog, &format!("<< /Type /Catalog /Pages {} 0 R >>", pages_id));
    pdf.finish(catalog)
}

fn page_background(c: &mut String, page: &Page, w: f32, h: f32, gs: &mut dyn FnMut(f32, bool) -> String) {
    let p = page.paper;
    let _ = writeln!(c, "{} {} {} rg 0 0 {} {} re f", n(p.r), n(p.g), n(p.b), n(w), n(h));
    let prims = page.template.primitives(w, h);
    if prims.is_empty() {
        return;
    }
    let (line, accent) = line_colors(p);
    let lw = page.template.line_width(w, h);
    for pr in &prims {
        match *pr {
            Prim::Line { x0, y0, x1, y1, accent: ac, strong } => {
                let col = if ac { accent } else { line };
                let _ = writeln!(c, "q /{} gs {} RG {} w {} {} m {} {} l S Q", gs(col[3], false), rgb(col), n(if strong { lw * 1.8 } else { lw }), n(x0), n(y0), n(x1), n(y1));
            }
            Prim::Dot { x, y, r } => {
                let k = r * 0.5523;
                let _ = writeln!(
                    c,
                    "q /{} gs {} rg {} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c f Q",
                    gs((line[3] * 1.4).min(1.0), false),
                    rgb(line),
                    n(x + r), n(y),
                    n(x + r), n(y + k), n(x + k), n(y + r), n(x), n(y + r),
                    n(x - k), n(y + r), n(x - r), n(y + k), n(x - r), n(y),
                    n(x - r), n(y - k), n(x - k), n(y - r), n(x), n(y - r),
                    n(x + k), n(y - r), n(x + r), n(y - k), n(x + r), n(y)
                );
            }
        }
    }
    if page.template.kind == TemplateKind::Planner {
        let size = page.template.unit(w, h) * 0.55;
        let _ = write!(c, "BT /F2 {} Tf {} rg ", n(size), rgb(line));
        for (x, y, l) in planner_label_pos(&page.template, w, h) {
            let _ = write!(c, "1 0 0 -1 {} {} Tm {} Tj ", n(x), n(y), pdf_text(l));
        }
        c.push_str("ET\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::color::Color;
    use crate::engine::ink::{Pt, Stroke};
    use crate::engine::note::{Format, Item};
    use crate::engine::paper::Template;

    #[test]
    fn text_escaping() {
        assert_eq!(pdf_text("a(b)"), "(a\\(b\\))");
        assert_eq!(pdf_text("ä€"), "(\\344\\200)");
    }

    #[test]
    fn exports_valid_looking_pdf() {
        let mut nb = Notebook::new("x".into(), "T", 0, Format::A4_PORTRAIT, Template { kind: TemplateKind::Planner, spacing: 1 }, Color::WHITE, 0.0);
        nb.pages[0].insert([
            Item { id: 5, order: 1.0, content: Content::Stroke(Stroke { pen: PenKind::Highlighter, color: [255, 230, 0, 90], width: 12.0, pts: vec![Pt::new(10.0, 10.0, 1.0), Pt::new(100.0, 10.0, 1.0)] }) },
            Item { id: 6, order: 2.0, content: Content::Text(TextBox { x: 10.0, y: 10.0, w: 100.0, size: 12.0, color: [0, 0, 0, 255], bold: false, text: "Grüße".into() }) },
        ]);
        let p = nb.new_page_like(0);
        nb.pages.push(p);
        let pdf = export_pdf(&nb, &[0, 1], &|t: &TextBox| vec![t.text.clone()]);
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.starts_with("%PDF-1.4"));
        assert!(s.contains("/Count 2"));
        assert!(s.contains("/BM /Multiply"));
        assert!(s.trim_end().ends_with("%%EOF"));
    }
}
