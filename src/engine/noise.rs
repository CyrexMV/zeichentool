/// Kachelbare Papierstruktur (256 x 256) für Bleistift, Kohle und Aquarell.
pub struct Noise {
    data: Vec<f32>,
}

const SIZE: usize = 256;

fn hash(x: u32, y: u32, seed: u32) -> f32 {
    let mut h = x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

impl Noise {
    pub fn new() -> Self {
        let mut data = vec![0f32; SIZE * SIZE];
        // (Zellen pro Kachel, Gewicht)
        let octaves: [(usize, f32); 4] = [(8, 0.18), (32, 0.27), (128, 0.30), (256, 0.25)];
        for (oi, &(cells, weight)) in octaves.iter().enumerate() {
            let cell = (SIZE / cells) as f32;
            for y in 0..SIZE {
                for x in 0..SIZE {
                    let fx = x as f32 / cell;
                    let fy = y as f32 / cell;
                    let ix = fx.floor() as u32;
                    let iy = fy.floor() as u32;
                    let tx = fx - fx.floor();
                    let ty = fy - fy.floor();
                    let sx = tx * tx * (3.0 - 2.0 * tx);
                    let sy = ty * ty * (3.0 - 2.0 * ty);
                    let c = cells as u32;
                    let h = |a: u32, b: u32| hash(a % c, b % c, oi as u32 + 1);
                    let a = h(ix, iy) + (h(ix + 1, iy) - h(ix, iy)) * sx;
                    let b = h(ix, iy + 1) + (h(ix + 1, iy + 1) - h(ix, iy + 1)) * sx;
                    data[y * SIZE + x] += (a + (b - a) * sy) * weight;
                }
            }
        }
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for v in &data {
            lo = lo.min(*v);
            hi = hi.max(*v);
        }
        for v in data.iter_mut() {
            let n = (*v - lo) / (hi - lo).max(1e-6);
            // etwas mehr Kontrast, damit die Körnung sichtbar wird
            *v = (n * 1.25 - 0.1).clamp(0.0, 1.0);
        }
        Noise { data }
    }

    #[inline]
    pub fn at(&self, x: i32, y: i32) -> f32 {
        self.data[(((y & 255) as usize) << 8) | (x & 255) as usize]
    }
}

impl Default for Noise {
    fn default() -> Self {
        Self::new()
    }
}
