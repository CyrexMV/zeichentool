//! Projektdatei (`.zeichen`) und PNG-Export.

use super::color::Color;
use super::doc::{Document, Overlay};
use super::history::{pack, unpack};
use super::layer::{BlendMode, Layer, LayerProps};
use std::collections::HashMap;

const MAGIC: &[u8; 4] = b"ZTL1";
const VERSION: u32 = 1;

/// Cache bereits komprimierter Ebenen, damit Autospeichern schnell bleibt.
#[derive(Default)]
pub struct PackCache {
    entries: HashMap<u32, (u64, Vec<u8>)>,
}

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn bytes(&mut self, b: &[u8]) {
        self.u32(b.len() as u32);
        self.0.extend_from_slice(b);
    }
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.p + n > self.b.len() {
            return Err("Datei ist beschädigt oder unvollständig".into());
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
    fn bytes(&mut self) -> Result<&'a [u8], String> {
        let n = self.u32()? as usize;
        self.take(n)
    }
}

pub fn save(doc: &Document, cache: &mut PackCache) -> Vec<u8> {
    let mut w = Writer(Vec::new());
    w.0.extend_from_slice(MAGIC);
    w.u32(VERSION);
    w.u32(doc.w as u32);
    w.u32(doc.h as u32);
    match doc.paper {
        Some(c) => {
            w.u8(1);
            w.f32(c.r);
            w.f32(c.g);
            w.f32(c.b);
        }
        None => {
            w.u8(0);
            w.f32(1.0);
            w.f32(1.0);
            w.f32(1.0);
        }
    }
    w.u32(doc.active as u32);
    w.u32(doc.next_id);
    w.u32(doc.layers.len() as u32);
    let mut alive = Vec::new();
    for l in &doc.layers {
        alive.push(l.id);
        w.u32(l.id);
        w.bytes(l.props.name.as_bytes());
        w.f32(l.props.opacity);
        w.u8(l.props.visible as u8);
        w.u8(l.props.blend as u8);
        w.u8(l.props.locked as u8);
        w.u8(l.props.alpha_lock as u8);
        let fresh = matches!(cache.entries.get(&l.id), Some((rev, _)) if *rev == l.rev);
        if !fresh {
            cache.entries.insert(l.id, (l.rev, pack(&l.data)));
        }
        w.bytes(&cache.entries[&l.id].1);
    }
    cache.entries.retain(|id, _| alive.contains(id));
    w.0
}

pub fn load(bytes: &[u8]) -> Result<Document, String> {
    let mut r = Reader { b: bytes, p: 0 };
    if r.take(4)? != MAGIC {
        return Err("Keine Zeichentool-Projektdatei".into());
    }
    let version = r.u32()?;
    if version > VERSION {
        return Err("Projekt wurde mit einer neueren Version erstellt".into());
    }
    let w = r.u32()? as usize;
    let h = r.u32()? as usize;
    if w == 0 || h == 0 || w > 16384 || h > 16384 {
        return Err("Ungültige Leinwandgröße".into());
    }
    let has_paper = r.u8()? == 1;
    let paper = Color::rgb(r.f32()?, r.f32()?, r.f32()?);
    let active = r.u32()? as usize;
    let next_id = r.u32()?;
    let n = r.u32()? as usize;
    let mut layers = Vec::with_capacity(n);
    for _ in 0..n {
        let id = r.u32()?;
        let name = String::from_utf8_lossy(r.bytes()?).into_owned();
        let opacity = r.f32()?.clamp(0.0, 1.0);
        let visible = r.u8()? != 0;
        let blend = BlendMode::from_u8(r.u8()?);
        let locked = r.u8()? != 0;
        let alpha_lock = r.u8()? != 0;
        let mut data = unpack(r.bytes()?);
        data.resize(w * h * 4, 0);
        layers.push(Layer { id, props: LayerProps { name, opacity, visible, blend, locked, alpha_lock }, data, rev: 1 });
    }
    if layers.is_empty() {
        return Err("Projekt enthält keine Ebenen".into());
    }
    let next_id = next_id.max(layers.iter().map(|l| l.id).max().unwrap_or(0) + 1);
    Ok(Document {
        w,
        h,
        active: active.min(layers.len() - 1),
        layers,
        paper: if has_paper { Some(paper) } else { None },
        next_id,
    })
}

/// Exportiert das sichtbare Bild als PNG.
pub fn export_png(doc: &Document, with_paper: bool) -> Vec<u8> {
    let mut rgba = Vec::new();
    doc.composite(doc.bounds(), &Overlay::None, with_paper, &mut rgba);
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, doc.w as u32, doc.h as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        if let Ok(mut wr) = enc.write_header() {
            let _ = wr.write_image_data(&rgba);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut d = Document::new(16, 9);
        d.add_layer("Linien", 1);
        d.layers[1].data[4 * 5 + 3] = 255;
        d.layers[1].props.blend = BlendMode::Multiply;
        d.paper = None;
        let mut cache = PackCache::default();
        let bytes = save(&d, &mut cache);
        let l = load(&bytes).unwrap();
        assert_eq!((l.w, l.h), (16, 9));
        assert_eq!(l.layers.len(), 2);
        assert_eq!(l.layers[1].props.name, "Linien");
        assert_eq!(l.layers[1].props.blend, BlendMode::Multiply);
        assert_eq!(l.layers[1].data, d.layers[1].data);
        assert!(l.paper.is_none());
        assert!(load(b"nope").is_err());
    }

    #[test]
    fn png_has_signature() {
        let d = Document::new(4, 4);
        let png = export_png(&d, true);
        assert_eq!(&png[1..4], b"PNG");
    }
}
