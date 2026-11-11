use super::*;

/// A handle on either side of a standard wooden outline claims the enclosed panel.
/// Recognition is only attempted near edits to unclaimed handles, never every tick.
pub fn recognize(world: &World, handle: [i32; 3]) -> Option<Door> {
    for facing in 0..4 {
        for x in [0, 1, SIZE[0] - 2, SIZE[0] - 1] {
            for y in [15, 16] {
                let center =
                    rotate([x * 2 + 1, y * 2 + 1, 7], facing).map(|v| (v - 1).div_euclid(2));
                let origin = sub(handle, center);
                let mut cells = vec![world::AIR; SIZE.iter().product::<i32>() as usize];
                let candidate = Door::new(origin, facing, x < SIZE[0] / 2, cells.clone());
                let mut frame = true;
                for yy in 0..SIZE[1] {
                    for xx in 0..SIZE[0] {
                        for zz in 1..=2 {
                            if xx != 0 && xx != SIZE[0] - 1 && yy != 0 && yy != SIZE[1] - 1 {
                                continue;
                            }
                            let cell = candidate.closed_cell([xx, yy, zz]);
                            if world.get_static(cell[0], cell[1], cell[2]) != world::WOOD {
                                frame = false;
                                break;
                            }
                        }
                    }
                }
                if !frame {
                    continue;
                }
                for z in 0..SIZE[2] {
                    for yy in 0..SIZE[1] {
                        for xx in 0..SIZE[0] {
                            let p = [xx, yy, z];
                            let cell = candidate.closed_cell(p);
                            if !world.contains(cell) || world.door_at(cell).is_some() {
                                frame = false;
                            }
                            cells[index(p)] = world.get_static(cell[0], cell[1], cell[2]);
                        }
                    }
                }
                if !frame {
                    continue;
                }
                let door = Door::new(origin, facing, x < SIZE[0] / 2, cells);
                if door.status() == Status::Ready {
                    return Some(door);
                }
            }
        }
    }
    None
}
