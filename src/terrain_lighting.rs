//! Lighting runs beside geometry preparation, using a bounded shared CPU budget.
use super::{Shared, Update};
use crate::{
    terrain_budget::View,
    terrain_region::{self, Builder, Region},
    world::World,
    worldgen::{features::Shared as Features, sky::Sky, Generator},
};
use std::sync::{atomic::Ordering, mpsc, Arc};

pub(super) struct Job {
    pub region: Region,
    pub world: World,
}
pub(super) fn spawn(
    world: World,
    sky: Sky,
    features: Arc<Features>,
    shared: Arc<Shared>,
    sender: mpsc::SyncSender<Result<Update, String>>,
    pages: crate::terrain_pages::Pages,
) {
    std::thread::Builder::new()
        .name("terrain-lighting".into())
        .spawn(move || {
            let failure = sender.clone();
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run(world, sky, features, &shared, &sender, pages)
            }))
            .is_err()
            {
                let _ = failure.send(Err("Terrain lighting worker stopped".into()));
            }
        })
        .expect("terrain lighting worker");
}
fn run(
    world: World,
    sky: Sky,
    features: Arc<Features>,
    shared: &Arc<Shared>,
    sender: &mpsc::SyncSender<Result<Update, String>>,
    pages: crate::terrain_pages::Pages,
) {
    let mut generator = Generator::new(world.noise_preview.expect("procedural light").seed)
        .expect("lighting generator");
    generator.feature_plans = features;
    let mut builder = Builder::new(world, &generator, sky);
    builder.set_page_pool(pages);
    loop {
        // Always lock request before jobs; request changes can reprioritise the queue.
        let request = shared.request.lock().unwrap();
        if shared.stop.load(Ordering::Acquire) {
            return;
        }
        let view = View {
            position: request.position,
            direction: request.direction,
            travel: request.travel,
            radius: request.radius,
        };
        let mut jobs = shared.light_jobs.lock().unwrap();
        jobs.retain(|job| {
            job.region.revision == request.revision
                && shared.current(job.region.key, job.region.revision, job.region.ticket)
                && terrain_region::visible(job.region.key, view.position, view.radius)
        });
        let index = jobs
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                view.priority(a.region.key)
                    .total_cmp(&view.priority(b.region.key))
            })
            .map(|(i, _)| i);
        let Some(index) = index else {
            drop(request);
            while jobs.is_empty() && !shared.stop.load(Ordering::Acquire) {
                jobs = shared.light_wake.wait(jobs).unwrap();
            }
            continue;
        };
        let mut job = jobs.swap_remove(index);
        job.region.timing.light_started = Some(std::time::Instant::now());
        shared.light_active.store(true, Ordering::Release);
        let dirty: Vec<_> = shared.light_dirty.lock().unwrap().drain().collect();
        drop(jobs);
        drop(request);
        let key = job.region.key;
        let revision = job.region.revision;
        let ticket = job.region.ticket;
        builder.lighting_snapshot(job.world, &dirty);
        let result = builder.finish_lighting(job.region, view.position, || {
            shared.stop.load(Ordering::Acquire) || !shared.current(key, revision, ticket)
        });
        shared.light_active.store(false, Ordering::Release);
        if let Some(region) = result {
            if !shared.send(sender, Update::Region(region)) {
                return;
            }
        } else if !shared.send(
            sender,
            Update::Cancelled {
                key,
                revision,
                ticket,
            },
        ) {
            return;
        }
        shared.wake.notify_all();
    }
}
