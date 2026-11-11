//! Lighting only asks for optical materials. Index exact feature shapes into
//! half-metre bins instead of rasterizing 32³ cells or generating ore deposits.
use super::{geometry, Primitive, TerrainSampler, V};
use std::sync::Arc;
pub(super) struct Page {
    bins: Box<[Vec<Primitive>; 64]>,
    base: Option<u8>,
}
impl super::Cache {
    pub(super) fn optical(
        &mut self,
        sampler: &mut TerrainSampler<'_>,
        p: [f64; 3],
        surfaces: bool,
    ) -> u8 {
        let cell = p.map(|v| (v * 16.).floor() as i32);
        let key = cell.map(|v| v.div_euclid(32));
        let page = if let Some((_, page)) = self
            .optical_last
            .as_ref()
            .filter(|(k, _)| *k == (key, surfaces))
        {
            page.clone()
        } else if let Some(page) = self.optics.get(&(key, surfaces)) {
            let page = page.clone();
            self.optical_last = Some(((key, surfaces), page.clone()));
            page
        } else {
            let origin = key.map(|v| v * 32);
            let min = origin.map(|v| v as f64 / 16.);
            let max = origin.map(|v| (v + 31) as f64 / 16.);
            let mut bins: Box<[Vec<Primitive>; 64]> = Box::new(std::array::from_fn(|_| Vec::new()));
            for shape in self
                .block(sampler, key.map(|v| v.div_euclid(4)), true)
                .iter()
            {
                if matches!(shape.replace, super::Replace::Rock) {
                    continue;
                }
                let (lo, hi) = shape.bounds();
                let lo = std::array::from_fn::<_, 3, _>(|i| {
                    ((lo[i] * 16.).floor() as i32 - origin[i]).max(0)
                });
                let hi = std::array::from_fn::<_, 3, _>(|i| {
                    ((hi[i] * 16.).ceil() as i32 - origin[i]).min(32)
                });
                if (0..3).any(|i| lo[i] >= hi[i]) {
                    continue;
                }
                for y in lo[1] / 8..=(hi[1] - 1) / 8 {
                    for z in lo[2] / 8..=(hi[2] - 1) / 8 {
                        for x in lo[0] / 8..=(hi[0] - 1) / 8 {
                            bins[(x + 4 * (z + 4 * y)) as usize].push(*shape);
                        }
                    }
                }
            }
            let page = Arc::new(Page {
                bins,
                base: sampler.light_uniform(min, max, surfaces),
            });
            self.optics.insert((key, surfaces), page.clone());
            self.optical_last = Some(((key, surfaces), page.clone()));
            page
        };
        let base = page
            .base
            .unwrap_or_else(|| sampler.light_material(p, surfaces));
        let local = cell.map(|v| v.rem_euclid(32) as usize / 8);
        let sample = (V::new(cell[0] as f64, cell[1] as f64, cell[2] as f64) + V::splat(0.5)) / 16.;
        let mut paint = 0;
        let mut priority = 0;
        for shape in &page.bins[local[0] + 4 * (local[2] + 4 * local[1])] {
            if shape.priority >= priority && shape.contains(sample) {
                priority = shape.priority;
                paint = u16::from(shape.material) | (u16::from(shape.replace as u8) << 8);
            }
        }
        geometry::apply(paint, base)
    }
}
