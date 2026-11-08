use super::mask::Mask;
use super::rect::IRect;

#[allow(clippy::too_many_arguments)]
/// Füllt die zusammenhängende Fläche um `(sx, sy)` in `src` (RGBA8, w×h)
/// und schreibt das Ergebnis in die Maske. `grow` erweitert die Fläche um
/// einige Pixel, damit keine hellen Ränder an geglätteten Linien bleiben.
pub fn flood_fill(src: &[u8], w: usize, h: usize, sx: i32, sy: i32, tolerance: f32, grow: i32, mask: &mut Mask) -> IRect {
    if sx < 0 || sy < 0 || sx >= w as i32 || sy >= h as i32 {
        return IRect::EMPTY;
    }
    let seed_i = (sy as usize * w + sx as usize) * 4;
    let seed = [src[seed_i], src[seed_i + 1], src[seed_i + 2], src[seed_i + 3]];
    let tol = (tolerance.clamp(0.0, 1.0) * 255.0) as i32;
    let matches = |i: usize| -> bool {
        let p = i * 4;
        (0..4).all(|c| (src[p + c] as i32 - seed[c] as i32).abs() <= tol)
    };
    let mut filled = vec![false; w * h];
    let mut stack = vec![(sx as usize, sy as usize)];
    let mut bb = IRect::EMPTY;
    while let Some((x, y)) = stack.pop() {
        let row = y * w;
        if filled[row + x] || !matches(row + x) {
            continue;
        }
        let mut l = x;
        while l > 0 && !filled[row + l - 1] && matches(row + l - 1) {
            l -= 1;
        }
        let mut r = x;
        while r + 1 < w && !filled[row + r + 1] && matches(row + r + 1) {
            r += 1;
        }
        for i in l..=r {
            filled[row + i] = true;
        }
        bb = bb.union(&IRect::new(l as i32, y as i32, r as i32 + 1, y as i32 + 1));
        for ny in [y.wrapping_sub(1), y + 1] {
            if ny >= h {
                continue;
            }
            let nrow = ny * w;
            let mut i = l;
            while i <= r {
                if !filled[nrow + i] && matches(nrow + i) {
                    stack.push((i, ny));
                    // überspringe den Rest dieses Laufs
                    while i <= r && !filled[nrow + i] && matches(nrow + i) {
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
        }
    }
    if bb.is_empty() {
        return bb;
    }
    // Erweitern (Dilatation) um `grow` Pixel
    let mut cur = filled;
    let mut area = bb;
    for _ in 0..grow.max(0) {
        area = area.expand(1).clamp(w, h);
        let mut next = cur.clone();
        for y in area.y0..area.y1 {
            for x in area.x0..area.x1 {
                let i = y as usize * w + x as usize;
                if cur[i] {
                    continue;
                }
                let n = (x > 0 && cur[i - 1])
                    || (x + 1 < w as i32 && cur[i + 1])
                    || (y > 0 && cur[i - w])
                    || (y + 1 < h as i32 && cur[i + w]);
                if n {
                    next[i] = true;
                }
            }
        }
        cur = next;
    }
    for y in area.y0..area.y1 {
        for x in area.x0..area.x1 {
            if cur[y as usize * w + x as usize] {
                mask.set_from_u8(x as usize, y as usize, 255);
            }
        }
    }
    mask.dirty = mask.dirty.union(&area);
    area
}

/// Gefülltes, kantengeglättetes Polygon (Even-Odd-Regel) in die Maske zeichnen.
pub fn fill_polygon(mask: &mut Mask, pts: &[(f32, f32)]) -> IRect {
    if pts.len() < 3 {
        return IRect::EMPTY;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(x, y) in pts {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let r = IRect::new(x0.floor() as i32, y0.floor() as i32, x1.ceil() as i32 + 1, y1.ceil() as i32 + 1).intersect(&mask.bounds());
    if r.is_empty() {
        return r;
    }
    const SUB: usize = 4;
    let rw = r.width();
    let mut cov = vec![0f32; rw];
    let mut xs: Vec<f32> = Vec::new();
    for y in r.y0..r.y1 {
        cov.fill(0.0);
        for s in 0..SUB {
            let sy = y as f32 + (s as f32 + 0.5) / SUB as f32;
            xs.clear();
            for i in 0..pts.len() {
                let (ax, ay) = pts[i];
                let (bx, by) = pts[(i + 1) % pts.len()];
                if (ay <= sy && by > sy) || (by <= sy && ay > sy) {
                    xs.push(ax + (sy - ay) / (by - ay) * (bx - ax));
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for pair in xs.chunks_exact(2) {
                let a = (pair[0] - r.x0 as f32).max(0.0);
                let b = (pair[1] - r.x0 as f32).min(rw as f32);
                if b <= a {
                    continue;
                }
                let ia = a.floor() as usize;
                let ib = (b.ceil() as usize).min(rw);
                for (i, c) in cov.iter_mut().enumerate().take(ib).skip(ia) {
                    let l = (i as f32).max(a);
                    let rr = ((i + 1) as f32).min(b);
                    if rr > l {
                        *c += (rr - l) / SUB as f32;
                    }
                }
            }
        }
        let row = y as usize * mask.w;
        for (i, c) in cov.iter().enumerate() {
            if *c > 0.0 {
                let idx = row + r.x0 as usize + i;
                let v = (c.min(1.0) * 65535.0) as u16;
                if v > mask.data[idx] {
                    mask.data[idx] = v;
                }
            }
        }
    }
    mask.dirty = mask.dirty.union(&r);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_enclosed_area_only() {
        let (w, h) = (10, 10);
        let mut src = vec![0u8; w * h * 4];
        // senkrechte Wand bei x = 5
        for y in 0..h {
            src[(y * w + 5) * 4 + 3] = 255;
        }
        let mut m = Mask::new(w, h);
        flood_fill(&src, w, h, 1, 1, 0.1, 0, &mut m);
        assert!(m.get(0, 9) > 0.99);
        assert!(m.get(4, 0) > 0.99);
        assert_eq!(m.get(5, 5), 0.0);
        assert_eq!(m.get(7, 5), 0.0);
    }

    #[test]
    fn polygon_square() {
        let mut m = Mask::new(20, 20);
        fill_polygon(&mut m, &[(2.0, 2.0), (12.0, 2.0), (12.0, 12.0), (2.0, 12.0)]);
        assert!(m.get(5, 5) > 0.99);
        assert_eq!(m.get(15, 15), 0.0);
        assert_eq!(m.get(1, 1), 0.0);
    }
}
