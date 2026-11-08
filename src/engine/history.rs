use super::doc::Document;
use super::layer::{Layer, LayerProps};
use super::rect::IRect;
use miniz_oxide::deflate::compress_to_vec;
use miniz_oxide::inflate::decompress_to_vec;

pub fn pack(bytes: &[u8]) -> Vec<u8> {
    compress_to_vec(bytes, 1)
}

pub fn unpack(bytes: &[u8]) -> Vec<u8> {
    decompress_to_vec(bytes).unwrap_or_default()
}

/// Eine gespeicherte Ebene (komprimiert) für Löschen/Wiederherstellen.
#[derive(Clone, Debug)]
pub struct StoredLayer {
    pub id: u32,
    pub props: LayerProps,
    pub data: Vec<u8>,
}

impl StoredLayer {
    pub fn from_layer(l: &Layer) -> Self {
        StoredLayer { id: l.id, props: l.props.clone(), data: pack(&l.data) }
    }

    pub fn to_layer(&self, w: usize, h: usize) -> Layer {
        let mut data = unpack(&self.data);
        data.resize(w * h * 4, 0);
        Layer { id: self.id, props: self.props.clone(), data, rev: 1 }
    }
}

/// Ein rückgängig machbarer Schritt.
#[derive(Clone, Debug)]
pub enum Action {
    Patch { layer: u32, rect: IRect, before: Vec<u8>, after: Vec<u8> },
    AddLayer { index: usize, layer: StoredLayer },
    RemoveLayer { index: usize, layer: StoredLayer },
    MoveLayer { from: usize, to: usize },
    Props { layer: u32, before: LayerProps, after: LayerProps },
    Compound(Vec<Action>),
}

impl Action {
    fn bytes(&self) -> usize {
        match self {
            Action::Patch { before, after, .. } => before.len() + after.len() + 64,
            Action::AddLayer { layer, .. } | Action::RemoveLayer { layer, .. } => layer.data.len() + 64,
            Action::Compound(v) => v.iter().map(|a| a.bytes()).sum(),
            _ => 64,
        }
    }

    /// Erstellt einen Patch aus Vorher/Nachher-Pixeln eines Bereichs.
    pub fn patch(layer: u32, rect: IRect, before: &[u8], after: &[u8]) -> Action {
        Action::Patch { layer, rect, before: pack(before), after: pack(after) }
    }

    fn apply(&self, doc: &mut Document, undo: bool) {
        match self {
            Action::Patch { layer, rect, before, after } => {
                if let Some(i) = doc.index_of(*layer) {
                    let bytes = unpack(if undo { before } else { after });
                    if bytes.len() == rect.area() * 4 {
                        doc.write_region(i, *rect, &bytes);
                    }
                }
            }
            Action::AddLayer { index, layer } | Action::RemoveLayer { index, layer } => {
                let insert = matches!(self, Action::AddLayer { .. }) != undo;
                if insert {
                    let idx = (*index).min(doc.layers.len());
                    doc.layers.insert(idx, layer.to_layer(doc.w, doc.h));
                    doc.active = idx;
                } else if let Some(i) = doc.index_of(layer.id) {
                    doc.layers.remove(i);
                    if doc.layers.is_empty() {
                        doc.add_layer("Ebene 1", 0);
                    }
                    doc.active = i.saturating_sub(1).min(doc.layers.len() - 1);
                }
            }
            Action::MoveLayer { from, to } => {
                let (f, t) = if undo { (*to, *from) } else { (*from, *to) };
                if f < doc.layers.len() && t < doc.layers.len() {
                    let l = doc.layers.remove(f);
                    doc.layers.insert(t, l);
                    doc.active = t;
                }
            }
            Action::Props { layer, before, after } => {
                if let Some(i) = doc.index_of(*layer) {
                    doc.layers[i].props = if undo { before.clone() } else { after.clone() };
                }
            }
            Action::Compound(list) => {
                if undo {
                    for a in list.iter().rev() {
                        a.apply(doc, true);
                    }
                } else {
                    for a in list {
                        a.apply(doc, false);
                    }
                }
            }
        }
    }

    /// Ebene, die nach dem Schritt aktiv sein sollte.
    fn focus_layer(&self) -> Option<u32> {
        match self {
            Action::Patch { layer, .. } | Action::Props { layer, .. } => Some(*layer),
            Action::Compound(v) => v.iter().rev().find_map(|a| a.focus_layer()),
            _ => None,
        }
    }
}

/// Verlauf mit Speicherbegrenzung.
pub struct History {
    undo: Vec<Action>,
    redo: Vec<Action>,
    bytes: usize,
    pub max_bytes: usize,
    pub max_steps: usize,
}

impl Default for History {
    fn default() -> Self {
        History { undo: Vec::new(), redo: Vec::new(), bytes: 0, max_bytes: 384 << 20, max_steps: 120 }
    }
}

impl History {
    pub fn push(&mut self, a: Action) {
        self.bytes += a.bytes();
        self.undo.push(a);
        for r in self.redo.drain(..) {
            self.bytes = self.bytes.saturating_sub(r.bytes());
        }
        while (self.bytes > self.max_bytes || self.undo.len() > self.max_steps) && self.undo.len() > 1 {
            let old = self.undo.remove(0);
            self.bytes = self.bytes.saturating_sub(old.bytes());
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, doc: &mut Document) -> bool {
        match self.undo.pop() {
            Some(a) => {
                a.apply(doc, true);
                focus(doc, &a);
                self.redo.push(a);
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self, doc: &mut Document) -> bool {
        match self.redo.pop() {
            Some(a) => {
                a.apply(doc, false);
                focus(doc, &a);
                self.undo.push(a);
                true
            }
            None => false,
        }
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.bytes = 0;
    }
}

fn focus(doc: &mut Document, a: &Action) {
    if let Some(i) = a.focus_layer().and_then(|id| doc.index_of(id)) {
        doc.active = i;
    }
    doc.active = doc.active.min(doc.layers.len().saturating_sub(1));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_undo_redo() {
        let mut d = Document::new(8, 8);
        let id = d.layers[0].id;
        let r = IRect::new(1, 1, 3, 3);
        let before = d.read_region(0, r);
        let after = vec![200u8; r.area() * 4];
        d.write_region(0, r, &after);
        let mut h = History::default();
        h.push(Action::patch(id, r, &before, &after));
        assert!(h.undo(&mut d));
        assert_eq!(d.read_region(0, r), before);
        assert!(h.redo(&mut d));
        assert_eq!(d.read_region(0, r), after);
    }

    #[test]
    fn add_remove_layer_undo() {
        let mut d = Document::new(4, 4);
        let idx = d.add_layer("Neu", 1);
        let stored = StoredLayer::from_layer(&d.layers[idx]);
        let mut h = History::default();
        h.push(Action::AddLayer { index: idx, layer: stored });
        assert_eq!(d.layers.len(), 2);
        h.undo(&mut d);
        assert_eq!(d.layers.len(), 1);
        h.redo(&mut d);
        assert_eq!(d.layers.len(), 2);
        assert_eq!(d.layers[1].props.name, "Neu");
    }
}
