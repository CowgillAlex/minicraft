use super::{preview::Preview, TerrainSampler};
use crate::{
    chunk::Chunk,
    world::{CHUNK, STONE},
};

pub struct Generated {
    pub data: Chunk,
    pub evaluated: usize,
}

pub fn generate(preview: Preview, sampler: &mut TerrainSampler<'_>, origin: [i32; 3]) -> Generated {
    let mut generated = generate_raw(preview, sampler, origin);
    if preview.surfaces && generated.data.occupied() > 0 {
        sampler.decorate(
            std::array::from_fn(|i| origin[i] + preview.origin[i] * 16),
            &mut generated.data,
        );
    }
    if preview.features {
        sampler.decorate_features(
            std::array::from_fn(|i| origin[i] + preview.origin[i] * 16),
            &mut generated.data,
        );
    }
    generated
}

pub fn generate_raw(
    preview: Preview,
    sampler: &mut TerrainSampler<'_>,
    origin: [i32; 3],
) -> Generated {
    if !preview.reference {
        let min = preview.point(origin);
        let max = preview.point(origin.map(|v| v + CHUNK as i32 - 1));
        if let Some(material) = sampler.uniform_material(min, max) {
            return Generated {
                data: Chunk::Uniform(material),
                evaluated: 0,
            };
        }
        if let Some(prepared) = sampler.prepared(min, max) {
            let fluid = sampler.open_fluid(min, max);
            let mut cells = vec![STONE; CHUNK.pow(3)];
            let evaluated = fill(
                preview, sampler, &prepared, fluid, origin, [0; 3], CHUNK, &mut cells,
            );
            return Generated {
                data: Chunk::from_cells(cells),
                evaluated,
            };
        }
    }
    Generated {
        data: Chunk::from_cells(reference(preview, sampler, origin)),
        evaluated: CHUNK.pow(3),
    }
}

/// The reference fill is retained for timing and byte-for-byte optimization checks.
pub fn reference(preview: Preview, sampler: &mut TerrainSampler<'_>, origin: [i32; 3]) -> Vec<u8> {
    let mut cells = vec![STONE; CHUNK.pow(3)];
    for y in 0..CHUNK {
        for z in 0..CHUNK {
            for x in 0..CHUNK {
                let cell = [
                    origin[0] + x as i32,
                    origin[1] + y as i32,
                    origin[2] + z as i32,
                ];
                cells[x + CHUNK * (z + CHUNK * y)] = sampler.material(preview.point(cell));
            }
        }
    }
    cells
}

/// Uniform subvolumes of a mixed page can still be proved exactly. Only the
/// uncertain bands around terrain and fluid boundaries need fine evaluation.
#[allow(clippy::too_many_arguments)]
fn fill(
    preview: Preview,
    sampler: &mut TerrainSampler<'_>,
    prepared: &super::fast_density::Prepared,
    fluid: Option<super::aquifer::Fluid>,
    origin: [i32; 3],
    local: [usize; 3],
    size: usize,
    cells: &mut [u8],
) -> usize {
    let min = std::array::from_fn(|i| origin[i] + local[i] as i32);
    let max = min.map(|v| v + size as i32 - 1);
    if size < CHUNK {
        if let Some(material) = sampler.uniform_material(preview.point(min), preview.point(max)) {
            for y in local[1]..local[1] + size {
                for z in local[2]..local[2] + size {
                    let start = local[0] + CHUNK * (z + CHUNK * y);
                    cells[start..start + size].fill(material);
                }
            }
            return 0;
        }
    }
    if size > 8 {
        let half = size / 2;
        let mut evaluated = 0;
        for y in [0, half] {
            for z in [0, half] {
                for x in [0, half] {
                    evaluated += fill(
                        preview,
                        sampler,
                        prepared,
                        fluid,
                        origin,
                        [local[0] + x, local[1] + y, local[2] + z],
                        half,
                        cells,
                    );
                }
            }
        }
        return evaluated;
    }
    for y in local[1]..local[1] + size {
        for z in local[2]..local[2] + size {
            for x in local[0]..local[0] + size {
                let point = preview.point([
                    origin[0] + x as i32,
                    origin[1] + y as i32,
                    origin[2] + z as i32,
                ]);
                let mut value = prepared.sample(point);
                if value.abs() < 1e-10 {
                    value = sampler.density(point);
                }
                cells[x + CHUNK * (z + CHUNK * y)] = sampler.prepared_material(point, value, fluid);
            }
        }
    }
    size.pow(3)
}
