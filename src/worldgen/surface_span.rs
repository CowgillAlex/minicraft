//! Fold biome constants once and prove constant results over solid vertical runs.
use super::*;
impl Condition {
    fn fixed(&self, biome: Id, cold: bool) -> Option<bool> {
        match self {
            Self::Biome(ids) => Some(ids.contains(&biome)),
            Self::Temperature => Some(cold),
            Self::Not(c) => c.fixed(biome, cold).map(|v| !v),
            _ => None,
        }
    }
    fn span(&self, c: &mut impl Context, low: f64) -> Option<bool> {
        let high = c.y();
        let agree = |a: bool, b: bool| (a == b).then_some(a);
        match self {
            Self::Not(condition) => condition.span(c, low).map(|v| !v),
            Self::Y {
                anchor,
                mult,
                depth,
            } => {
                let offset = if *depth { c.above() } else { 0. };
                let a = if *depth { high + offset } else { low };
                let threshold = anchor + c.surface_depth() * mult;
                agree(a >= threshold, high + offset >= threshold)
            }
            Self::Water {
                offset,
                mult,
                depth,
            } => match c.water() {
                None => Some(*offset <= 0.),
                Some(w) => {
                    let d = if *depth { c.above() } else { 0. };
                    let a = if *depth { high + d } else { low };
                    let threshold = w + offset + c.surface_depth() * mult;
                    agree(a >= threshold, high + d >= threshold)
                }
            },
            Self::Stone {
                offset,
                surface,
                secondary,
                ceiling,
            } => {
                let s = if *surface { c.surface_depth() } else { 0. };
                let second = if *secondary == 0. {
                    0.
                } else {
                    ((c.secondary() + 1.) * 0.5 * secondary) as i32 as f64
                };
                let threshold = 1. + offset + s + second;
                let d = if *ceiling { c.below() } else { c.above() };
                let other = if *ceiling {
                    d - (high - low)
                } else {
                    d + (high - low)
                };
                agree(d <= threshold, other <= threshold)
            }
            Self::Preliminary => {
                let threshold = c.preliminary();
                agree(low >= threshold, high >= threshold)
            }
            Self::Gradient {
                low: lo, high: hi, ..
            } => {
                if high <= *lo {
                    Some(true)
                } else if low >= *hi {
                    Some(false)
                } else {
                    None
                }
            }
            _ => Some(self.test(c)),
        }
    }
}
impl Rule {
    pub fn specialize(&self, biome: Id, cold: bool) -> Option<Self> {
        match self {
            Self::If(c, r) => match c.fixed(biome, cold) {
                Some(false) => None,
                Some(true) => r.specialize(biome, cold),
                None => r
                    .specialize(biome, cold)
                    .map(|r| Self::If(c.clone(), Box::new(r))),
            },
            Self::Sequence(rules) => {
                let mut out = Vec::new();
                for rule in rules {
                    if let Some(r) = rule.specialize(biome, cold) {
                        let stop = matches!(r, Self::Block(_) | Self::Bands);
                        out.push(r);
                        if stop {
                            break;
                        }
                    }
                }
                match out.len() {
                    0 => None,
                    1 => out.pop(),
                    _ => Some(Self::Sequence(out)),
                }
            }
            _ => Some(self.clone()),
        }
    }
    pub fn uniform_span(&self, c: &mut impl Context, low: f64) -> Option<Option<u8>> {
        match self {
            Self::Block(m) => Some(Some(*m)),
            Self::Bands => None,
            Self::If(condition, r) => {
                if condition.span(c, low)? {
                    r.uniform_span(c, low)
                } else {
                    Some(None)
                }
            }
            Self::Sequence(rules) => {
                for r in rules {
                    let result = r.uniform_span(c, low)?;
                    if result.is_some() {
                        return Some(result);
                    }
                }
                Some(None)
            }
        }
    }
}
