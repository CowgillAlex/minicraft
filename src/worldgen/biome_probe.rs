use super::{
    biomes,
    density::{Density, Sampler},
};
use std::sync::Arc;

/// Debug queries reuse the generator's four-metre biome grid. Only entering a
/// new cell evaluates noise; vertical travel reuses the horizontal candidates.
pub struct BiomeProbe {
    density: Density,
    table: Arc<biomes::Table>,
    column: Option<([i32; 2], biomes::Column)>,
    cell: Option<[i32; 3]>,
    biome: biomes::Id,
}
impl BiomeProbe {
    pub fn new(seed: i64) -> Result<Self, String> {
        Ok(Self {
            density: Density::new(seed)?,
            table: biomes::Table::shared()?,
            column: None,
            cell: None,
            biome: 0,
        })
    }
    pub fn update(&mut self, point: [f64; 3]) {
        let cell = [
            (point[0] / 4.0).floor() as i32,
            ((point[1] + 64.0) / 4.0).floor().clamp(0.0, 95.0) as i32,
            (point[2] / 4.0).floor() as i32,
        ];
        if self.cell == Some(cell) {
            return;
        }
        let xz = [cell[0], cell[2]];
        let mut sampler = Sampler::new(&self.density);
        let column = match self.column {
            Some((key, column)) if key == xz => column,
            _ => {
                let climate = std::array::from_fn(|i| {
                    sampler.eval(
                        self.density.climate[i],
                        [(cell[0] * 4 + 2) as f64, 63.0, (cell[2] * 4 + 2) as f64],
                    )
                });
                let column = self.table.column(climate);
                self.column = Some((xz, column));
                column
            }
        };
        let depth = sampler.eval(
            self.density.climate[4],
            [
                (cell[0] * 4 + 2) as f64,
                (-64 + cell[1] * 4 + 2) as f64,
                (cell[2] * 4 + 2) as f64,
            ],
        );
        self.biome = self.table.sample(column, depth);
        self.cell = Some(cell);
    }
    pub fn name(&self) -> &str {
        self.table.name(self.biome)
    }
}
