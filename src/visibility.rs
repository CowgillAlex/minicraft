//! Conservative current-frame occlusion from solid, resident two-metre pages.
//! Only fully covered tiles count; near-plane crossings and partial coverage stay visible.
//! No previous-frame visibility is reused, so camera turns cannot expose stale holes.
use glam::{Mat4, Vec2, Vec3};
use std::cell::Cell;
const W: usize = 64;
const H: usize = 36;
pub struct Visibility {
    matrix: Mat4,
    depth: [f32; W * H],
    covered: bool,
    pub rejected: Cell<usize>,
}
fn cross(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    (b - a).perp_dot(c - a)
}
impl Visibility {
    pub fn new(matrix: Mat4) -> Self {
        Self {
            matrix,
            depth: [1.0; W * H],
            covered: false,
            rejected: Cell::new(0),
        }
    }
    fn project(&self, center: Vec3, half: Vec3) -> Option<([Vec2; 8], f32, f32)> {
        let mut points = [Vec2::ZERO; 8];
        let (mut near, mut far) = (1.0_f32, 0.0_f32);
        for (i, point) in points.iter_mut().enumerate() {
            let offset = Vec3::new(
                if i & 1 == 0 { -half.x } else { half.x },
                if i & 2 == 0 { -half.y } else { half.y },
                if i & 4 == 0 { -half.z } else { half.z },
            );
            let clip = self.matrix * (center + offset).extend(1.0);
            if clip.w <= 0.0 {
                return None;
            }
            let ndc = clip.truncate() / clip.w;
            if !ndc.is_finite() || ndc.z <= 0.0 || ndc.z >= 1.0 {
                return None;
            }
            *point = (ndc.truncate() * 0.5 + Vec2::splat(0.5)) * Vec2::new(W as f32, H as f32);
            near = near.min(ndc.z);
            far = far.max(ndc.z);
        }
        Some((points, near, far))
    }
    pub fn occluder(&mut self, center: Vec3) {
        let Some((mut points, _, depth)) = self.project(center, Vec3::splat(0.999)) else {
            return;
        };
        points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
        let mut hull = Vec::with_capacity(16);
        for p in points {
            while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
                hull.pop();
            }
            hull.push(p);
        }
        let lower = hull.len();
        for p in points.into_iter().rev().skip(1) {
            while hull.len() > lower && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0
            {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
        let min = points
            .iter()
            .copied()
            .fold(Vec2::splat(f32::INFINITY), Vec2::min)
            .floor()
            .max(Vec2::ZERO);
        let max = points
            .iter()
            .copied()
            .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max)
            .ceil()
            .min(Vec2::new(W as f32, H as f32));
        for y in min.y as usize..max.y.max(0.0) as usize {
            for x in min.x as usize..max.x.max(0.0) as usize {
                let corners = [
                    Vec2::new(x as f32, y as f32),
                    Vec2::new(x as f32 + 1.0, y as f32),
                    Vec2::new(x as f32 + 1.0, y as f32 + 1.0),
                    Vec2::new(x as f32, y as f32 + 1.0),
                ];
                if corners.iter().all(|&p| {
                    (0..hull.len()).all(|i| cross(hull[i], hull[(i + 1) % hull.len()], p) > 0.001)
                }) {
                    self.depth[y * W + x] = self.depth[y * W + x].min(depth);
                    self.covered = true;
                }
            }
        }
    }
    pub fn hidden(&self, center: Vec3, half: Vec3) -> bool {
        if !self.covered {
            return false;
        }
        // The projected centre is inside the projected bounds. If even that
        // tile is not covered in front of the centre, the whole box cannot be
        // occluded. Avoid eight corner transforms for the common visible case.
        let clip = self.matrix * center.extend(1.0);
        if clip.w <= 0.0 {
            return false;
        }
        let ndc = clip.truncate() / clip.w;
        let tile = (ndc.truncate() * 0.5 + Vec2::splat(0.5)) * Vec2::new(W as f32, H as f32);
        if !tile.is_finite()
            || tile.x < 0.0
            || tile.y < 0.0
            || tile.x >= W as f32
            || tile.y >= H as f32
        {
            return false;
        }
        if self.depth[tile.y as usize * W + tile.x as usize] + 0.00001 >= ndc.z {
            return false;
        }
        let Some((points, near, _)) = self.project(center, half) else {
            return false;
        };
        let min = points
            .iter()
            .copied()
            .fold(Vec2::splat(f32::INFINITY), Vec2::min)
            .floor();
        let max = points
            .iter()
            .copied()
            .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max)
            .ceil();
        if min.x < 0.0 || min.y < 0.0 || max.x > W as f32 || max.y > H as f32 {
            return false;
        }
        let hidden = (min.y as usize..max.y as usize).all(|y| {
            (min.x as usize..max.x as usize).all(|x| self.depth[y * W + x] + 0.00001 < near)
        });
        if hidden {
            self.rejected.set(self.rejected.get() + 1);
        }
        hidden
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_only_covered_geometry_and_never_near_plane_crossings() {
        let mut v = Visibility::new(Mat4::perspective_rh(1.0, 16.0 / 9.0, 0.03, 512.0));
        v.occluder(Vec3::new(0.0, 0.0, -3.0));
        assert!(v.hidden(Vec3::new(0.0, 0.0, -10.0), Vec3::splat(0.5)));
        assert!(!v.hidden(Vec3::new(0.0, 0.0, -3.0), Vec3::ONE));
        assert!(!v.hidden(Vec3::new(4.0, 0.0, -10.0), Vec3::ONE));
        assert!(!v.hidden(Vec3::ZERO, Vec3::ONE));
        assert!(!v.hidden(Vec3::new(0.0, 0.0, -10.0), Vec3::splat(8.0)));
    }
}
