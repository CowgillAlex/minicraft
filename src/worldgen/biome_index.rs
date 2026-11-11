//! Exact nearest-range search with lower bounds and original table tie order.
use super::{distance, Entry};
const AXES: [usize; 5] = [0, 1, 2, 3, 5];
pub(super) struct Node {
    bounds: [[f64; 2]; 6],
    contents: Contents,
}
enum Contents {
    Leaf(Vec<usize>),
    Branch(Box<[Node; 2]>),
}
impl Node {
    pub fn build(entries: &[Entry], mut ids: Vec<usize>) -> Self {
        let bounds = std::array::from_fn(|a| {
            [
                ids.iter()
                    .map(|&i| entries[i].ranges[a][0])
                    .fold(f64::INFINITY, f64::min),
                ids.iter()
                    .map(|&i| entries[i].ranges[a][1])
                    .fold(f64::NEG_INFINITY, f64::max),
            ]
        });
        let contents = if ids.len() <= 12 {
            Contents::Leaf(ids)
        } else {
            let axis = *AXES
                .iter()
                .max_by(|&&a, &&b| {
                    (bounds[a][1] - bounds[a][0]).total_cmp(&(bounds[b][1] - bounds[b][0]))
                })
                .unwrap();
            ids.sort_by(|&a, &b| {
                (entries[a].ranges[axis][0] + entries[a].ranges[axis][1])
                    .total_cmp(&(entries[b].ranges[axis][0] + entries[b].ranges[axis][1]))
            });
            let right = ids.split_off(ids.len() / 2);
            Contents::Branch(Box::new([
                Self::build(entries, ids),
                Self::build(entries, right),
            ]))
        };
        Self { bounds, contents }
    }
    fn lower(&self, climate: [f64; 6]) -> f64 {
        AXES.into_iter()
            .map(|a| distance(self.bounds[a], climate[a]).powi(2))
            .sum()
    }
    pub fn nearest(&self, entries: &[Entry], climate: [f64; 6]) -> Option<(usize, f64)> {
        let mut best = None;
        self.search(entries, climate, &mut best);
        best
    }
    fn search(&self, entries: &[Entry], climate: [f64; 6], best: &mut Option<(usize, f64)>) {
        if best.is_some_and(|(_, score)| self.lower(climate) > score) {
            return;
        }
        match &self.contents {
            Contents::Leaf(ids) => {
                for &id in ids {
                    let score = AXES
                        .into_iter()
                        .map(|a| distance(entries[id].ranges[a], climate[a]).powi(2))
                        .sum();
                    if best.is_none_or(|(i, s)| score < s || (score == s && id < i)) {
                        *best = Some((id, score));
                    }
                }
            }
            Contents::Branch(children) => {
                let first = usize::from(children[1].lower(climate) < children[0].lower(climate));
                children[first].search(entries, climate, best);
                children[1 - first].search(entries, climate, best);
            }
        }
    }
}
#[cfg(test)]
#[test]
fn indexed_candidates_match_table_order_at_random_points_and_edges() {
    let table = super::Table::shared().unwrap();
    let mut rng = crate::worldgen::random::Random::new(91234);
    for n in 0..6000 {
        let climate = if n < table.entries.len().min(3000) {
            table.entries[n].ranges.map(|r| r[n % 2])
        } else {
            std::array::from_fn(|_| rng.double() * 4. - 2.)
        };
        let mut expected = [None; 4];
        for (i, e) in table.entries.iter().enumerate() {
            let score = AXES
                .into_iter()
                .map(|a| distance(e.ranges[a], climate[a]).powi(2))
                .sum();
            if expected[e.band()].is_none_or(|(_, best)| score < best) {
                expected[e.band()] = Some((i, score));
            }
        }
        assert_eq!(table.column(climate).candidates, expected);
    }
}
