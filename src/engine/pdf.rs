//! Vektor-PDF-Export: Papier, Vorlagen, Tinte, Textmarker, Text (auswählbar) und Bilder.

use super::format::decode_png;
use super::ink::PenKind;
use super::note::{Content, ImageItem, Notebook, Page, TextBox};
use super::paper::{line_colors, Prim, TemplateKind};
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

/// Zeichen → WinAnsi (cp1252) für die Standardschrift Helvetica.
fn winansi(ch: char) -> Option<u8> {
    Some(match ch {
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
        _ => return None,
    })
}

/// Zeichen → Standardschrift „Symbol“ (griechische Buchstaben, Mathematik, Pfeile).
fn symbol_code(ch: char) -> Option<u8> {
    const GREEK_UP: &str = "ΑΒΧΔΕΦΓΗΙϑΚΛΜΝΟΠΘΡΣΤΥςΩΞΨΖ";
    const GREEK_LO: &str = "αβχδεφγηιϕκλμνοπθρστυϖωξψζ";
    if let Some(i) = GREEK_UP.chars().position(|c| c == ch) {
        return Some(0x41 + i as u8);
    }
    if let Some(i) = GREEK_LO.chars().position(|c| c == ch) {
        return Some(0x61 + i as u8);
    }
    Some(match ch {
        '∆' => 0x44,
        'Ω' => 0x57,
        '∀' => 0x22,
        '∃' => 0x24,
        '∋' => 0x27,
        '∗' => 0x2a,
        '−' => 0x2d,
        '≅' => 0x40,
        '⊥' => 0x5e,
        '∼' => 0x7e,
        '′' => 0xa2,
        '≤' => 0xa3,
        '∞' => 0xa5,
        '♣' => 0xa7,
        '♦' => 0xa8,
        '♥' => 0xa9,
        '♠' => 0xaa,
        '↔' => 0xab,
        '←' => 0xac,
        '↑' => 0xad,
        '→' => 0xae,
        '↓' => 0xaf,
        '″' => 0xb2,
        '≥' => 0xb3,
        '∝' => 0xb5,
        '∂' => 0xb6,
        '≠' => 0xb9,
        '≡' => 0xba,
        '≈' => 0xbb,
        'ℵ' => 0xc0,
        '⊗' => 0xc4,
        '⊕' => 0xc5,
        '∅' | '⌀' => 0xc6,
        '∩' => 0xc7,
        '∪' => 0xc8,
        '⊃' => 0xc9,
        '⊇' => 0xca,
        '⊂' => 0xcc,
        '⊆' => 0xcd,
        '∈' => 0xce,
        '∉' => 0xcf,
        '∠' => 0xd0,
        '∇' => 0xd1,
        '∏' => 0xd5,
        '√' => 0xd6,
        '⋅' => 0xd7,
        '¬' => 0xd8,
        '∧' => 0xd9,
        '∨' => 0xda,
        '⇔' => 0xdb,
        '⇐' => 0xdc,
        '⇑' => 0xdd,
        '⇒' => 0xde,
        '⇓' => 0xdf,
        '◊' => 0xe0,
        '∑' => 0xe5,
        '∫' => 0xf2,
        _ => return None,
    })
}

/// Zeichen → Standardschrift „ZapfDingbats“ (Häkchen, Sterne, Formen).
fn dingbat_code(ch: char) -> Option<u8> {
    Some(match ch {
        '✓' => 0x33,
        '✔' => 0x34,
        '✗' => 0x37,
        '✘' => 0x38,
        '★' => 0x48,
        '☆' => 0x49,
        '●' => 0x6c,
        '■' => 0x6e,
        '▲' => 0x73,
        '▼' => 0x74,
        '◆' => 0x75,
        '❤' => 0xa4,
        '➔' => 0xd4,
        '✎' => 0x2f,
        '✉' => 0x29,
        '☎' => 0x25,
        _ => return None,
    })
}

fn push_byte(out: &mut String, b: u8) {
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

/// Text in WinAnsi (cp1252) für die Standardschrift Helvetica, als PDF-String-Literal.
pub fn pdf_text(s: &str) -> String {
    let mut out = String::from("(");
    for ch in s.chars() {
        push_byte(&mut out, winansi(ch).unwrap_or(b'?'));
    }
    out.push(')');
    out
}

/// Textzeile als Folge von `Tj`-Befehlen: Sonderzeichen wechseln in die Schriften Symbol (F3)
/// bzw. ZapfDingbats (F4), danach zurück zu `font`. Die Position läuft dabei automatisch weiter.
pub fn pdf_runs(s: &str, font: &str, size: f32) -> String {
    let mut out = String::new();
    let mut cur = font;
    let mut run = String::new();
    let flush = |out: &mut String, run: &mut String| {
        if !run.is_empty() {
            let _ = write!(out, "({}) Tj ", run);
            run.clear();
        }
    };
    for ch in s.chars() {
        let (f, b) = if let Some(b) = winansi(ch) {
            (font, b)
        } else if let Some(b) = symbol_code(ch) {
            ("F3", b)
        } else if let Some(b) = dingbat_code(ch) {
            ("F4", b)
        } else {
            (font, b'?')
        };
        if f != cur {
            flush(&mut out, &mut run);
            let _ = write!(out, "/{} {} Tf ", f, n(size));
            cur = f;
        }
        push_byte(&mut run, b);
    }
    flush(&mut out, &mut run);
    if cur != font {
        let _ = write!(out, "/{} {} Tf ", font, n(size));
    }
    if out.is_empty() {
        out.push_str("() Tj ");
    }
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

    /// Stream ohne zusätzliche Kompression (z. B. JPEG mit /DCTDecode).
    fn stream_raw(&mut self, id: usize, dict: &str, data: &[u8]) {
        self.offsets[id - 1] = self.buf.len();
        self.buf.extend_from_slice(format!("{} 0 obj\n<< {} /Length {} >>\nstream\n", id, dict, data.len()).as_bytes());
        self.buf.extend_from_slice(data);
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
    let font_sym = pdf.reserve();
    let font_ding = pdf.reserve();
    pdf.obj(font_sym, "<< /Type /Font /Subtype /Type1 /BaseFont /Symbol >>");
    pdf.obj(font_ding, "<< /Type /Font /Subtype /Type1 /BaseFont /ZapfDingbats >>");

    // Transparenzstufen (Alpha in 5%-Schritten), mit/ohne Multiplizieren
    let mut gstates: BTreeMap<(u8, bool), String> = BTreeMap::new();
    let mut gs_name = |alpha: f32, multiply: bool| -> String {
        let a = ((alpha.clamp(0.0, 1.0) * 20.0).round() as u8).min(20);
        let len = gstates.len();
        gstates.entry((a, multiply)).or_insert_with(|| format!("G{}", len)).clone()
    };
    // Farbverläufe (Cover): Shading-Dictionaries, Name = Sh{Index}
    let mut shadings: Vec<String> = Vec::new();
    // Bilder: (Schlüssel → Name, Objekt-ID)
    let mut images: BTreeMap<u64, (String, usize)> = BTreeMap::new();
    // Seitenobjekte vorab reservieren (für Verweise zwischen Seiten)
    let exported: Vec<usize> = pages.iter().copied().filter(|&pi| pi < nb.pages.len()).collect();
    let page_ids: Vec<usize> = exported.iter().map(|_| pdf.reserve()).collect();
    let pid_of = |page_id: u32| -> Option<usize> { exported.iter().position(|&pi| nb.pages[pi].id == page_id).map(|k| page_ids[k]) };
    let mut annots: Vec<Vec<(super::geom::Rect, usize)>> = Vec::new();

    let mut contents: Vec<String> = Vec::new();
    for &pi in &exported {
        let page = &nb.pages[pi];
        let (w, h) = page.size(&nb.format);
        annots.push(page.links.iter().filter_map(|l| match l.target {
            super::note::LinkTarget::Page(id) => pid_of(id).map(|t| (l.rect, t)),
            _ => None,
        }).collect());
        let mut c = String::new();
        let _ = writeln!(c, "q 1 0 0 -1 0 {} cm", n(h));
        let bg = page.bg.as_ref().and_then(|img| {
            let (bw, bh) = image_size(&img.png)?;
            let (x, y, dw, dh) = fit_contain(bw as f32, bh as f32, w, h);
            embed_image(&mut pdf, &mut images, img).map(|id| format!("q {} 0 0 {} {} {} cm /{} Do Q\n", n(dw), n(-dh), n(x), n(y + dh), id))
        });
        page_background(&mut c, page, w, h, &mut gs_name, bg.as_deref(), &mut shadings);
        // Reihenfolge: Bilder, Textmarker, Tinte, Text
        for pass in 0..4 {
            for it in &page.items {
                match (&it.content, pass) {
                    (Content::Image(img), 0) => {
                        let Some(id) = embed_image(&mut pdf, &mut images, img) else { continue };
                        let _ = writeln!(c, "q {} 0 0 {} {} {} cm /{} Do Q", n(img.w), n(-img.h), n(img.x), n(img.y + img.h), id);
                    }
                    (Content::Stroke(s), 0..=2) => {
                        let hl = s.pen == PenKind::Highlighter;
                        let want = if s.pen == PenKind::Fill { 0 } else if hl { 1 } else { 2 };
                        if want != pass {
                            continue;
                        }
                        if s.is_fx() {
                            for (polys, col) in s.fx_layers() {
                                if polys.is_empty() {
                                    continue;
                                }
                                let _ = write!(c, "q /{} gs {} rg ", gs_name(col[3] as f32 / 255.0, false), rgb8(col));
                                for o in &polys {
                                    if o.len() < 3 {
                                        continue;
                                    }
                                    let _ = write!(c, "{} {} m ", n(o[0].0), n(o[0].1));
                                    for p in &o[1..] {
                                        let _ = write!(c, "{} {} l ", n(p.0), n(p.1));
                                    }
                                    c.push_str("h ");
                                }
                                c.push_str("f Q\n");
                            }
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
                    (Content::Node(nd), 0) => {
                        // Verbindung zur Elternblase (liegt unter allen Blasen)
                        if let Some(Content::Node(par)) = page.get(nd.parent).map(|p| &p.content) {
                            let q = par.edge_to(nd);
                            let _ = writeln!(
                                c,
                                "q /{} gs {} RG 2 w 1 J {} {} m {} {} {} {} {} {} c S Q",
                                gs_name(0.8, false),
                                rgb8(par.color),
                                n(q[0].0), n(q[0].1), n(q[1].0), n(q[1].1), n(q[2].0), n(q[2].1), n(q[3].0), n(q[3].1)
                            );
                        }
                    }
                    (Content::Node(nd), 3) => {
                        let r = (nd.h * 0.5).min(14.0);
                        let fill = [nd.color[0], nd.color[1], nd.color[2], 255];
                        let _ = write!(c, "q {} rg ", rgb8(fill));
                        round_rect(&mut c, nd.x, nd.y, nd.w, nd.h, r);
                        c.push_str("f Q\n");
                        let fs = nd.font_size();
                        let tb = TextBox { x: nd.x + nd.pad(), y: 0.0, w: nd.w - nd.pad() * 2.0, size: fs, color: [255, 255, 255, 255], bold: true, text: nd.text.clone() };
                        let lines = wrap(&tb);
                        let total = fs * 1.25 * lines.len() as f32;
                        let ink = if (fill[0] as f32 * 0.299 + fill[1] as f32 * 0.587 + fill[2] as f32 * 0.114) > 170.0 { "0.1 0.11 0.17" } else { "1 1 1" };
                        let _ = write!(c, "BT /F2 {} Tf {} rg ", n(fs), ink);
                        for (i, line) in lines.iter().enumerate() {
                            let lw = line.chars().count() as f32 * fs * 0.55;
                            let x = nd.x + (nd.w - lw).max(0.0) * 0.5;
                            let y = nd.y + (nd.h - total) * 0.5 + fs * (0.95 + 1.25 * i as f32);
                            let _ = write!(c, "1 0 0 -1 {} {} Tm {}", n(x), n(y), pdf_runs(line, "F2", fs));
                        }
                        c.push_str("ET\n");
                    }
                    (Content::Text(t), 3) => {
                        let lines = wrap(t);
                        let f = if t.bold { "F2" } else { "F1" };
                        let _ = write!(c, "BT /{} {} Tf {} rg ", f, n(t.size), rgb8(t.color));
                        for (i, line) in lines.iter().enumerate() {
                            let y = t.y + t.size * (0.95 + 1.3 * i as f32);
                            let _ = write!(c, "1 0 0 -1 {} {} Tm {}", n(t.x), n(y), pdf_runs(line, f, t.size));
                        }
                        c.push_str("ET\n");
                    }
                    _ => {}
                }
            }
        }
        page_checks(&mut c, page, w, h, &mut gs_name);
        c.push_str("Q\n");
        contents.push(c);
    }

    // Gemeinsame Ressourcen
    let mut res = String::from("<< /Font << /F1 ");
    let _ = write!(res, "{} 0 R /F2 {} 0 R /F3 {} 0 R /F4 {} 0 R >> /ExtGState <<", font, font_b, font_sym, font_ding);
    for ((a, m), name) in &gstates {
        let _ = write!(res, " /{} << /ca {} /CA {}{} >>", name, n(*a as f32 / 20.0), n(*a as f32 / 20.0), if *m { " /BM /Multiply" } else { "" });
    }
    res.push_str(" >>");
    if !shadings.is_empty() {
        res.push_str(" /Shading <<");
        for (i, d) in shadings.iter().enumerate() {
            let _ = write!(res, " /Sh{} {}", i, d);
        }
        res.push_str(" >>");
    }
    res.push_str(" /XObject <<");
    for (name, oid) in images.values() {
        let _ = write!(res, " /{} {} 0 R", name, oid);
    }
    res.push_str(" >> >>");
    let res_id = pdf.reserve();
    pdf.obj(res_id, &res);

    for (k, c) in contents.into_iter().enumerate() {
        let (w, h) = nb.pages[exported[k]].size(&nb.format);
        let cid = pdf.reserve();
        pdf.stream(cid, "", c.as_bytes());
        let mut annot_refs = Vec::new();
        for (r, target) in &annots[k] {
            let aid = pdf.reserve();
            pdf.obj(aid, &format!("<< /Type /Annot /Subtype /Link /Rect [{} {} {} {}] /Border [0 0 0] /Dest [{} 0 R /Fit] >>", n(r.x0), n(h - r.y1), n(r.x1), n(h - r.y0), target));
            annot_refs.push(format!("{} 0 R", aid));
        }
        let annots_str = if annot_refs.is_empty() { String::new() } else { format!(" /Annots [{}]", annot_refs.join(" ")) };
        let pid = page_ids[k];
        pdf.obj(pid, &format!("<< /Type /Page /Parent {} 0 R /MediaBox [0 0 {} {}] /Resources {} 0 R /Contents {} 0 R{} >>", pages_id, n(w), n(h), res_id, cid, annots_str));
    }
    let kids: Vec<String> = page_ids.iter().map(|i| format!("{} 0 R", i)).collect();
    pdf.obj(pages_id, &format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), page_ids.len()));
    pdf.obj(catalog, &format!("<< /Type /Catalog /Pages {} 0 R >>", pages_id));
    pdf.finish(catalog)
}

/// Abgehakte Checklisten-Zeilen im PDF.
fn page_checks(c: &mut String, page: &Page, w: f32, h: f32, gs: &mut dyn FnMut(f32, bool) -> String) {
    if page.checks.is_empty() {
        return;
    }
    let fade = page.template.kind == TemplateKind::Checklist;
    let p = page.paper;
    let light = p.luminance() > 0.5;
    for (i, row) in page.template.check_rows(w, h).iter().enumerate() {
        if !page.is_checked(i) {
            continue;
        }
        let s = row.size;
        if fade {
            let _ = writeln!(c, "q /{} gs {} {} {} rg {} {} {} {} re f Q", gs(0.55, false), n(p.r), n(p.g), n(p.b), n(row.text_x - 4.0), n(row.y0 + 1.0), n(row.x1 - row.text_x + 4.0), n(row.y1 - row.y0 - 2.0));
        }
        if let Some((x0, x1, y)) = page.row_strike(row) {
            let col = if light { "0.14 0.15 0.2" } else { "0.92 0.93 0.96" };
            let _ = writeln!(c, "q /{} gs {} RG {} w 1 J {} {} m {} {} l S Q", gs(0.85, false), col, n(s * 0.09), n(x0), n(y), n(x1), n(y));
        }
        let (bx, by) = (row.bx, row.by);
        let _ = writeln!(c, "q 0.427 0.361 1 rg {} {} {} {} re f Q", n(bx - s * 0.04), n(by - s * 0.04), n(s * 1.08), n(s * 1.08));
        let _ = writeln!(
            c,
            "q 1 1 1 RG {} w 1 J 1 j {} {} m {} {} l {} {} l S Q",
            n(s * 0.13),
            n(bx + s * 0.22), n(by + s * 0.53),
            n(bx + s * 0.43), n(by + s * 0.73),
            n(bx + s * 0.79), n(by + s * 0.3)
        );
    }
}

/// Abgerundetes Rechteck als PDF-Pfad.
fn round_rect(c: &mut String, x: f32, y: f32, w: f32, h: f32, r: f32) {
    let r = r.min(w * 0.5).min(h * 0.5).max(0.0);
    if r < 0.01 {
        let _ = write!(c, "{} {} {} {} re ", n(x), n(y), n(w), n(h));
        return;
    }
    let k = r * 0.4477;
    let (x1, y1) = (x + w, y + h);
    let _ = write!(
        c,
        "{} {} m {} {} l {} {} {} {} {} {} c {} {} l {} {} {} {} {} {} c {} {} l {} {} {} {} {} {} c {} {} l {} {} {} {} {} {} c h ",
        n(x + r), n(y),
        n(x1 - r), n(y),
        n(x1 - k), n(y), n(x1), n(y + k), n(x1), n(y + r),
        n(x1), n(y1 - r),
        n(x1), n(y1 - k), n(x1 - k), n(y1), n(x1 - r), n(y1),
        n(x + r), n(y1),
        n(x + k), n(y1), n(x), n(y1 - k), n(x), n(y1 - r),
        n(x), n(y + r),
        n(x), n(y + k), n(x + k), n(y), n(x + r), n(y)
    );
}

/// Breite, Höhe und Farbkanäle eines JPEG (aus dem SOF-Abschnitt).
pub fn jpeg_size(b: &[u8]) -> Option<(u32, u32, u8)> {
    if b.len() < 4 || b[0] != 0xFF || b[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 9 < b.len() {
        if b[i] != 0xFF {
            i += 1;
            continue;
        }
        let m = b[i + 1];
        if m == 0xD8 || m == 0x01 || (0xD0..=0xD7).contains(&m) || m == 0xFF {
            i += if m == 0xFF { 1 } else { 2 };
            continue;
        }
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        if matches!(m, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
            let h = u16::from_be_bytes([b[i + 5], b[i + 6]]) as u32;
            let w = u16::from_be_bytes([b[i + 7], b[i + 8]]) as u32;
            return Some((w, h, b[i + 9]));
        }
        i += 2 + len;
    }
    None
}

/// Pixelgröße eines PNG oder JPEG.
pub fn image_size(b: &[u8]) -> Option<(u32, u32)> {
    if let Some((w, h, _)) = jpeg_size(b) {
        return Some((w, h));
    }
    if b.len() > 24 && &b[1..4] == b"PNG" {
        return Some((u32::from_be_bytes([b[16], b[17], b[18], b[19]]), u32::from_be_bytes([b[20], b[21], b[22], b[23]])));
    }
    None
}

/// Bild (bw × bh) eingepasst und zentriert in w × h: (x, y, Breite, Höhe).
pub fn fit_contain(bw: f32, bh: f32, w: f32, h: f32) -> (f32, f32, f32, f32) {
    let s = (w / bw.max(1.0)).min(h / bh.max(1.0));
    let (dw, dh) = (bw * s, bh * s);
    // fast gleiches Seitenverhältnis: ganz füllen
    if (dw - w).abs() < w * 0.03 && (dh - h).abs() < h * 0.03 {
        return (0.0, 0.0, w, h);
    }
    ((w - dw) * 0.5, (h - dh) * 0.5, dw, dh)
}

/// Bild als XObject einbetten (einmal je Inhalt); liefert den Ressourcennamen.
fn embed_image(pdf: &mut Pdf, images: &mut BTreeMap<u64, (String, usize)>, img: &ImageItem) -> Option<String> {
    if let Some((name, _)) = images.get(&img.key) {
        return Some(name.clone());
    }
    if let Some((iw, ih, comps)) = jpeg_size(&img.png) {
        let name = format!("Im{}", images.len());
        let oid = pdf.reserve();
        let cs = if comps == 1 { "/DeviceGray" } else { "/DeviceRGB" };
        pdf.stream_raw(oid, &format!("/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace {} /BitsPerComponent 8 /Filter /DCTDecode", iw, ih, cs), &img.png);
        images.insert(img.key, (name.clone(), oid));
        return Some(name);
    }
    let (iw, ih, rgba) = decode_png(&img.png)?;
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
    Some(name)
}

/// Kreis als Bézier-Pfad.
fn circle_path(c: &mut String, x: f32, y: f32, r: f32) {
    let k = r * 0.5523;
    let _ = write!(
        c,
        "{} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c ",
        n(x + r), n(y),
        n(x + r), n(y + k), n(x + k), n(y + r), n(x), n(y + r),
        n(x - k), n(y + r), n(x - r), n(y + k), n(x - r), n(y),
        n(x - r), n(y - k), n(x - k), n(y - r), n(x), n(y - r),
        n(x + k), n(y - r), n(x + r), n(y - k), n(x + r), n(y)
    );
}

fn page_background(c: &mut String, page: &Page, w: f32, h: f32, gs: &mut dyn FnMut(f32, bool) -> String, bg: Option<&str>, shadings: &mut Vec<String>) {
    let p = page.paper;
    let _ = writeln!(c, "{} {} {} rg 0 0 {} {} re f", n(p.r), n(p.g), n(p.b), n(w), n(h));
    if let Some(cmd) = bg {
        c.push_str(cmd);
    }
    let prims = page.template.primitives(w, h);
    if prims.is_empty() {
        return;
    }
    let light = p.luminance() > 0.5;
    let (line, accent) = line_colors(p);
    let lw = page.template.line_width(w, h);
    // Reihenfolge: Flächen, Linien/Punkte/Kästchen, Beschriftungen
    for pass in 0..3 {
        for pr in &prims {
            match (pr.clone(), pass) {
                (Prim::Grad { x, y, w: gw, h: gh, c1, c2 }, 0) => {
                    let d = format!(
                        "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [{} {} {} {}] /Function << /FunctionType 2 /Domain [0 1] /C0 [{}] /C1 [{}] /N 1 >> /Extend [true true] >>",
                        n(x), n(y), n(x + gw), n(y + gh), rgb8(c1), rgb8(c2)
                    );
                    let idx = shadings.iter().position(|e| *e == d).unwrap_or_else(|| {
                        shadings.push(d);
                        shadings.len() - 1
                    });
                    let _ = writeln!(c, "q {} {} {} {} re W n /Sh{} sh Q", n(x), n(y), n(gw), n(gh), idx);
                }
                (Prim::Circle { x, y, r, color }, 0) => {
                    let _ = write!(c, "q /{} gs {} rg ", gs(color[3] as f32 / 255.0, false), rgb8(color));
                    circle_path(c, x, y, r);
                    c.push_str("f Q\n");
                }
                (Prim::Seg { x0, y0, x1, y1, w: lw, color, dash }, 0) => {
                    let d = if dash { format!("[{} {}] 0 d ", n(lw * 4.0), n(lw * 3.0)) } else { String::new() };
                    let _ = writeln!(c, "q /{} gs {} RG {} w 1 J {}{} {} m {} {} l S Q", gs(color[3] as f32 / 255.0, false), rgb8(color), n(lw), d, n(x0), n(y0), n(x1), n(y1));
                }
                (Prim::Poly { pts, w: lw, color }, 0) if pts.len() > 1 => {
                    let _ = write!(c, "q /{} gs {} RG {} w 1 J 1 j {} {} m ", gs(color[3] as f32 / 255.0, false), rgb8(color), n(lw), n(pts[0].0), n(pts[0].1));
                    for p in &pts[1..] {
                        let _ = write!(c, "{} {} l ", n(p.0), n(p.1));
                    }
                    c.push_str("S Q\n");
                }
                (Prim::Fill { x, y, w: fw, h: fh, r, color }, 0) => {
                    let _ = write!(c, "q /{} gs {} rg ", gs(color[3] as f32 / 255.0, false), rgb8(color));
                    round_rect(c, x, y, fw, fh, r);
                    c.push_str("f Q\n");
                }
                (Prim::Line { x0, y0, x1, y1, accent: ac, strong }, 1) => {
                    let col = if ac { accent } else { line };
                    let _ = writeln!(c, "q /{} gs {} RG {} w {} {} m {} {} l S Q", gs(col[3], false), rgb(col), n(if strong { lw * 1.8 } else { lw }), n(x0), n(y0), n(x1), n(y1));
                }
                (Prim::Box { x, y, size }, 1) => {
                    let _ = writeln!(c, "q /{} gs {} RG {} w {} {} {} {} re S Q", gs(line[3], false), rgb(line), n(lw * 1.8), n(x), n(y), n(size), n(size));
                }
                (Prim::Dot { x, y, r }, 1) => {
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
                (Prim::Label { x, y, size, text, color, bold }, 2) => {
                    let (col, a) = match color {
                        Some(cl) => (rgb8(cl), cl[3] as f32 / 255.0),
                        None => (rgb(line), if light { 0.9 } else { 0.7 }),
                    };
                    let f = if bold { "F2" } else { "F1" };
                    let _ = writeln!(c, "q /{} gs BT /{} {} Tf {} rg 1 0 0 -1 {} {} Tm {}ET Q", gs(a, false), f, n(size), col, n(x), n(y), pdf_runs(&text, f, size));
                }
                _ => {}
            }
        }
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
    fn exports_all_templates() {
        for k in TemplateKind::ALL {
            let nb = Notebook::new("x".into(), "T", 0, Format::A4_PORTRAIT, Template::new(k, 1), Color::WHITE, 0.0);
            let s = String::from_utf8_lossy(&export_pdf(&nb, &[0], &|t: &TextBox| vec![t.text.clone()])).to_string();
            assert!(s.ends_with("%%EOF\n"), "{:?}", k);
        }
    }

    #[test]
    fn cover_page_pdf() {
        let mut nb = Notebook::new("x".into(), "Mein Buch", 18, Format::A4_PORTRAIT, Template::default(), Color::WHITE, 0.0);
        let p = nb.make_cover_page();
        nb.pages.insert(0, p);
        let s = String::from_utf8_lossy(&export_pdf(&nb, &[0, 1], &|t: &TextBox| vec![t.text.clone()])).to_string();
        assert!(s.contains("/Shading <<"));
        assert!(nb.set_title("Neu"));
        assert!(nb.pages[0].items.iter().any(|i| matches!(&i.content, Content::Text(t) if t.text == "Neu")));
    }

    #[test]
    fn planner_pdf_has_links() {
        let nb = crate::engine::planner::year_planner("p".into(), "P", 0, Format::A4_PORTRAIT, 2027, Color::WHITE, 0.0);
        let all: Vec<usize> = (0..nb.pages.len()).collect();
        let s = String::from_utf8_lossy(&export_pdf(&nb, &all, &|t: &TextBox| vec![t.text.clone()])).to_string();
        assert!(s.contains("/Subtype /Link"));
        assert!(s.contains(&format!("/Count {}", nb.pages.len())));
    }

    #[test]
    fn text_escaping() {
        assert_eq!(pdf_text("a(b)"), "(a\\(b\\))");
        assert_eq!(pdf_text("ä€"), "(\\344\\200)");
        // Sonderzeichen wechseln die Schrift und wieder zurück
        assert_eq!(pdf_runs("a∑b", "F1", 12.0), "(a) Tj /F3 12 Tf (\\345) Tj /F1 12 Tf (b) Tj ");
        assert_eq!(pdf_runs("α✓", "F2", 10.0), "/F3 10 Tf (a) Tj /F4 10 Tf (3) Tj /F2 10 Tf ");
    }

    #[test]
    fn exports_valid_looking_pdf() {
        let mut nb = Notebook::new("x".into(), "T", 0, Format::A4_PORTRAIT, Template::new(TemplateKind::Planner, 1), Color::WHITE, 0.0);
        nb.pages[0].insert([
            Item { id: 5, order: 1.0, content: Content::Stroke(Stroke { pen: PenKind::Highlighter, color: [255, 230, 0, 90], width: 12.0, pts: vec![Pt::new(10.0, 10.0, 1.0), Pt::new(100.0, 10.0, 1.0)] }), pinned: false },
            Item { id: 6, order: 2.0, content: Content::Text(TextBox { x: 10.0, y: 10.0, w: 100.0, size: 12.0, color: [0, 0, 0, 255], bold: false, text: "Grüße".into() }), pinned: false },
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
