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
            for y in 0..CHUNK {
                for z in 0..CHUNK {
                    for x in 0..CHUNK {
                        let point = preview.point([
                            origin[0] + x as i32,
                            origin[1] + y as i32,
                            origin[2] + z as i32,
                        ]);
                        let mut value = prepared.sample(point);
                        if value.abs() < 1e-10 {
                            value = sampler.density(point);
                        }
                        cells[x + CHUNK * (z + CHUNK * y)] =
                            sampler.prepared_material(point, value, fluid);
                    }
                }
            }
            return Generated {
                data: Chunk::from_cells(cells),
                evaluated: CHUNK.pow(3),
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
