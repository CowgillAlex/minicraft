//! Exact source preliminary heights without re-evaluating horizontal climate
//! noise at every metre. Unsupported column expressions use the literal scan.
use super::density::{Density, Id, Node, Sampler};

pub(super) fn surface(sampler: &mut Sampler<'_>, x: i32, z: i32) -> i32 {
    let min = sampler.density.min_y;
    let top = min + sampler.density.height - 1;
    let p = [f64::from(x), 0.0, f64::from(z)];
    let root = sampler.density.roots[1];
    let mut nodes = Vec::new();
    if let Some(root) = prepare(sampler, root, p, &mut nodes) {
        let column = Density {
            nodes,
            noises: Vec::new(),
            blended: Vec::new(),
            roots: [root; 5],
            climate: [root; 6],
            min_y: min,
            height: sampler.density.height,
            sea_level: sampler.density.sea_level,
        };
        return highest(&mut Sampler::new(&column), p, min, top).unwrap_or(min);
    }
    (min..=top)
        .rev()
        .find(|&y| sampler.root(1, [p[0], f64::from(y), p[2]]) > 0.0)
        .unwrap_or(min)
}

fn highest(sampler: &mut Sampler<'_>, p: [f64; 3], lo: i32, hi: i32) -> Option<i32> {
    let bounds = sampler.bounds(
        sampler.density.roots[1],
        [p[0], f64::from(lo), p[2]],
        [p[0], f64::from(hi), p[2]],
    );
    if bounds.high < -1e-12 {
        return None;
    }
    if bounds.low > 1e-12 {
        return Some(hi);
    }
    if lo == hi {
        return (sampler.root(1, [p[0], f64::from(hi), p[2]]) > 0.0).then_some(hi);
    }
    let mid = lo + (hi - lo) / 2;
    highest(sampler, p, mid + 1, hi).or_else(|| highest(sampler, p, lo, mid))
}

/// Fold only subexpressions proved independent of Y. The remaining nodes use
/// the ordinary evaluator and conservative bounds, preserving source arithmetic.
fn prepare(sampler: &mut Sampler<'_>, id: Id, p: [f64; 3], nodes: &mut Vec<Node>) -> Option<Id> {
    let node = if horizontal(sampler.density, id) {
        Node::Constant(sampler.eval(id, p))
    } else {
        match &sampler.density.nodes[id] {
            Node::Y => Node::Y,
            Node::Gradient {
                from_y,
                to_y,
                from,
                to,
            } => Node::Gradient {
                from_y: *from_y,
                to_y: *to_y,
                from: *from,
                to: *to,
            },
            Node::Binary(op, a, b) => Node::Binary(
                *op,
                prepare(sampler, *a, p, nodes)?,
                prepare(sampler, *b, p, nodes)?,
            ),
            Node::Unary(op, a) => Node::Unary(*op, prepare(sampler, *a, p, nodes)?),
            Node::Clamp(a, lo, hi) => Node::Clamp(prepare(sampler, *a, p, nodes)?, *lo, *hi),
            Node::Range {
                input,
                min,
                max,
                inside,
                outside,
            } => Node::Range {
                input: prepare(sampler, *input, p, nodes)?,
                min: *min,
                max: *max,
                inside: prepare(sampler, *inside, p, nodes)?,
                outside: prepare(sampler, *outside, p, nodes)?,
            },
            _ => return None,
        }
    };
    let result = nodes.len();
    nodes.push(node);
    Some(result)
}

fn horizontal(density: &Density, id: Id) -> bool {
    match &density.nodes[id] {
        Node::Constant(_) | Node::Shift { .. } => true,
        Node::Noise { y, .. } => *y == 0.0,
        Node::Shifted { y, shift, .. } => {
            *y == 0.0 && shift.iter().all(|&id| horizontal(density, id))
        }
        Node::Binary(_, a, b) => horizontal(density, *a) && horizontal(density, *b),
        Node::Unary(_, a) | Node::Clamp(a, ..) => horizontal(density, *a),
        Node::Range {
            input,
            inside,
            outside,
            ..
        } => [*input, *inside, *outside]
            .into_iter()
            .all(|id| horizontal(density, id)),
        Node::Spline { coordinate, points } => {
            horizontal(density, *coordinate) && points.iter().all(|p| horizontal(density, p.value))
        }
        _ => false,
    }
}
