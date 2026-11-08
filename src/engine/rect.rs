/// Ganzzahliges Rechteck, `x1`/`y1` sind exklusiv.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct IRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl IRect {
    pub const EMPTY: IRect = IRect { x0: 0, y0: 0, x1: 0, y1: 0 };

    pub fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        IRect { x0, y0, x1, y1 }
    }

    pub fn full(w: usize, h: usize) -> Self {
        IRect::new(0, 0, w as i32, h as i32)
    }

    /// Rechteck, das einen Kreis (mit etwas Rand für Kantenglättung) umschließt.
    pub fn around(cx: f32, cy: f32, r: f32) -> Self {
        IRect::new(
            (cx - r).floor() as i32 - 1,
            (cy - r).floor() as i32 - 1,
            (cx + r).ceil() as i32 + 2,
            (cy + r).ceil() as i32 + 2,
        )
    }

    pub fn is_empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }

    pub fn width(&self) -> usize {
        if self.is_empty() {
            0
        } else {
            (self.x1 - self.x0) as usize
        }
    }

    pub fn height(&self) -> usize {
        if self.is_empty() {
            0
        } else {
            (self.y1 - self.y0) as usize
        }
    }

    pub fn union(&self, o: &IRect) -> IRect {
        if self.is_empty() {
            return *o;
        }
        if o.is_empty() {
            return *self;
        }
        IRect::new(
            self.x0.min(o.x0),
            self.y0.min(o.y0),
            self.x1.max(o.x1),
            self.y1.max(o.y1),
        )
    }

    pub fn intersect(&self, o: &IRect) -> IRect {
        let r = IRect::new(
            self.x0.max(o.x0),
            self.y0.max(o.y0),
            self.x1.min(o.x1),
            self.y1.min(o.y1),
        );
        if r.is_empty() {
            IRect::EMPTY
        } else {
            r
        }
    }

    pub fn clamp(&self, w: usize, h: usize) -> IRect {
        self.intersect(&IRect::full(w, h))
    }

    pub fn expand(&self, n: i32) -> IRect {
        if self.is_empty() {
            return *self;
        }
        IRect::new(self.x0 - n, self.y0 - n, self.x1 + n, self.y1 + n)
    }

    pub fn area(&self) -> usize {
        self.width() * self.height()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_and_intersect() {
        let a = IRect::new(0, 0, 10, 10);
        let b = IRect::new(5, 5, 20, 20);
        assert_eq!(a.union(&b), IRect::new(0, 0, 20, 20));
        assert_eq!(a.intersect(&b), IRect::new(5, 5, 10, 10));
        assert!(a.intersect(&IRect::new(30, 30, 40, 40)).is_empty());
        assert_eq!(IRect::EMPTY.union(&a), a);
    }
}
