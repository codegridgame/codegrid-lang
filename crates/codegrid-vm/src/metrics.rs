use std::collections::BTreeSet;

use num_bigint::BigInt;

use codegrid_ir::{BoardId, CodeGridId};
use codegrid_model::{AttachmentInstruction, PrimaryInstruction, Slot};

use crate::Coordinate;

pub const GAS_SCHEDULE_VERSION: u32 = 1;

/// A dynamic instruction category used by the Instruction Variety metric.
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
    /// Stable public name used when serializing the raw Instruction Variety
    /// metric across native and WebAssembly hosts.
    pub const fn name(self) -> &'static str {
        match self {
            Self::RandomDirection => "RandomDirection",
            Self::Compare => "Compare",
            Self::Condition => "Condition",
            Self::Read => "Read",
            Self::Clear => "Clear",
            Self::Add => "Add",
            Self::Sub => "Sub",
            Self::MoveRegisterPointer => "MoveRegisterPointer",
            Self::Output => "Output",
            Self::Push => "Push",
            Self::PopAdd => "PopAdd",
            Self::Decode => "Decode",
            Self::Encode => "Encode",
            Self::Call => "Call",
            Self::Return => "Return",
            Self::Neg => "Neg",
            Self::Nand => "Nand",
            Self::MemoryLoad => "MemoryLoad",
            Self::MemoryStore => "MemoryStore",
            Self::MovePage => "MovePage",
            Self::Shift => "Shift",
            Self::ReadCode => "ReadCode",
            Self::WriteCode => "WriteCode",
            Self::CustomReturn => "CustomReturn",
            Self::Halt => "Halt",
        }
    }

    /// Returns the metric category for an executed Primary instruction.
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

    /// Returns the metric category for an executed Attachment. Repeat is not
    /// an operation and is excluded from Instruction Variety.
    pub const fn from_attachment(attachment: AttachmentInstruction) -> Option<Self> {
        match attachment {
            AttachmentInstruction::ReadCode => Some(Self::ReadCode),
            AttachmentInstruction::WriteCode => Some(Self::WriteCode),
            AttachmentInstruction::Repeat(_) => None,
        }
    }
}

/// Identifies a static normal-board or Folded Block cell.
///
/// Custom invocation identity is intentionally absent: repeated calls to the
/// same static Custom definition must deduplicate in Used Cells.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StaticCellId {
    pub code_grid: CodeGridId,
    pub board: BoardId,
    pub folded_block: Option<Slot>,
    pub position: Coordinate,
}

/// Identifies one isolated memory instance for Used Memory Addresses.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemorySpaceId {
    Outer,
    CustomInvocation {
        global_tick: u64,
        caller_thread_id: u64,
        custom_id: Slot,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryLocationId {
    pub space: MemorySpaceId,
    pub address: BigInt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetricCounterOverflow {
    OperationCount,
    GlobalTick,
    DataStackUsage,
    InstructionStackUsage,
    CallStackUsage,
}

/// Deterministic raw VM metrics. Ordered sets make snapshots independent of
/// hash iteration order and suitable for stable host serialization.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeMetrics {
    execution_gas: u64,
    memory_gas: u64,
    stack_gas: u64,
    gas_used: u64,
    gas_overflow: bool,
    global_tick: u64,
    operation_count: u64,
    used_cells: BTreeSet<StaticCellId>,
    peak_data_stack_usage: u64,
    peak_instruction_stack_usage: u64,
    peak_call_stack_usage: u64,
    used_memory_addresses: BTreeSet<MemoryLocationId>,
    instruction_variety: BTreeSet<InstructionKind>,
}

/// Compact cumulative raw metrics for per-step host responses.
///
/// Unlike [`RuntimeMetrics`], this summary does not retain or expose the full
/// identities of used cells and memory addresses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeMetricSummary {
    execution_gas: u64,
    memory_gas: u64,
    stack_gas: u64,
    gas_used: u64,
    gas_overflow: bool,
    global_tick: u64,
    operation_count: u64,
    used_cell_count: usize,
    peak_data_stack_usage: u64,
    peak_instruction_stack_usage: u64,
    peak_call_stack_usage: u64,
    used_memory_address_count: usize,
    instruction_variety: BTreeSet<InstructionKind>,
}

impl RuntimeMetricSummary {
    pub const fn gas_used(&self) -> u64 {
        self.gas_used
    }
    pub const fn execution_gas(&self) -> u64 {
        self.execution_gas
    }
    pub const fn memory_gas(&self) -> u64 {
        self.memory_gas
    }
    pub const fn stack_gas(&self) -> u64 {
        self.stack_gas
    }
    pub const fn gas_schedule_version(&self) -> u32 {
        GAS_SCHEDULE_VERSION
    }

    pub const fn global_tick(&self) -> u64 {
        self.global_tick
    }

    pub const fn operation_count(&self) -> u64 {
        self.operation_count
    }

    pub const fn used_cell_count(&self) -> usize {
        self.used_cell_count
    }

    pub const fn peak_data_stack_usage(&self) -> u64 {
        self.peak_data_stack_usage
    }

    pub const fn peak_instruction_stack_usage(&self) -> u64 {
        self.peak_instruction_stack_usage
    }

    pub const fn peak_call_stack_usage(&self) -> u64 {
        self.peak_call_stack_usage
    }

    pub const fn used_memory_address_count(&self) -> usize {
        self.used_memory_address_count
    }

    pub fn instruction_variety(&self) -> &BTreeSet<InstructionKind> {
        &self.instruction_variety
    }
}

impl RuntimeMetrics {
    pub(crate) fn begin_tick_attempt(&self) -> Self {
        Self {
            execution_gas: self.execution_gas,
            memory_gas: self.memory_gas,
            stack_gas: self.stack_gas,
            gas_used: self.gas_used,
            gas_overflow: self.gas_overflow,
            global_tick: self.global_tick,
            operation_count: self.operation_count,
            used_cells: BTreeSet::new(),
            peak_data_stack_usage: self.peak_data_stack_usage,
            peak_instruction_stack_usage: self.peak_instruction_stack_usage,
            peak_call_stack_usage: self.peak_call_stack_usage,
            used_memory_addresses: self.used_memory_addresses.clone(),
            instruction_variety: BTreeSet::new(),
        }
    }

    pub(crate) fn merge_tick_attempt(&mut self, mut attempt: Self) {
        self.execution_gas = attempt.execution_gas;
        self.memory_gas = attempt.memory_gas;
        self.stack_gas = attempt.stack_gas;
        self.gas_used = attempt.gas_used;
        self.gas_overflow = attempt.gas_overflow;
        self.global_tick = attempt.global_tick;
        self.operation_count = attempt.operation_count;
        self.peak_data_stack_usage = attempt.peak_data_stack_usage;
        self.peak_instruction_stack_usage = attempt.peak_instruction_stack_usage;
        self.peak_call_stack_usage = attempt.peak_call_stack_usage;
        self.used_cells.append(&mut attempt.used_cells);
        self.used_memory_addresses
            .append(&mut attempt.used_memory_addresses);
        self.instruction_variety
            .append(&mut attempt.instruction_variety);
    }

    pub const fn gas_used(&self) -> u64 {
        self.gas_used
    }
    pub const fn execution_gas(&self) -> u64 {
        self.execution_gas
    }
    pub const fn memory_gas(&self) -> u64 {
        self.memory_gas
    }
    pub const fn stack_gas(&self) -> u64 {
        self.stack_gas
    }
    pub const fn gas_schedule_version(&self) -> u32 {
        GAS_SCHEDULE_VERSION
    }

    pub const fn global_tick(&self) -> u64 {
        self.global_tick
    }

    pub const fn operation_count(&self) -> u64 {
        self.operation_count
    }

    pub fn used_cells(&self) -> &BTreeSet<StaticCellId> {
        &self.used_cells
    }

    pub fn used_cell_count(&self) -> usize {
        self.used_cells.len()
    }

    pub const fn peak_data_stack_usage(&self) -> u64 {
        self.peak_data_stack_usage
    }

    pub const fn peak_instruction_stack_usage(&self) -> u64 {
        self.peak_instruction_stack_usage
    }

    pub const fn peak_call_stack_usage(&self) -> u64 {
        self.peak_call_stack_usage
    }

    pub fn used_memory_addresses(&self) -> &BTreeSet<MemoryLocationId> {
        &self.used_memory_addresses
    }

    pub fn used_memory_address_count(&self) -> usize {
        self.used_memory_addresses.len()
    }

    pub fn instruction_variety(&self) -> &BTreeSet<InstructionKind> {
        &self.instruction_variety
    }

    pub fn summary(&self) -> RuntimeMetricSummary {
        RuntimeMetricSummary {
            execution_gas: self.execution_gas,
            memory_gas: self.memory_gas,
            stack_gas: self.stack_gas,
            gas_used: self.gas_used,
            gas_overflow: self.gas_overflow,
            global_tick: self.global_tick,
            operation_count: self.operation_count,
            used_cell_count: self.used_cells.len(),
            peak_data_stack_usage: self.peak_data_stack_usage,
            peak_instruction_stack_usage: self.peak_instruction_stack_usage,
            peak_call_stack_usage: self.peak_call_stack_usage,
            used_memory_address_count: self.used_memory_addresses.len(),
            instruction_variety: self.instruction_variety.clone(),
        }
    }

    pub(crate) fn charge_execution(&mut self, price: u64) {
        if let Some(total) = self.execution_gas.checked_add(price) {
            self.execution_gas = total;
        } else {
            self.gas_overflow = true;
        }
    }

    /// An unrepresentable attempted total has no numeric Gas result. Retain
    /// the last completed tick's exact, reconciling breakdown for diagnostics.
    pub(crate) fn retain_representable_gas(&mut self, prior: &Self) {
        self.execution_gas = prior.execution_gas;
        self.memory_gas = prior.memory_gas;
        self.stack_gas = prior.stack_gas;
        self.gas_used = prior.gas_used;
    }

    pub(crate) fn finalize_gas(&mut self) -> Result<(), ()> {
        let memory = u64::try_from(self.used_memory_addresses.len())
            .ok()
            .and_then(|n| n.checked_mul(10));
        let stack = self
            .peak_instruction_stack_usage
            .checked_mul(4)
            .and_then(|n| {
                self.peak_call_stack_usage
                    .checked_mul(16)
                    .and_then(|c| n.checked_add(c))
            })
            .and_then(|n| n.checked_add(self.peak_data_stack_usage));
        match (memory, stack) {
            (Some(memory), Some(stack)) if !self.gas_overflow => {
                if let Some(total) = self
                    .execution_gas
                    .checked_add(memory)
                    .and_then(|n| n.checked_add(stack))
                {
                    self.memory_gas = memory;
                    self.stack_gas = stack;
                    self.gas_used = total;
                    return Ok(());
                }
            }
            _ => {}
        }
        self.gas_overflow = true;
        Err(())
    }

    pub fn record_operation(&mut self, kind: InstructionKind) -> Result<(), MetricCounterOverflow> {
        self.charge_execution(match kind {
            InstructionKind::RandomDirection => 1,
            InstructionKind::Compare
            | InstructionKind::Condition
            | InstructionKind::MoveRegisterPointer
            | InstructionKind::MovePage
            | InstructionKind::Shift
            | InstructionKind::Return
            | InstructionKind::CustomReturn => 2,
            InstructionKind::Clear
            | InstructionKind::Add
            | InstructionKind::Sub
            | InstructionKind::Neg
            | InstructionKind::Nand
            | InstructionKind::Push
            | InstructionKind::PopAdd
            | InstructionKind::Decode
            | InstructionKind::Encode => 3,
            InstructionKind::Read
            | InstructionKind::Output
            | InstructionKind::MemoryLoad
            | InstructionKind::ReadCode => 5,
            InstructionKind::MemoryStore | InstructionKind::WriteCode | InstructionKind::Call => 8,
            InstructionKind::Halt => 0,
        });
        self.instruction_variety.insert(kind);
        self.operation_count = self
            .operation_count
            .checked_add(1)
            .ok_or(MetricCounterOverflow::OperationCount)?;
        Ok(())
    }

    /// Advances the outer clock after a successful Global Tick commit.
    pub fn commit_global_tick(&mut self) -> Result<(), MetricCounterOverflow> {
        self.global_tick = self
            .global_tick
            .checked_add(1)
            .ok_or(MetricCounterOverflow::GlobalTick)?;
        Ok(())
    }

    pub fn record_cell(&mut self, cell: StaticCellId) {
        self.used_cells.insert(cell);
    }

    pub fn record_memory_access(&mut self, location: MemoryLocationId) {
        self.used_memory_addresses.insert(location);
    }

    /// Samples one normative logical instant for the three aggregate stack
    /// high-water marks. Callers are responsible for summing concurrently
    /// resident outer and Custom stacks for that instant.
    pub fn observe_stack_usage(&mut self, data: u64, instruction: u64, call: u64) {
        self.peak_data_stack_usage = self.peak_data_stack_usage.max(data);
        self.peak_instruction_stack_usage = self.peak_instruction_stack_usage.max(instruction);
        self.peak_call_stack_usage = self.peak_call_stack_usage.max(call);
    }
}

#[cfg(test)]
mod tests {
    use super::{InstructionKind, MemoryLocationId, MemorySpaceId, RuntimeMetrics, StaticCellId};
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
    fn gas_arithmetic_overflow_does_not_wrap() {
        let mut metrics = RuntimeMetrics {
            execution_gas: u64::MAX,
            ..RuntimeMetrics::default()
        };
        metrics.charge_execution(1);
        assert!(metrics.finalize_gas().is_err());
        assert_eq!(metrics.execution_gas(), u64::MAX);
        let mut metrics = RuntimeMetrics::default();
        metrics.observe_stack_usage(0, 0, u64::MAX);
        assert!(metrics.finalize_gas().is_err());
    }

    #[test]
    fn counts_static_cells_once_and_keeps_custom_definitions_distinct() {
        let mut metrics = RuntimeMetrics::default();
        let outer = cell(CodeGridId::Outer, 0);
        let custom = cell(
            CodeGridId::Custom(Slot::new(0).expect("zero is a valid ID")),
            0,
        );

        metrics.record_cell(outer);
        metrics.record_cell(outer);
        metrics.record_cell(custom);

        assert_eq!(metrics.used_cell_count(), 2);
    }

    #[test]
    fn counts_each_custom_memory_invocation_as_a_distinct_space() {
        let custom_id = Slot::new(0).expect("zero is a valid ID");
        let address = BigInt::from(10);
        let mut metrics = RuntimeMetrics::default();

        for global_tick in [1, 2] {
            metrics.record_memory_access(MemoryLocationId {
                space: MemorySpaceId::CustomInvocation {
                    global_tick,
                    caller_thread_id: 3,
                    custom_id,
                },
                address: address.clone(),
            });
        }
        metrics.record_memory_access(MemoryLocationId {
            space: MemorySpaceId::Outer,
            address,
        });

        assert_eq!(metrics.used_memory_address_count(), 3);
    }

    #[test]
    fn operation_count_includes_repeated_executions_but_variety_deduplicates() {
        let mut metrics = RuntimeMetrics::default();
        metrics
            .record_operation(InstructionKind::Add)
            .expect("test operation count cannot overflow");
        metrics
            .record_operation(InstructionKind::Add)
            .expect("test operation count cannot overflow");
        metrics
            .record_operation(InstructionKind::ReadCode)
            .expect("test operation count cannot overflow");

        assert_eq!(metrics.operation_count(), 3);
        assert_eq!(metrics.instruction_variety().len(), 2);
    }

    #[test]
    fn instruction_variety_mapping_covers_the_full_source_inventory() {
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
                "incorrect variety mapping for {instruction:?}"
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
    fn summary_preserves_all_raw_metrics_without_copying_used_identities() {
        let mut metrics = RuntimeMetrics::default();
        metrics
            .record_operation(InstructionKind::Add)
            .expect("test operation count cannot overflow");
        metrics.record_cell(cell(CodeGridId::Outer, 4));
        metrics.record_memory_access(MemoryLocationId {
            space: MemorySpaceId::Outer,
            address: BigInt::from(13),
        });
        metrics.observe_stack_usage(2, 3, 1);
        metrics.commit_global_tick().unwrap();

        let summary = metrics.summary();

        assert_eq!(summary.global_tick(), 1);
        assert_eq!(summary.operation_count(), 1);
        assert_eq!(summary.used_cell_count(), 1);
        assert_eq!(summary.peak_data_stack_usage(), 2);
        assert_eq!(summary.peak_instruction_stack_usage(), 3);
        assert_eq!(summary.peak_call_stack_usage(), 1);
        assert_eq!(summary.used_memory_address_count(), 1);
        assert_eq!(summary.instruction_variety(), metrics.instruction_variety());
    }

    #[test]
    fn stack_high_water_marks_never_decrease() {
        let mut metrics = RuntimeMetrics::default();
        metrics.observe_stack_usage(4, 2, 1);
        metrics.observe_stack_usage(1, 5, 0);

        assert_eq!(metrics.peak_data_stack_usage(), 4);
        assert_eq!(metrics.peak_instruction_stack_usage(), 5);
        assert_eq!(metrics.peak_call_stack_usage(), 1);
    }

    #[test]
    fn reports_operation_counter_overflow_instead_of_wrapping_or_saturating() {
        let mut metrics = RuntimeMetrics {
            operation_count: u64::MAX,
            ..RuntimeMetrics::default()
        };

        assert_eq!(
            metrics.record_operation(InstructionKind::Add),
            Err(super::MetricCounterOverflow::OperationCount)
        );
        assert_eq!(metrics.operation_count(), u64::MAX);
        assert!(metrics
            .instruction_variety()
            .contains(&InstructionKind::Add));
    }

    #[test]
    fn failed_attempts_do_not_advance_global_tick() {
        let mut metrics = RuntimeMetrics::default();

        assert_eq!(metrics.global_tick(), 0);
        metrics
            .record_operation(InstructionKind::Add)
            .expect("test operation count cannot overflow");
        assert_eq!(metrics.global_tick(), 0);

        metrics
            .commit_global_tick()
            .expect("test global tick cannot overflow");
        assert_eq!(metrics.global_tick(), 1);
    }

    #[test]
    fn reports_global_tick_overflow_without_wrapping() {
        let mut metrics = RuntimeMetrics {
            global_tick: u64::MAX,
            ..RuntimeMetrics::default()
        };

        assert_eq!(
            metrics.commit_global_tick(),
            Err(super::MetricCounterOverflow::GlobalTick)
        );
        assert_eq!(metrics.global_tick(), u64::MAX);
    }

    #[test]
    fn metric_kinds_exclude_directions_structural_forms_and_repeat() {
        let zero = Slot::new(0).expect("zero is a valid ID");

        assert_eq!(
            InstructionKind::from_primary(PrimaryInstruction::Direction(
                codegrid_model::Direction::Up
            )),
            None
        );
        assert_eq!(
            InstructionKind::from_primary(PrimaryInstruction::FoldedBlock(zero)),
            None
        );
        assert_eq!(
            InstructionKind::from_primary(PrimaryInstruction::Custom(zero)),
            None
        );
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
