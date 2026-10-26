use std::collections::{BTreeMap, BTreeSet};

use num_bigint::BigInt;

use codegrid_ir::{BoardId, CodeGridId};
use codegrid_model::{AttachmentInstruction, PrimaryInstruction, Slot};

use crate::Coordinate;

/// A dynamic instruction category used by candidate `StepFootprint` counts.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InstructionKind {
    RandomDirection,
    Compare,
    Condition,
    Read,
    Clear,
    Add,
    Sub,
    MoveRegisterPointer,
    Output,
    Push,
    PopAdd,
    Decode,
    Encode,
    Call,
    Return,
    Neg,
    Nand,
    MemoryLoad,
    MemoryStore,
    MovePage,
    Shift,
    ReadCode,
    WriteCode,
    CustomReturn,
    Halt,
}

impl InstructionKind {
    /// Returns the candidate operation category for an executed Primary.
    /// Direction instructions and structural shells are intentionally excluded.
    pub const fn from_primary(instruction: PrimaryInstruction) -> Option<Self> {
        match instruction {
            PrimaryInstruction::Direction(_)
            | PrimaryInstruction::FoldedBlock(_)
            | PrimaryInstruction::Custom(_) => None,
            PrimaryInstruction::RandomDirection => Some(Self::RandomDirection),
            PrimaryInstruction::Compare => Some(Self::Compare),
            PrimaryInstruction::Read => Some(Self::Read),
            PrimaryInstruction::Clear => Some(Self::Clear),
            PrimaryInstruction::Add => Some(Self::Add),
            PrimaryInstruction::Sub => Some(Self::Sub),
            PrimaryInstruction::MoveRegisterPointer(_) => Some(Self::MoveRegisterPointer),
            PrimaryInstruction::Output | PrimaryInstruction::OutputImmediate(_) => {
                Some(Self::Output)
            }
            PrimaryInstruction::Push => Some(Self::Push),
            PrimaryInstruction::PopAdd => Some(Self::PopAdd),
            PrimaryInstruction::Decode => Some(Self::Decode),
            PrimaryInstruction::Encode => Some(Self::Encode),
            PrimaryInstruction::Call(_) => Some(Self::Call),
            PrimaryInstruction::Return => Some(Self::Return),
            PrimaryInstruction::Neg => Some(Self::Neg),
            PrimaryInstruction::Nand => Some(Self::Nand),
            PrimaryInstruction::MemoryLoad => Some(Self::MemoryLoad),
            PrimaryInstruction::MemoryStore => Some(Self::MemoryStore),
            PrimaryInstruction::MovePage(_) => Some(Self::MovePage),
            PrimaryInstruction::Shift(_) => Some(Self::Shift),
            PrimaryInstruction::CustomReturn => Some(Self::CustomReturn),
            PrimaryInstruction::Halt => Some(Self::Halt),
        }
    }

    /// Returns the candidate operation category for an executed Attachment.
    /// Repeat is not an operation.
    pub const fn from_attachment(attachment: AttachmentInstruction) -> Option<Self> {
        match attachment {
            AttachmentInstruction::ReadCode => Some(Self::ReadCode),
            AttachmentInstruction::WriteCode => Some(Self::WriteCode),
            AttachmentInstruction::Repeat(_) | AttachmentInstruction::RepeatRegister => None,
        }
    }
}

/// Identifies a static normal-board or Folded Block cell.
///
/// Custom invocation identity is intentionally absent: repeated calls to the
/// same static Custom definition deduplicate within one candidate footprint.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StaticCellId {
    pub code_grid: CodeGridId,
    pub board: BoardId,
    pub folded_block: Option<Slot>,
    pub position: Coordinate,
}

/// Identifies one isolated memory instance in a runtime memory location.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemorySpaceId {
    Outer,
    CustomInvocation {
        transition_id: u64,
        caller_thread_id: u64,
        custom_id: Slot,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryLocationId {
    pub space: MemorySpaceId,
    pub address: BigInt,
}

/// Identifies an isolated memory region without exposing host pricing policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemoryAccessSpace {
    Outer,
    CustomInvocation {
        /// Stable ordinal for the enclosing VM transition, used only to
        /// distinguish this invocation's isolated memory from other calls.
        transition_id: u64,
        caller_thread_id: u64,
        custom_id: Slot,
    },
}

/// Exact memory location observed during one candidate transition.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryAccessObservation {
    pub space: MemoryAccessSpace,
    pub address: BigInt,
}

impl From<&MemoryLocationId> for MemoryAccessObservation {
    fn from(location: &MemoryLocationId) -> Self {
        let space = match location.space {
            MemorySpaceId::Outer => MemoryAccessSpace::Outer,
            MemorySpaceId::CustomInvocation {
                transition_id,
                caller_thread_id,
                custom_id,
            } => MemoryAccessSpace::CustomInvocation {
                transition_id,
                caller_thread_id,
                custom_id,
            },
        };
        Self {
            space,
            address: location.address.clone(),
        }
    }
}

/// Candidate high-water marks sampled while evaluating one transition.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StackPeaks {
    pub data: u64,
    pub instruction: u64,
    pub call: u64,
}

/// Policy-neutral observations from one prepared VM transition.
///
/// Hosts can combine these observations with their own resource prices and
/// budgets. Repeated operations are counted, while code-cell and memory
/// identities are deduplicated within this candidate transition.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StepFootprint {
    pub operations: BTreeMap<InstructionKind, u64>,
    pub code_cells: BTreeSet<StaticCellId>,
    pub memory_accesses: BTreeSet<MemoryAccessObservation>,
    pub stack_peaks: StackPeaks,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetricCounterOverflow {
    OperationCount,
    CustomInternalDispatches,
    CommittedSteps,
    DataStackUsage,
    InstructionStackUsage,
    CallStackUsage,
}

/// Committed-step count plus a temporary candidate-observation collector.
/// Candidate observations are returned through `StepReport` and discarded
/// when an attempt is merged into persistent VM state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeMetrics {
    committed_steps: u64,
    candidate_operations: BTreeMap<InstructionKind, u64>,
    candidate_code_cells: BTreeSet<StaticCellId>,
    candidate_memory_accesses: BTreeSet<MemoryAccessObservation>,
    candidate_stack_peaks: StackPeaks,
}

/// Step count carried by run and snapshot responses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeMetricSummary {
    committed_steps: u64,
}

impl RuntimeMetricSummary {
    pub const fn committed_steps(&self) -> u64 {
        self.committed_steps
    }
}

impl RuntimeMetrics {
    pub(crate) fn begin_step_attempt(&self) -> Self {
        Self {
            committed_steps: self.committed_steps,
            candidate_operations: BTreeMap::new(),
            candidate_code_cells: BTreeSet::new(),
            candidate_memory_accesses: BTreeSet::new(),
            candidate_stack_peaks: StackPeaks::default(),
        }
    }

    pub(crate) fn merge_step_attempt(&mut self, attempt: Self) {
        self.committed_steps = attempt.committed_steps;
    }

    pub(crate) fn step_footprint(&self) -> StepFootprint {
        StepFootprint {
            operations: self.candidate_operations.clone(),
            code_cells: self.candidate_code_cells.clone(),
            memory_accesses: self.candidate_memory_accesses.clone(),
            stack_peaks: self.candidate_stack_peaks,
        }
    }

    pub const fn committed_steps(&self) -> u64 {
        self.committed_steps
    }

    pub fn summary(&self) -> RuntimeMetricSummary {
        RuntimeMetricSummary {
            committed_steps: self.committed_steps,
        }
    }

    pub fn record_operation(&mut self, kind: InstructionKind) -> Result<(), MetricCounterOverflow> {
        let count = self.candidate_operations.entry(kind).or_default();
        *count = count
            .checked_add(1)
            .ok_or(MetricCounterOverflow::OperationCount)?;
        Ok(())
    }

    /// Counts one successfully committed outer transition.
    pub fn commit_step(&mut self) -> Result<(), MetricCounterOverflow> {
        self.committed_steps = self
            .committed_steps
            .checked_add(1)
            .ok_or(MetricCounterOverflow::CommittedSteps)?;
        Ok(())
    }

    pub fn record_cell(&mut self, cell: StaticCellId) {
        self.candidate_code_cells.insert(cell);
    }

    pub fn record_memory_access(&mut self, location: MemoryLocationId) {
        self.candidate_memory_accesses
            .insert(MemoryAccessObservation::from(&location));
    }

    /// Samples one normative logical instant for the three aggregate stack
    /// high-water marks. Callers are responsible for summing concurrently
    /// resident outer and Custom stacks for that instant.
    pub fn observe_stack_usage(&mut self, data: u64, instruction: u64, call: u64) {
        self.candidate_stack_peaks.data = self.candidate_stack_peaks.data.max(data);
        self.candidate_stack_peaks.instruction =
            self.candidate_stack_peaks.instruction.max(instruction);
        self.candidate_stack_peaks.call = self.candidate_stack_peaks.call.max(call);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        InstructionKind, MemoryAccessObservation, MemoryAccessSpace, MemoryLocationId,
        MemorySpaceId, RuntimeMetrics, StaticCellId,
    };
    use crate::Coordinate;
    use codegrid_ir::{BoardId, CodeGridId};
    use codegrid_model::{AttachmentInstruction, PrimaryInstruction, Slot};
    use num_bigint::BigInt;

    fn cell(code_grid: CodeGridId, x: usize) -> StaticCellId {
        StaticCellId {
            code_grid,
            board: BoardId::Main,
            folded_block: None,
            position: Coordinate { x, y: 0 },
        }
    }

    #[test]
    fn candidate_footprint_contains_only_current_step_observations() {
        let mut metrics = RuntimeMetrics::default().begin_step_attempt();
        let visited = cell(CodeGridId::Outer, 3);
        let custom_id = Slot::new(2).expect("custom ID is valid");
        metrics.record_cell(visited);
        metrics.record_cell(visited);
        for _ in 0..2 {
            metrics
                .record_operation(InstructionKind::Add)
                .expect("one candidate operation cannot overflow");
        }
        metrics
            .record_operation(InstructionKind::ReadCode)
            .expect("one candidate operation cannot overflow");
        metrics.record_memory_access(MemoryLocationId {
            space: MemorySpaceId::Outer,
            address: BigInt::from(10),
        });
        metrics.record_memory_access(MemoryLocationId {
            space: MemorySpaceId::CustomInvocation {
                transition_id: 4,
                caller_thread_id: 7,
                custom_id,
            },
            address: BigInt::from(10),
        });
        metrics.observe_stack_usage(2, 1, 3);
        metrics.observe_stack_usage(1, 4, 2);

        let footprint = metrics.step_footprint();

        assert_eq!(footprint.operations[&InstructionKind::Add], 2);
        assert_eq!(footprint.operations[&InstructionKind::ReadCode], 1);
        assert_eq!(footprint.code_cells, [visited].into_iter().collect());
        assert_eq!(footprint.memory_accesses.len(), 2);
        assert_eq!(footprint.stack_peaks.data, 2);
        assert_eq!(footprint.stack_peaks.instruction, 4);
        assert_eq!(footprint.stack_peaks.call, 3);
    }

    #[test]
    fn committed_metrics_do_not_retain_candidate_footprints() {
        let mut metrics = RuntimeMetrics::default();
        let mut attempt = metrics.begin_step_attempt();
        attempt
            .record_operation(InstructionKind::Add)
            .expect("one candidate operation cannot overflow");
        attempt.record_cell(cell(CodeGridId::Outer, 4));
        attempt.observe_stack_usage(2, 3, 1);
        let footprint = attempt.step_footprint();

        attempt.commit_step().expect("step count cannot overflow");
        metrics.merge_step_attempt(attempt);

        assert_eq!(metrics.committed_steps(), 1);
        assert_eq!(metrics.summary().committed_steps(), 1);
        assert_eq!(footprint.operations[&InstructionKind::Add], 1);
        assert_eq!(metrics.step_footprint(), Default::default());

        let next_attempt = metrics.begin_step_attempt();
        assert_eq!(next_attempt.step_footprint(), Default::default());
    }

    #[test]
    fn candidate_operation_counter_reports_overflow_without_wrapping() {
        let mut metrics = RuntimeMetrics::default().begin_step_attempt();
        metrics
            .candidate_operations
            .insert(InstructionKind::Add, u64::MAX);

        assert_eq!(
            metrics.record_operation(InstructionKind::Add),
            Err(super::MetricCounterOverflow::OperationCount)
        );
        assert_eq!(
            metrics.candidate_operations[&InstructionKind::Add],
            u64::MAX
        );
    }

    #[test]
    fn committed_step_counter_reports_overflow_without_wrapping() {
        let mut metrics = RuntimeMetrics {
            committed_steps: u64::MAX,
            ..RuntimeMetrics::default()
        };

        assert_eq!(
            metrics.commit_step(),
            Err(super::MetricCounterOverflow::CommittedSteps)
        );
        assert_eq!(metrics.committed_steps(), u64::MAX);
    }

    #[test]
    fn instruction_kind_mapping_covers_source_forms() {
        let mut observed = std::collections::BTreeSet::new();

        for instruction in PrimaryInstruction::source_forms() {
            let expected = match instruction {
                PrimaryInstruction::Direction(_)
                | PrimaryInstruction::FoldedBlock(_)
                | PrimaryInstruction::Custom(_) => None,
                PrimaryInstruction::RandomDirection => Some(InstructionKind::RandomDirection),
                PrimaryInstruction::Compare => Some(InstructionKind::Compare),
                PrimaryInstruction::Read => Some(InstructionKind::Read),
                PrimaryInstruction::Add => Some(InstructionKind::Add),
                PrimaryInstruction::Sub => Some(InstructionKind::Sub),
                PrimaryInstruction::Output | PrimaryInstruction::OutputImmediate(_) => {
                    Some(InstructionKind::Output)
                }
                PrimaryInstruction::Shift(_) => Some(InstructionKind::Shift),
                PrimaryInstruction::Halt => Some(InstructionKind::Halt),
                PrimaryInstruction::Clear => Some(InstructionKind::Clear),
                PrimaryInstruction::MoveRegisterPointer(_) => {
                    Some(InstructionKind::MoveRegisterPointer)
                }
                PrimaryInstruction::Push => Some(InstructionKind::Push),
                PrimaryInstruction::PopAdd => Some(InstructionKind::PopAdd),
                PrimaryInstruction::Decode => Some(InstructionKind::Decode),
                PrimaryInstruction::Encode => Some(InstructionKind::Encode),
                PrimaryInstruction::Call(_) => Some(InstructionKind::Call),
                PrimaryInstruction::Return => Some(InstructionKind::Return),
                PrimaryInstruction::Neg => Some(InstructionKind::Neg),
                PrimaryInstruction::Nand => Some(InstructionKind::Nand),
                PrimaryInstruction::MemoryLoad => Some(InstructionKind::MemoryLoad),
                PrimaryInstruction::MemoryStore => Some(InstructionKind::MemoryStore),
                PrimaryInstruction::MovePage(_) => Some(InstructionKind::MovePage),
                PrimaryInstruction::CustomReturn => Some(InstructionKind::CustomReturn),
            };
            let actual = InstructionKind::from_primary(instruction);
            assert_eq!(
                actual, expected,
                "incorrect instruction mapping for {instruction:?}"
            );
            observed.extend(actual);
        }

        let expected_kinds = [
            InstructionKind::RandomDirection,
            InstructionKind::Compare,
            InstructionKind::Read,
            InstructionKind::Add,
            InstructionKind::Sub,
            InstructionKind::Output,
            InstructionKind::Shift,
            InstructionKind::Halt,
            InstructionKind::Clear,
            InstructionKind::MoveRegisterPointer,
            InstructionKind::Push,
            InstructionKind::PopAdd,
            InstructionKind::Decode,
            InstructionKind::Encode,
            InstructionKind::Call,
            InstructionKind::Return,
            InstructionKind::Neg,
            InstructionKind::Nand,
            InstructionKind::MemoryLoad,
            InstructionKind::MemoryStore,
            InstructionKind::MovePage,
            InstructionKind::CustomReturn,
        ];
        assert_eq!(observed, expected_kinds.into_iter().collect());
    }

    #[test]
    fn memory_footprints_distinguish_outer_and_custom_transition_spaces() {
        let custom_id = Slot::new(2).expect("custom ID is valid");
        let outer = MemoryAccessObservation::from(&MemoryLocationId {
            space: MemorySpaceId::Outer,
            address: BigInt::from(10),
        });
        let custom = MemoryAccessObservation::from(&MemoryLocationId {
            space: MemorySpaceId::CustomInvocation {
                transition_id: 4,
                caller_thread_id: 7,
                custom_id,
            },
            address: BigInt::from(10),
        });
        let next_custom = MemoryAccessObservation::from(&MemoryLocationId {
            space: MemorySpaceId::CustomInvocation {
                transition_id: 5,
                caller_thread_id: 7,
                custom_id,
            },
            address: BigInt::from(10),
        });

        assert_eq!(outer.space, MemoryAccessSpace::Outer);
        assert_ne!(outer, custom);
        assert_ne!(custom, next_custom);
        assert_eq!(
            [outer, custom, next_custom]
                .into_iter()
                .collect::<BTreeSet<_>>()
                .len(),
            3
        );
    }

    #[test]
    fn repeat_attachment_is_not_counted_as_an_operation() {
        assert_eq!(
            InstructionKind::from_attachment(AttachmentInstruction::Repeat(2)),
            None
        );
        assert_eq!(
            InstructionKind::from_attachment(AttachmentInstruction::ReadCode),
            Some(InstructionKind::ReadCode)
        );
    }
}
