/// Paper-Minecraft's xoroshiro128+ stream, including its seed upgrade and MD5 names.
#[derive(Clone)]
pub struct Random {
    lo: u64,
    hi: u64,
}

impl Random {
    pub fn new(seed: i64) -> Self {
        let lo = mix(seed as u64);
        Self {
            lo,
            hi: mix(lo.wrapping_add(0x9e3779b97f4a7c15)),
        }
    }

    pub fn named(&self, name: &str) -> Self {
        let bytes = md5::compute(name.as_bytes()).0;
        Self {
            lo: u64::from_be_bytes(bytes[..8].try_into().unwrap()) ^ self.lo,
            hi: u64::from_be_bytes(bytes[8..].try_into().unwrap()) ^ self.hi,
        }
    }

    pub fn next(&mut self) -> u64 {
        let result = self.lo.wrapping_add(self.hi);
        let hi = self.hi ^ self.lo;
        self.lo = self.lo.rotate_left(55) ^ hi ^ (hi << 14);
        self.hi = hi.rotate_left(36);
        result
    }

    pub fn int(&mut self, max: u32) -> usize {
        let threshold = 0x100000000_u64 % u64::from(max);
        loop {
            let value = self.next() >> 32;
            if value >= threshold {
                return (value % u64::from(max)) as usize;
            }
        }
    }

    pub fn double(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / (1_u64 << 53) as f64)
    }

    pub fn fork(&mut self) -> Self {
        Self {
            lo: self.next(),
            hi: self.next(),
        }
    }
}

fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul((-4658895280553007687_i64) as u64);
    value = (value ^ (value >> 27)).wrapping_mul((-7723592293110705685_i64) as u64);
    value ^ (value >> 31)
}

/// java.util.Random's 48-bit stream, used by the source terracotta bands.
pub(super) struct JavaRandom(u64);
impl JavaRandom {
    pub fn new(seed: i64) -> Self {
        Self((seed as u64 ^ 0x5deece66d) & ((1_u64 << 48) - 1))
    }
    fn bits(&mut self, count: u32) -> u32 {
        self.0 = self.0.wrapping_mul(0x5deece66d).wrapping_add(11) & ((1_u64 << 48) - 1);
        (self.0 >> (48 - count)) as u32
    }
    pub fn int(&mut self, bound: i32) -> usize {
        if (bound as u32).is_power_of_two() {
            return ((i64::from(bound) * i64::from(self.bits(31))) >> 31) as usize;
        }
        loop {
            let bits = self.bits(31) as i32;
            let value = bits % bound;
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return value as usize;
            }
        }
    }
    pub fn boolean(&mut self) -> bool {
        self.bits(1) != 0
    }
}
pub(super) fn string_hash(s: &str) -> i64 {
    i64::from(
        s.encode_utf16()
            .fold(0_i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32)),
    )
}
pub(super) fn positional(seed: i64, name: &str, [x, y, z]: [i32; 3]) -> f64 {
    let h = (seed ^ string_hash(name))
        .wrapping_add(i64::from(x).wrapping_mul(341873128712))
        .wrapping_add(i64::from(y).wrapping_mul(132897987541))
        .wrapping_add(i64::from(z).wrapping_mul(42317861));
    (mix(h as u64) >> 11) as f64 / (1_u64 << 53) as f64
}
