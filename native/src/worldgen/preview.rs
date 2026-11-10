use super::Generator;
#[cfg(test)]
use super::TerrainSampler;
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
    pub height_m: i32,
    pub reference: bool,
    pub infinite: bool,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            seed: 1,
            origin: [120, -64, -20],
            height_m: 384,
            reference: false,
            infinite: true,
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
                if let Some(height) = value("--height")? {
                    preview.height_m = height
                        .parse()
                        .map_err(|_| "--height needs a number of metres")?;
                } else if value("--origin")?.is_some() {
                    preview.height_m = 12;
                }
                if preview.height_m != 384 {
                    preview.origin[1] = 58;
                }
                preview.infinite = !args
                    .iter()
                    .any(|arg| matches!(arg.as_str(), "--finite" | "--worldgen-benchmark"));
                preview.reference = args.iter().any(|v| v == "--reference-worldgen");
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
                if preview.height_m < 2 || preview.height_m > 384 || preview.height_m % 2 != 0 {
                    return Err(
                        "--height must be an even number of metres between 2 and 384".into(),
                    );
                }
                if !(-64..=320 - preview.height_m).contains(&preview.origin[1]) {
                    return Err(
                        "The preview must fit in the -64 to 320 metre vertical range".into(),
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

    pub fn build_with_progress(self, progress: &crate::startup::Progress) -> Result<World, String> {
        progress.check()?;
        match self {
            Self::Noise(preview) => preview.build_with_progress(progress),
            Self::Village => Ok(World::generate()),
        }
    }
}

impl Preview {
    pub fn point(self, cell: [i32; 3]) -> [f64; 3] {
        std::array::from_fn(|i| f64::from(self.origin[i]) + f64::from(cell[i]) / 16.0)
    }

    pub fn build_with_progress(self, progress: &crate::startup::Progress) -> Result<World, String> {
        progress.check()?;
        if self.infinite {
            return crate::terrain_region::initial(self, progress);
        }
        progress.begin(
            crate::startup::Stage::Terrain,
            world::NX * self.height_m as usize / 2 * world::NZ,
        );
        let generator = Generator::new(self.seed)?;
        let workers = thread::available_parallelism()
            .map_or(1, usize::from)
            .saturating_sub(2)
            .clamp(1, 8);
        let next = AtomicUsize::new(0);
        let mut world = World::with_height(self.height_m * 16);
        let chunks_y = world.chunks_y();
        let mut evaluated = 0_usize;
        let mut uniform = 0_usize;
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
                        if progress.check().is_err() {
                            break;
                        }
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        if index >= world::NX * chunks_y * world::NZ {
                            break;
                        }
                        let chunk = [
                            index % world::NX,
                            index / (world::NX * world::NZ),
                            index / world::NX % world::NZ,
                        ];
                        let data = super::page::generate(
                            self,
                            &mut sampler,
                            chunk.map(|v| v as i32 * CHUNK as i32),
                        );
                        if sender.send((chunk, data)).is_err() {
                            break;
                        }
                    }
                });
            }
            drop(sender);
            for (chunk, data) in &receiver {
                if progress.check().is_err() {
                    break;
                }
                evaluated += data.evaluated;
                uniform += usize::from(data.evaluated == 0);
                world.insert_chunk(chunk, data.data);
                progress.advance();
            }
            drop(receiver);
        });
        progress.check()?;
        println!(
            "Generation: {evaluated} fine samples, {uniform} uniform pages, {} metres tall",
            self.height_m
        );
        Ok(world)
    }

    #[cfg(test)]
    pub(super) fn page(self, sampler: &mut TerrainSampler<'_>, origin: [i32; 3]) -> Vec<u8> {
        super::page::generate(self, sampler, origin).data.to_vec()
    }

    pub fn benchmark(self) -> Result<(), String> {
        let start = Instant::now();
        let generator = Generator::new(self.seed)?;
        let mut sampler = generator.sampler();
        let cells = super::page::reference(self, &mut sampler, [0; 3]);
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
