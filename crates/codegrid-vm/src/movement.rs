use codegrid_model::Direction;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Coordinate {
    pub x: usize,
    pub y: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FoldStep {
    Moved(Coordinate),
    VerticalExit(Direction),
}

/// Moves one cell on a board that wraps across all four edges.
/// Returns `None` only for invalid geometry or an invalid starting coordinate.
pub fn move_normal(
    position: Coordinate,
    direction: Direction,
    width: usize,
    height: usize,
) -> Option<Coordinate> {
    if width == 0 || height == 0 || position.x >= width || position.y >= height {
        return None;
    }
    Some(match direction {
        Direction::Up => Coordinate {
            y: if position.y == 0 {
                height - 1
            } else {
                position.y - 1
            },
            ..position
        },
        Direction::Down => Coordinate {
            y: if position.y == height - 1 {
                0
            } else {
                position.y + 1
            },
            ..position
        },
        Direction::Left => Coordinate {
            x: if position.x == 0 {
                width - 1
            } else {
                position.x - 1
            },
            ..position
        },
        Direction::Right => Coordinate {
            x: if position.x == width - 1 {
                0
            } else {
                position.x + 1
            },
            ..position
        },
    })
}

/// Moves one cell in a one-row Folded Block.
///
/// Horizontal movement wraps. A
/// vertical direction produces the normative Fold Resume transition.
pub fn move_folded(position: Coordinate, direction: Direction, width: usize) -> Option<FoldStep> {
    if width == 0 || position.y != 0 || position.x >= width {
        return None;
    }
    match direction {
        Direction::Up | Direction::Down => Some(FoldStep::VerticalExit(direction)),
        Direction::Left if position.x == 0 => {
            Some(FoldStep::Moved(Coordinate { x: width - 1, y: 0 }))
        }
        Direction::Left => Some(FoldStep::Moved(Coordinate {
            x: position.x - 1,
            y: 0,
        })),
        Direction::Right if position.x + 1 == width => {
            Some(FoldStep::Moved(Coordinate { x: 0, y: 0 }))
        }
        Direction::Right => Some(FoldStep::Moved(Coordinate {
            x: position.x + 1,
            y: 0,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::{move_folded, move_normal, Coordinate, FoldStep};
    use codegrid_model::Direction;

    #[test]
    fn normal_edges_wrap() {
        let right_edge = Coordinate { x: 2, y: 1 };
        assert_eq!(
            move_normal(right_edge, Direction::Right, 3, 2),
            Some(Coordinate { x: 0, y: 1 })
        );
        assert_eq!(
            move_normal(Coordinate { x: 0, y: 0 }, Direction::Up, 3, 2,),
            Some(Coordinate { x: 0, y: 1 })
        );
    }

    #[test]
    fn normal_board_edges_wrap_in_each_direction() {
        let cases = [
            (
                Coordinate { x: 1, y: 0 },
                Direction::Up,
                Coordinate { x: 1, y: 3 },
            ),
            (
                Coordinate { x: 2, y: 3 },
                Direction::Down,
                Coordinate { x: 2, y: 0 },
            ),
            (
                Coordinate { x: 0, y: 1 },
                Direction::Left,
                Coordinate { x: 2, y: 1 },
            ),
            (
                Coordinate { x: 2, y: 2 },
                Direction::Right,
                Coordinate { x: 0, y: 2 },
            ),
        ];

        for (position, direction, wrapped) in cases {
            assert_eq!(
                move_normal(position, direction, 3, 4),
                Some(wrapped),
                "Wrap should reach {wrapped:?} from {position:?} toward {direction:?}"
            );
        }
    }

    #[test]
    fn folded_horizontal_edges_wrap_and_vertical_motion_exits() {
        assert_eq!(
            move_folded(Coordinate { x: 0, y: 0 }, Direction::Left, 4),
            Some(FoldStep::Moved(Coordinate { x: 3, y: 0 }))
        );
        assert_eq!(
            move_folded(Coordinate { x: 3, y: 0 }, Direction::Right, 4),
            Some(FoldStep::Moved(Coordinate { x: 0, y: 0 }))
        );
        assert_eq!(
            move_folded(Coordinate { x: 2, y: 0 }, Direction::Down, 4),
            Some(FoldStep::VerticalExit(Direction::Down))
        );
    }

    #[test]
    fn rejects_invalid_geometry_without_panicking() {
        assert_eq!(
            move_normal(Coordinate { x: 0, y: 0 }, Direction::Right, 0, 1,),
            None
        );
        assert_eq!(
            move_folded(Coordinate { x: 0, y: 1 }, Direction::Right, 2),
            None
        );
    }
}
