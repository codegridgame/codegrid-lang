use num_bigint::BigInt;

use codegrid_ir::BoardId;
use codegrid_model::Slot;

use crate::{Coordinate, StaticCellId};

/// Identifies the execution context that produced a runtime error.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionScope {
    Outer,
    Custom {
        caller_thread_id: u64,
        custom_id: Slot,
        internal_tick: u64,
    },
}

/// A deterministic runtime failure reported by the VM.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeError {
    global_tick: u64,
    scope: ExecutionScope,
    kind: RuntimeErrorKind,
}

/// The machine-readable category and resource context of a runtime error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeErrorKind {
    ConcurrentCallerStackReadConflict {
        internal_thread_ids: Vec<u64>,
    },
    ConcurrentCallerStackReadWriteConflict {
        internal_thread_ids: Vec<u64>,
    },
    ConcurrentCallerStackWriteConflict {
        internal_thread_ids: Vec<u64>,
    },
    ConcurrentCodeWriteConflict {
        cell: StaticCellId,
        thread_ids: Vec<u64>,
    },
    ConcurrentInputConflict {
        thread_ids: Vec<u64>,
    },
    ConcurrentMemoryWriteConflict {
        address: BigInt,
        thread_ids: Vec<u64>,
    },
    ConcurrentOutputConflict {
        thread_ids: Vec<u64>,
    },
    ConcurrentWriteConflict {
        register: u8,
        thread_ids: Vec<u64>,
    },
    CustomExecutionLimitExceeded {
        limit: u64,
    },
    ReturnWithoutCall {
        thread_id: u64,
        board: BoardId,
        position: Coordinate,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CanonicalErrorKey {
    scope: ExecutionScope,
    kind_name: &'static str,
    resource: ResourceKey,
    thread_ids: Vec<u64>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ResourceKey {
    None,
    BoardCell(BoardId, Coordinate),
    Register(u8),
    Memory(BigInt),
    CodeCell(StaticCellId),
    CallerStack,
}

impl RuntimeError {
    /// Creates an error and canonicalizes all participant lists.
    pub fn new(global_tick: u64, scope: ExecutionScope, mut kind: RuntimeErrorKind) -> Self {
        match &mut kind {
            RuntimeErrorKind::ConcurrentCallerStackReadConflict {
                internal_thread_ids,
            }
            | RuntimeErrorKind::ConcurrentCallerStackReadWriteConflict {
                internal_thread_ids,
            }
            | RuntimeErrorKind::ConcurrentCallerStackWriteConflict {
                internal_thread_ids,
            } => {
                normalize_ids(internal_thread_ids);
            }
            RuntimeErrorKind::ConcurrentCodeWriteConflict { thread_ids, .. }
            | RuntimeErrorKind::ConcurrentInputConflict { thread_ids }
            | RuntimeErrorKind::ConcurrentMemoryWriteConflict { thread_ids, .. }
            | RuntimeErrorKind::ConcurrentOutputConflict { thread_ids }
            | RuntimeErrorKind::ConcurrentWriteConflict { thread_ids, .. } => {
                normalize_ids(thread_ids);
            }
            RuntimeErrorKind::CustomExecutionLimitExceeded { .. }
            | RuntimeErrorKind::ReturnWithoutCall { .. } => {}
        }

        Self {
            global_tick,
            scope,
            kind,
        }
    }

    pub const fn global_tick(&self) -> u64 {
        self.global_tick
    }

    pub const fn scope(&self) -> ExecutionScope {
        self.scope
    }

    pub fn kind(&self) -> &RuntimeErrorKind {
        &self.kind
    }

    pub fn code(&self) -> &'static str {
        match &self.kind {
            RuntimeErrorKind::ConcurrentCallerStackReadConflict { .. } => {
                "ConcurrentCallerStackReadConflict"
            }
            RuntimeErrorKind::ConcurrentCallerStackReadWriteConflict { .. } => {
                "ConcurrentCallerStackReadWriteConflict"
            }
            RuntimeErrorKind::ConcurrentCallerStackWriteConflict { .. } => {
                "ConcurrentCallerStackWriteConflict"
            }
            RuntimeErrorKind::ConcurrentCodeWriteConflict { .. } => "ConcurrentCodeWriteConflict",
            RuntimeErrorKind::ConcurrentInputConflict { .. } => "ConcurrentInputConflict",
            RuntimeErrorKind::ConcurrentMemoryWriteConflict { .. } => {
                "ConcurrentMemoryWriteConflict"
            }
            RuntimeErrorKind::ConcurrentOutputConflict { .. } => "ConcurrentOutputConflict",
            RuntimeErrorKind::ConcurrentWriteConflict { .. } => "ConcurrentWriteConflict",
            RuntimeErrorKind::CustomExecutionLimitExceeded { .. } => "CustomExecutionLimitExceeded",
            RuntimeErrorKind::ReturnWithoutCall { .. } => "ReturnWithoutCall",
        }
    }

    pub fn error_number(&self) -> Option<&'static str> {
        codegrid_model::error_number("vm", self.code())
    }

    fn canonical_key(&self) -> CanonicalErrorKey {
        let (resource, thread_ids) = match &self.kind {
            RuntimeErrorKind::ConcurrentCallerStackReadConflict {
                internal_thread_ids,
            }
            | RuntimeErrorKind::ConcurrentCallerStackReadWriteConflict {
                internal_thread_ids,
            }
            | RuntimeErrorKind::ConcurrentCallerStackWriteConflict {
                internal_thread_ids,
            } => (ResourceKey::CallerStack, internal_thread_ids.clone()),
            RuntimeErrorKind::ConcurrentCodeWriteConflict { cell, thread_ids } => {
                (ResourceKey::CodeCell(*cell), thread_ids.clone())
            }
            RuntimeErrorKind::ConcurrentInputConflict { thread_ids }
            | RuntimeErrorKind::ConcurrentOutputConflict { thread_ids } => {
                (ResourceKey::None, thread_ids.clone())
            }
            RuntimeErrorKind::ConcurrentMemoryWriteConflict {
                address,
                thread_ids,
            } => (ResourceKey::Memory(address.clone()), thread_ids.clone()),
            RuntimeErrorKind::ConcurrentWriteConflict {
                register,
                thread_ids,
            } => (ResourceKey::Register(*register), thread_ids.clone()),
            RuntimeErrorKind::CustomExecutionLimitExceeded { .. } => {
                (ResourceKey::None, Vec::new())
            }
            RuntimeErrorKind::ReturnWithoutCall {
                thread_id,
                board,
                position,
            } => (ResourceKey::BoardCell(*board, *position), vec![*thread_id]),
        };

        CanonicalErrorKey {
            scope: self.scope,
            kind_name: self.code(),
            resource,
            thread_ids,
        }
    }

    /// Compares errors using the VM specification's stable diagnostic order.
    pub fn canonical_cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.canonical_key().cmp(&other.canonical_key())
    }

    /// Orders records and removes duplicate errors for one execution result.
    pub fn sort_canonical(errors: &mut Vec<Self>) {
        errors.sort_by(Self::canonical_cmp);
        errors.dedup();
    }
}

fn normalize_ids(ids: &mut Vec<u64>) {
    ids.sort_unstable();
    ids.dedup();
}

#[cfg(test)]
mod tests {
    use super::{ExecutionScope, RuntimeError, RuntimeErrorKind};
    use crate::Coordinate;
    use codegrid_ir::BoardId;
    use codegrid_model::Slot;
    use num_bigint::BigInt;

    #[test]
    fn canonicalizes_participant_ids_and_reports_stable_error_codes() {
        let error = RuntimeError::new(
            7,
            ExecutionScope::Outer,
            RuntimeErrorKind::ConcurrentWriteConflict {
                register: 3,
                thread_ids: vec![9, 2, 9],
            },
        );

        assert_eq!(error.code(), "ConcurrentWriteConflict");
        let RuntimeErrorKind::ConcurrentWriteConflict { thread_ids, .. } = error.kind() else {
            panic!("constructor must preserve the requested error kind");
        };
        assert_eq!(thread_ids.as_slice(), &[2, 9]);
    }

    #[test]
    fn canonical_order_uses_scope_then_error_code_then_resource() {
        let input = RuntimeError::new(
            1,
            ExecutionScope::Outer,
            RuntimeErrorKind::ConcurrentInputConflict {
                thread_ids: vec![0, 1],
            },
        );
        let write = RuntimeError::new(
            1,
            ExecutionScope::Outer,
            RuntimeErrorKind::ConcurrentWriteConflict {
                register: 0,
                thread_ids: vec![0, 1],
            },
        );
        assert!(input.canonical_cmp(&write).is_lt());

        let custom = RuntimeError::new(
            1,
            ExecutionScope::Custom {
                caller_thread_id: 0,
                custom_id: Slot::new(0).expect("zero is valid"),
                internal_tick: 1,
            },
            RuntimeErrorKind::ConcurrentMemoryWriteConflict {
                address: BigInt::from(-1),
                thread_ids: vec![0, 1],
            },
        );
        assert!(input.canonical_cmp(&custom).is_lt());
    }

    #[test]
    fn custom_error_order_uses_caller_then_custom_id_then_internal_tick() {
        let custom_zero = Slot::new(0).expect("zero is a valid Custom ID");
        let custom_one = Slot::new(1).expect("one is a valid Custom ID");
        let make_error = |scope| {
            RuntimeError::new(
                1,
                scope,
                RuntimeErrorKind::CustomExecutionLimitExceeded { limit: 4 },
            )
        };
        let expected = vec![
            ExecutionScope::Outer,
            ExecutionScope::Custom {
                caller_thread_id: 0,
                custom_id: custom_zero,
                internal_tick: 1,
            },
            ExecutionScope::Custom {
                caller_thread_id: 0,
                custom_id: custom_zero,
                internal_tick: 2,
            },
            ExecutionScope::Custom {
                caller_thread_id: 0,
                custom_id: custom_one,
                internal_tick: 1,
            },
            ExecutionScope::Custom {
                caller_thread_id: 1,
                custom_id: custom_zero,
                internal_tick: 1,
            },
        ];
        let mut errors = vec![
            make_error(ExecutionScope::Custom {
                caller_thread_id: 1,
                custom_id: custom_zero,
                internal_tick: 1,
            }),
            make_error(ExecutionScope::Custom {
                caller_thread_id: 0,
                custom_id: custom_one,
                internal_tick: 1,
            }),
            make_error(ExecutionScope::Custom {
                caller_thread_id: 0,
                custom_id: custom_zero,
                internal_tick: 2,
            }),
            make_error(ExecutionScope::Outer),
            make_error(ExecutionScope::Custom {
                caller_thread_id: 0,
                custom_id: custom_zero,
                internal_tick: 1,
            }),
        ];

        RuntimeError::sort_canonical(&mut errors);

        assert_eq!(
            errors.iter().map(RuntimeError::scope).collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn same_board_cell_errors_sort_by_thread_id_() {
        let thread_zero = RuntimeError::new(
            1,
            ExecutionScope::Outer,
            RuntimeErrorKind::ReturnWithoutCall {
                thread_id: 0,
                board: BoardId::Main,
                position: Coordinate { x: 0, y: 0 },
            },
        );
        let thread_one = RuntimeError::new(
            1,
            ExecutionScope::Outer,
            RuntimeErrorKind::ReturnWithoutCall {
                thread_id: 1,
                board: BoardId::Main,
                position: Coordinate { x: 0, y: 0 },
            },
        );
        let mut errors = vec![thread_one, thread_zero];

        RuntimeError::sort_canonical(&mut errors);

        let thread_ids = errors
            .iter()
            .map(|error| match error.kind() {
                RuntimeErrorKind::ReturnWithoutCall { thread_id, .. } => *thread_id,
                _ => unreachable!("the test only inserts ReturnWithoutCall errors"),
            })
            .collect::<Vec<_>>();
        assert_eq!(thread_ids, vec![0, 1]);
    }

    #[test]
    fn local_error_context_retains_precise_board_position() {
        let error = RuntimeError::new(
            2,
            ExecutionScope::Outer,
            RuntimeErrorKind::ReturnWithoutCall {
                thread_id: 4,
                board: BoardId::Main,
                position: Coordinate { x: 3, y: 1 },
            },
        );

        assert_eq!(error.code(), "ReturnWithoutCall");
        assert_eq!(error.global_tick(), 2);
    }
}
