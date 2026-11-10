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
