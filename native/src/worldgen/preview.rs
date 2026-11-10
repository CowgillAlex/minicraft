use super::{Generator, TerrainSampler};
use crate::world::{self, World, CHUNK};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Instant,
};

#[derive(Clone, Copy, Debug)]
pub struct Preview {
    pub seed: i64,
    /// Lower corner in the reference generator's metre coordinates.
    pub origin: [i32; 3],
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            seed: 1,
            origin: [120, 58, -20],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Scene {
    Noise(Preview),
    Village,
}

impl Scene {
    pub fn from_args(args: &[String]) -> Result<Self, String> {
        let value = |key: &str| -> Result<Option<&str>, String> {
            args.iter()
                .position(|a| a == key)
                .map(|i| {
                    args.get(i + 1)
                        .map(String::as_str)
                        .ok_or_else(|| format!("{key} needs a value"))
                })
                .transpose()
        };
        match value("--scene")?.unwrap_or("noise") {
            "village" => Ok(Self::Village),
            "noise" => {
                let mut preview = Preview::default();
                if let Some(seed) = value("--seed")? {
                    preview.seed = seed
                        .parse()
                        .map_err(|_| "--seed needs a signed 64-bit integer")?;
                }
                if let Some(origin) = value("--origin")? {
                    preview.origin = origin
                        .split(',')
                        .map(str::parse::<i32>)
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| "--origin needs integer X,Y,Z metre coordinates")?
                        .try_into()
                        .map_err(|_| "--origin needs exactly three coordinates")?;
                }
                if !(-64..=308).contains(&preview.origin[1]) {
                    return Err(
                        "--origin Y must be between -64 and 308 for this finite preview".into(),
                    );
                }
                if preview
                    .origin
                    .into_iter()
                    .any(|v| v.unsigned_abs() > 30_000_000)
                {
                    return Err("Preview coordinates must be within 30 million metres".into());
                }
                Ok(Self::Noise(preview))
            }
            _ => Err("--scene must be noise or village".into()),
        }
    }

    pub fn build(self) -> Result<World, String> {
        match self {
            Self::Noise(preview) => preview.build(),
            Self::Village => Ok(World::generate()),
        }
    }
}

impl Preview {
    pub fn point(self, cell: [i32; 3]) -> [f64; 3] {
        std::array::from_fn(|i| f64::from(self.origin[i]) + f64::from(cell[i]) / 16.0)
    }

    pub fn build(self) -> Result<World, String> {
        let generator = Generator::new(self.seed)?;
        let workers = thread::available_parallelism()
            .map_or(1, usize::from)
            .saturating_sub(2)
            .clamp(1, 8);
        let next = AtomicUsize::new(0);
        let mut world = World::empty();
        world.noise_preview = Some(self);
        println!(
            "Noise preview: seed {}, origin {:?} metres, {workers} workers",
            self.seed, self.origin
        );
        thread::scope(|scope| {
            let (sender, receiver) = mpsc::sync_channel(workers * 2);
            for _ in 0..workers {
                let sender = sender.clone();
                let generator = &generator;
                let next = &next;
                scope.spawn(move || {
                    let mut sampler = generator.sampler();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        if index >= world::NX * world::NY * world::NZ {
                            break;
                        }
                        let chunk = [
                            index % world::NX,
                            index / (world::NX * world::NZ),
                            index / world::NX % world::NZ,
                        ];
                        let data = self.page(&mut sampler, chunk.map(|v| v as i32 * CHUNK as i32));
                        if sender.send((chunk, data)).is_err() {
                            break;
                        }
                        sampler.clear_density_cache();
                    }
                });
            }
            drop(sender);
            for (chunk, data) in receiver {
                world.insert_chunk(chunk, data);
            }
        });
        Ok(world)
    }

    pub(super) fn page(self, sampler: &mut TerrainSampler<'_>, origin: [i32; 3]) -> Vec<u8> {
        let mut cells = vec![world::AIR; CHUNK.pow(3)];
        for y in 0..CHUNK {
            for z in 0..CHUNK {
                for x in 0..CHUNK {
                    let cell = [
                        origin[0] + x as i32,
                        origin[1] + y as i32,
                        origin[2] + z as i32,
                    ];
                    cells[x + CHUNK * (z + CHUNK * y)] = sampler.material(self.point(cell));
                }
            }
        }
        cells
    }

    pub fn benchmark(self) -> Result<(), String> {
        let start = Instant::now();
        let generator = Generator::new(self.seed)?;
        let mut sampler = generator.sampler();
        let cells = self.page(&mut sampler, [0; 3]);
        let elapsed = start.elapsed();
        let stone = cells.iter().filter(|v| **v == world::STONE).count();
        let water = cells.iter().filter(|v| **v == world::WATER).count();
        println!(
            "{} fine voxels in {:.3}s: {stone} stone, {water} water, {} air",
            cells.len(),
            elapsed.as_secs_f64(),
            cells.len() - stone - water
        );
        Ok(())
    }
}
