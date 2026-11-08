use super::rect::IRect;

/// Deckungsmaske eines laufenden Strichs (16 Bit pro Pixel).
pub struct Mask {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u16>,
    /// Bereich, der seit dem letzten `clear` verändert wurde.
    pub dirty: IRect,
}

impl Mask {
    pub fn new(w: usize, h: usize) -> Self {
        Mask { w, h, data: vec![0; w * h], dirty: IRect::EMPTY }
    }

    pub fn bounds(&self) -> IRect {
        IRect::full(self.w, self.h)
    }

    /// Setzt nur den benutzten Bereich zurück (schnell auch bei großen Leinwänden).
    pub fn clear(&mut self) {
        let r = self.dirty.clamp(self.w, self.h);
        for y in r.y0..r.y1 {
            let row = y as usize * self.w;
            self.data[row + r.x0 as usize..row + r.x1 as usize].fill(0);
        }
        self.dirty = IRect::EMPTY;
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> f32 {
        self.data[y * self.w + x] as f32 * (1.0 / 65535.0)
    }

    pub fn resize(&mut self, w: usize, h: usize) {
        *self = Mask::new(w, h);
    }

    /// Füllt einen Bereich mit Werten aus einer 8-Bit-Maske (für Füllwerkzeug).
    pub fn set_from_u8(&mut self, x: usize, y: usize, v: u8) {
        let i = y * self.w + x;
        let nv = v as u16 * 257;
        if nv > self.data[i] {
            self.data[i] = nv;
        }
    }
}
