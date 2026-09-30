use num_bigint::BigInt;

use codegrid_ir::BoardId;
use codegrid_model::{Direction, InstructionStackItem, Value};

use crate::{Coordinate, SplitMix64};

/// The explicit per-thread execution phase carried across ticks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExecutionPhase {
    Normal,
    AfterCall,
    Repeat {
        total: u8,
        completed: u8,
    },
    Fold {
        fold_id: codegrid_model::Slot,
        internal_position: Coordinate,
        internal_direction: Direction,
        saved_outer_direction: Direction,
    },
    FoldResume {
        saved_outer_direction: Direction,
    },
    Terminated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CallFrame {
    pub caller_board: BoardId,
    pub call_position: Coordinate,
    pub saved_direction: Direction,
}

/// Mutable state private to one outer or Custom-internal thread.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ThreadState {
    pub id: u64,
    pub board: BoardId,
    pub position: Coordinate,
    pub direction: Direction,
    pub register_pointer: u8,
    pub page: BigInt,
    pub data_stack: Vec<Value>,
    pub instruction_stack: Vec<InstructionStackItem>,
    pub call_stack: Vec<CallFrame>,
    pub phase: ExecutionPhase,
    pub rng: SplitMix64,
}

impl ThreadState {
    pub fn initial(id: u64, position: Coordinate, direction: Direction, rng_state: u64) -> Self {
        Self {
            id,
            board: BoardId::Main,
            position,
            direction,
            register_pointer: 0,
            page: BigInt::from(0u8),
            data_stack: Vec::new(),
            instruction_stack: Vec::new(),
            call_stack: Vec::new(),
            phase: ExecutionPhase::Normal,
            rng: SplitMix64::new(rng_state),
        }
    }

    pub fn stack_depths(&self) -> Option<(u64, u64, u64)> {
        Some((
            u64::try_from(self.data_stack.len()).ok()?,
            u64::try_from(self.instruction_stack.len()).ok()?,
            u64::try_from(self.call_stack.len()).ok()?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{ExecutionPhase, ThreadState};
    use crate::Coordinate;
    use codegrid_model::Direction;

    #[test]
    fn initial_thread_state_is_fresh_and_starts_on_its_entry_cell() {
        let thread = ThreadState::initial(3, Coordinate { x: 2, y: 1 }, Direction::Left, 0x1234);

        assert_eq!(thread.id, 3);
        assert_eq!(thread.position, Coordinate { x: 2, y: 1 });
        assert_eq!(thread.direction, Direction::Left);
        assert_eq!(thread.register_pointer, 0);
        assert!(thread.data_stack.is_empty());
        assert!(thread.instruction_stack.is_empty());
        assert!(thread.call_stack.is_empty());
        assert_eq!(thread.phase, ExecutionPhase::Normal);
        assert_eq!(thread.rng.state(), 0x1234);
        assert_eq!(thread.stack_depths(), Some((0, 0, 0)));
    }
}
