//! Speicherformat: Notizbücher (`.znote`), Bibliotheksliste, Import alter Zeichnungen, PNG-Helfer.

use super::color::Color;
use super::ink::{PenKind, Pt, Stroke};
use super::geom::Rect;
use super::note::{hash_bytes, Card, Content, Format, ImageItem, Item, Notebook, Page, Region, TextBox};
use super::paper::{Template, TemplateKind};
use miniz_oxide::deflate::compress_to_vec;
use miniz_oxide::inflate::decompress_to_vec;
use std::rc::Rc;

const MAGIC: &[u8; 4] = b"ZNB1";
const VERSION: u32 = 3;

struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn bytes(&mut self, b: &[u8]) {
        self.u32(b.len() as u32);
        self.0.extend_from_slice(b);
    }
    fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }
    fn rgba(&mut self, c: [u8; 4]) {
        self.0.extend_from_slice(&c);
    }
}

pub struct R<'a> {
    b: &'a [u8],
    p: usize,
}

const BROKEN: &str = "Datei ist beschädigt oder unvollständig";

impl<'a> R<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        R { b, p: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.p + n > self.b.len() {
            return Err(BROKEN.into());
        }
        let s = &self.b[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn f64(&mut self) -> Result<f64, String> {
        Ok(f64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn bytes(&mut self) -> Result<&'a [u8], String> {
        let n = self.u32()? as usize;
        self.take(n)
    }
    fn str(&mut self) -> Result<String, String> {
        Ok(String::from_utf8_lossy(self.bytes()?).into_owned())
    }
    fn rgba(&mut self) -> Result<[u8; 4], String> {
        Ok(self.take(4)?.try_into().unwrap())
    }
}

pub fn save_notebook(nb: &Notebook) -> Vec<u8> {
    let mut w = W(Vec::new());
    w.str(&nb.id);
    w.str(&nb.title);
    w.u8(nb.cover);
    w.u8(nb.favorite as u8);
    w.f64(nb.created);
    w.f64(nb.modified);
    w.f32(nb.format.w);
    w.f32(nb.format.h);
    w.u32(nb.next_id);
    w.str(&nb.folder);
    w.u8(nb.is_template as u8);
    w.u32(nb.pages.len() as u32);
    for p in &nb.pages {
        w.u32(p.id);
        w.u8(p.template.kind as u8);
        w.u8(p.template.spacing);
        let [r, g, b] = p.paper.to_u8();
        w.rgba([r, g, b, 255]);
        w.u8(p.bookmark as u8);
        w.u32(p.checks.len() as u32);
        for c in &p.checks {
            w.0.extend_from_slice(&c.to_le_bytes());
        }
        w.str(&p.title);
        w.u32(p.items.len() as u32);
        for it in &p.items {
            match &it.content {
                Content::Stroke(_) => w.u8(0),
                Content::Text(_) => w.u8(1),
                Content::Image(_) => w.u8(2),
            }
            w.u32(it.id);
            w.f64(it.order);
            match &it.content {
                Content::Stroke(s) => {
                    w.u8(s.pen as u8);
                    w.rgba(s.color);
                    w.f32(s.width);
                    w.u32(s.pts.len() as u32);
                    for q in &s.pts {
                        w.f32(q.x);
                        w.f32(q.y);
                        w.u8((q.p.clamp(0.0, 1.0) * 255.0).round() as u8);
                    }
                }
                Content::Text(t) => {
                    w.f32(t.x);
                    w.f32(t.y);
                    w.f32(t.w);
                    w.f32(t.size);
                    w.rgba(t.color);
                    w.u8(t.bold as u8);
                    w.str(&t.text);
                }
                Content::Image(i) => {
                    w.f32(i.x);
                    w.f32(i.y);
                    w.f32(i.w);
                    w.f32(i.h);
                    w.bytes(&i.png);
                }
            }
        }
    }
    w.u32(nb.cards.len() as u32);
    for c in &nb.cards {
        w.u32(c.id);
        for r in [&c.front, &c.back] {
            w.u32(r.page);
            w.f32(r.rect.x0);
            w.f32(r.rect.y0);
            w.f32(r.rect.x1);
            w.f32(r.rect.y1);
        }
        w.u8(c.level);
        w.u32(c.right);
        w.u32(c.wrong);
    }
    let mut out = Vec::with_capacity(w.0.len() / 3 + 16);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&compress_to_vec(&w.0, 3));
    out
}

pub fn load_notebook(bytes: &[u8]) -> Result<Notebook, String> {
    if bytes.len() < 8 || &bytes[0..4] != MAGIC {
        return Err("Keine Notizbuch-Datei (.znote)".into());
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if version > VERSION {
        return Err("Notizbuch stammt aus einer neueren Version".into());
    }
    let body = decompress_to_vec(&bytes[8..]).map_err(|_| BROKEN.to_string())?;
    let mut r = R::new(&body);
    let id = r.str()?;
    let title = r.str()?;
    let cover = r.u8()?;
    let favorite = r.u8()? != 0;
    let created = r.f64()?;
    let modified = r.f64()?;
    let format = Format { w: r.f32()?.clamp(50.0, 5000.0), h: r.f32()?.clamp(50.0, 5000.0) };
    let next_id = r.u32()?;
    let (folder, is_template) = if version >= 3 { (r.str()?, r.u8()? != 0) } else { (String::new(), false) };
    let np = r.u32()? as usize;
    let mut pages = Vec::with_capacity(np.min(10_000));
    let mut max_id = 0;
    for _ in 0..np {
        let pid = r.u32()?;
        max_id = max_id.max(pid);
        let template = Template { kind: TemplateKind::from_u8(r.u8()?), spacing: r.u8()?.min(2) };
        let c = r.rgba()?;
        let paper = Color::from_u8(c[0], c[1], c[2]);
        let bookmark = r.u8()? != 0;
        let mut checks = Vec::new();
        if version >= 2 {
            let nc = r.u32()? as usize;
            for _ in 0..nc.min(10_000) {
                let b = r.take(2)?;
                checks.push(u16::from_le_bytes([b[0], b[1]]));
            }
        }
        let title = if version >= 3 { r.str()? } else { String::new() };
        let ni = r.u32()? as usize;
        let mut items = Vec::with_capacity(ni.min(100_000));
        for _ in 0..ni {
            let kind = r.u8()?;
            let id = r.u32()?;
            max_id = max_id.max(id);
            let order = r.f64()?;
            let content = match kind {
                0 => {
                    let pen = PenKind::from_u8(r.u8()?);
                    let color = r.rgba()?;
                    let width = r.f32()?;
                    let n = r.u32()? as usize;
                    let mut pts = Vec::with_capacity(n.min(1_000_000));
                    for _ in 0..n {
                        pts.push(Pt::new(r.f32()?, r.f32()?, r.u8()? as f32 / 255.0));
                    }
                    Content::Stroke(Stroke { pen, color, width, pts })
                }
                1 => Content::Text(TextBox { x: r.f32()?, y: r.f32()?, w: r.f32()?, size: r.f32()?, color: r.rgba()?, bold: r.u8()? != 0, text: r.str()? }),
                2 => {
                    let (x, y, w, h) = (r.f32()?, r.f32()?, r.f32()?, r.f32()?);
                    let png = r.bytes()?.to_vec();
                    let key = hash_bytes(&png);
                    Content::Image(ImageItem { x, y, w, h, png: Rc::new(png), key })
                }
                _ => return Err(BROKEN.into()),
            };
            items.push(Item { id, order, content });
        }
        items.sort_by(|a, b| a.order.partial_cmp(&b.order).unwrap_or(std::cmp::Ordering::Equal));
        pages.push(Page { id: pid, template, paper, bookmark, checks, title, items, version: 1 });
    }
    if pages.is_empty() {
        return Err("Notizbuch enthält keine Seiten".into());
    }
    let mut cards = Vec::new();
    if version >= 3 {
        let nc = r.u32()? as usize;
        for _ in 0..nc.min(100_000) {
            let cid = r.u32()?;
            max_id = max_id.max(cid);
            let mut reg = || -> Result<Region, String> {
                let page = r.u32()?;
                Ok(Region { page, rect: Rect::new(r.f32()?, r.f32()?, r.f32()?, r.f32()?) })
            };
            let front = reg()?;
            let back = reg()?;
            cards.push(Card { id: cid, front, back, level: r.u8()?.min(4), right: r.u32()?, wrong: r.u32()? });
        }
    }
    let mut nb = Notebook { id, title, cover, favorite, created, modified, format, pages, next_id: next_id.max(max_id + 1), folder, is_template, cards };
    nb.prune_cards();
    Ok(nb)
}

// ---------------------------------------------------------------------------
// Bibliothek
// ---------------------------------------------------------------------------

/// Kurzinfo für die Bibliotheksansicht (ohne Seiteninhalt).
#[derive(Clone, Debug, PartialEq)]
pub struct NoteMeta {
    pub id: String,
    pub title: String,
    pub cover: u8,
    pub favorite: bool,
    pub created: f64,
    pub modified: f64,
    pub pages: u32,
    pub w: f32,
    pub h: f32,
    pub folder: String,
    pub is_template: bool,
    /// Lernbare Karten / davon sicher gelernt
    pub cards: u32,
    pub learned: u32,
    /// Enthält Seiten mit der Vorlage „Lernkarte“ (Lernkarten-Set)
    pub deck: bool,
    /// Lernfortschritt in Prozent (Durchschnitt der Karteikasten-Fächer)
    pub progress: u32,
}

impl NoteMeta {
    pub fn of(nb: &Notebook) -> NoteMeta {
        NoteMeta {
            id: nb.id.clone(),
            title: nb.title.clone(),
            cover: nb.cover,
            favorite: nb.favorite,
            created: nb.created,
            modified: nb.modified,
            pages: nb.pages.len() as u32,
            w: nb.format.w,
            h: nb.format.h,
            folder: nb.folder.clone(),
            is_template: nb.is_template,
            cards: nb.study_cards().len() as u32,
            learned: nb.study_cards().iter().filter(|c| c.level >= 4).count() as u32,
            progress: {
                let c = nb.study_cards();
                if c.is_empty() {
                    0
                } else {
                    (c.iter().map(|c| c.level as u32).sum::<u32>() * 100) / (c.len() as u32 * 4)
                }
            },
            deck: nb.pages.iter().any(|p| p.template.kind == TemplateKind::Flashcard),
        }
    }
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

pub fn save_library(list: &[NoteMeta]) -> String {
    list.iter()
        .map(|m| format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", clean(&m.id), clean(&m.title), m.cover, m.favorite as u8, m.created, m.modified, m.pages, m.w, m.h, clean(&m.folder), m.is_template as u8, m.cards, m.learned, m.deck as u8, m.progress))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn load_library(s: &str) -> Vec<NoteMeta> {
    s.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 9 {
                return None;
            }
            Some(NoteMeta {
                id: f[0].to_string(),
                title: f[1].to_string(),
                cover: f[2].parse().unwrap_or(0),
                favorite: f[3] == "1",
                created: f[4].parse().unwrap_or(0.0),
                modified: f[5].parse().unwrap_or(0.0),
                pages: f[6].parse().unwrap_or(1),
                w: f[7].parse().unwrap_or(595.0),
                h: f[8].parse().unwrap_or(842.0),
                folder: f.get(9).map(|x| x.to_string()).unwrap_or_default(),
                is_template: f.get(10) == Some(&"1"),
                cards: f.get(11).and_then(|x| x.parse().ok()).unwrap_or(0),
                learned: f.get(12).and_then(|x| x.parse().ok()).unwrap_or(0),
                deck: f.get(13) == Some(&"1"),
                progress: f.get(14).and_then(|x| x.parse().ok()).unwrap_or(0),
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// PNG
// ---------------------------------------------------------------------------

pub fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        if let Ok(mut wr) = enc.write_header() {
            let _ = wr.write_image_data(rgba);
        }
    }
    out
}

/// PNG → (Breite, Höhe, RGBA8).
pub fn decode_png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut dec = png::Decoder::new(bytes);
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    let data = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => data.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        _ => return None,
    };
    Some((w, h, rgba))
}

// ---------------------------------------------------------------------------
// Alte Zeichnungen (Ebenen-Format „ZTL1“) als Bild übernehmen
// ---------------------------------------------------------------------------

/// Liest eine alte Zeichnung und setzt die sichtbaren Ebenen zu einem Bild zusammen.
pub fn legacy_drawing_to_rgba(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut r = R::new(bytes);
    if r.take(4)? != b"ZTL1" {
        return Err("Keine alte Zeichnung".into());
    }
    let version = r.u32()?;
    let (w, h) = (r.u32()? as usize, r.u32()? as usize);
    if w == 0 || h == 0 || w > 16384 || h > 16384 {
        return Err(BROKEN.into());
    }
    let has_paper = r.u8()? == 1;
    let paper = [r.f32()?, r.f32()?, r.f32()?];
    if version >= 2 {
        r.u8()?;
        r.u8()?;
    }
    let _active = r.u32()?;
    let _next = r.u32()?;
    let n = r.u32()? as usize;
    let mut acc = vec![[0f32; 4]; w * h];
    if has_paper {
        acc.fill([paper[0], paper[1], paper[2], 1.0]);
    }
    for _ in 0..n {
        let _id = r.u32()?;
        let _name = r.bytes()?;
        let opacity = r.f32()?.clamp(0.0, 1.0);
        let visible = r.u8()? != 0;
        let _blend = r.u8()?;
        let _locked = r.u8()?;
        let _alpha = r.u8()?;
        let data = decompress_to_vec(r.bytes()?).map_err(|_| BROKEN.to_string())?;
        if !visible || data.len() < w * h * 4 {
            continue;
        }
        for (a, px) in acc.iter_mut().zip(data.chunks_exact(4)) {
            let sa = px[3] as f32 / 255.0 * opacity;
            if sa <= 0.0 {
                continue;
            }
            let k = 1.0 - sa;
            for c in 0..3 {
                a[c] = px[c] as f32 / 255.0 * opacity + a[c] * k;
            }
            a[3] = sa + a[3] * k;
        }
    }
    let mut out = vec![0u8; w * h * 4];
    for (o, a) in out.chunks_exact_mut(4).zip(acc.iter()) {
        if a[3] > 0.0 {
            for c in 0..3 {
                o[c] = ((a[c] / a[3]).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            }
            o[3] = (a[3].min(1.0) * 255.0 + 0.5) as u8;
        }
    }
    Ok((w as u32, h as u32, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notebook_roundtrip() {
        let mut nb = Notebook::new("nb1".into(), "Mathe ✓ Ärger", 3, Format::A4_PORTRAIT, Template { kind: TemplateKind::Grid, spacing: 2 }, Color::from_hex("#fbf6e9").unwrap(), 1234.0);
        let id = nb.alloc_id();
        let png = encode_png(2, 1, &[255, 0, 0, 255, 0, 0, 255, 128]);
        nb.pages[0].insert([
            Item { id, order: 1.0, content: Content::Stroke(Stroke { pen: PenKind::Brush, color: [1, 2, 3, 255], width: 2.5, pts: vec![Pt::new(1.0, 2.0, 1.0), Pt::new(3.0, 4.0, 0.0)] }) },
            Item { id: id + 1, order: 2.0, content: Content::Text(TextBox { x: 10.0, y: 20.0, w: 200.0, size: 14.0, color: [0, 0, 0, 255], bold: true, text: "Hallo\nWelt".into() }) },
            Item { id: id + 2, order: 3.0, content: Content::Image(ImageItem { x: 0.0, y: 0.0, w: 50.0, h: 25.0, key: hash_bytes(&png), png: Rc::new(png) }) },
        ]);
        nb.next_id = id + 3;
        nb.pages[0].bookmark = true;
        nb.pages[0].checks = vec![0, 3];
        nb.pages[0].title = "Kapitel 1".into();
        nb.folder = "Schule".into();
        nb.cards.push(Card { id: 99, front: Region { page: nb.pages[0].id, rect: Rect::new(1.0, 2.0, 30.0, 40.0) }, back: Region { page: nb.pages[0].id, rect: Rect::new(5.0, 50.0, 60.0, 90.0) }, level: 2, right: 3, wrong: 1 });
        nb.next_id = nb.next_id.max(100);
        let bytes = save_notebook(&nb);
        let mut back = load_notebook(&bytes).unwrap();
        back.pages[0].version = nb.pages[0].version;
        assert_eq!(back, nb);
        assert!(load_notebook(b"nope").is_err());
    }

    #[test]
    fn library_roundtrip() {
        let m = NoteMeta { id: "a".into(), title: "Ti\ttel".into(), cover: 2, favorite: true, created: 1.0, modified: 2.0, pages: 3, w: 595.0, h: 842.0, folder: "Schule".into(), is_template: true, cards: 7, learned: 2, deck: true, progress: 40 };
        let back = load_library(&save_library(&[m.clone()]));
        assert_eq!(back[0].title, "Ti tel");
        assert_eq!(back[0].pages, 3);
        assert!(back[0].favorite);
        assert_eq!(back[0].folder, "Schule");
        assert!(back[0].is_template);
        assert_eq!((back[0].cards, back[0].learned, back[0].deck), (7, 2, true));
        // alte Zeilen ohne Ordner bleiben lesbar
        let old = load_library("x\tAlt\t0\t0\t1\t2\t1\t595\t842");
        assert_eq!(old[0].folder, "");
    }

    #[test]
    fn png_roundtrip() {
        let px = [1, 2, 3, 4, 5, 6, 7, 8];
        let (w, h, d) = decode_png(&encode_png(2, 1, &px)).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(d, px);
    }
}
