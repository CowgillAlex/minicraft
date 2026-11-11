//! Greedy quads can meet at T-junctions. Insert existing edge vertices before
//! simplification so those junctions are not mistaken for open mesh borders.
use crate::mesh::{Mesh, Vertex};
use std::collections::HashSet;
fn key(p: [f32; 3]) -> [i32; 3] {
    p.map(|v| (v * 16.0).round() as i32)
}
fn between(a: Vertex, b: Vertex, t: f32) -> Vertex {
    Vertex {
        position: std::array::from_fn(|i| a.position[i] + (b.position[i] - a.position[i]) * t),
        uv: std::array::from_fn(|i| a.uv[i] + (b.uv[i] - a.uv[i]) * t),
        ..a
    }
}
pub fn conform(mesh: &mut Mesh) {
    let points: HashSet<_> = mesh.vertices.iter().map(|v| key(v.position)).collect();
    let mut indices = Vec::with_capacity(mesh.indices.len());
    for quad in mesh.indices.as_chunks::<6>().0 {
        let base = *quad.iter().min().unwrap();
        let corners = std::array::from_fn::<_, 4, _>(|i| mesh.vertices[base as usize + i]);
        let mut edge = Vec::new();
        let mut extra = false;
        for a in 0..4 {
            let b = (a + 1) % 4;
            edge.push(base + a as u32);
            let p = key(corners[a].position);
            let q = key(corners[b].position);
            let count = (0..3).map(|i| (q[i] - p[i]).unsigned_abs()).max().unwrap() as i32;
            for step in 1..count {
                let point = std::array::from_fn(|i| p[i] + (q[i] - p[i]) * step / count);
                if points.contains(&point) {
                    let mut v = between(corners[a], corners[b], step as f32 / count as f32);
                    v.position = point.map(|p| p as f32 / 16.0);
                    edge.push(mesh.vertices.len() as u32);
                    mesh.vertices.push(v);
                    extra = true;
                }
            }
        }
        if !extra {
            indices.extend_from_slice(quad);
            continue;
        }
        let center = mesh.vertices.len() as u32;
        mesh.vertices.push(between(corners[0], corners[2], 0.5));
        for i in 0..edge.len() {
            let a = edge[i];
            let b = edge[(i + 1) % edge.len()];
            if corners[0].normal.iter().sum::<f32>() > 0.0 {
                indices.extend([center, a, b]);
            } else {
                indices.extend([center, b, a]);
            }
        }
    }
    mesh.indices = indices;
}
