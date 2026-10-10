use std::collections::{BTreeMap, VecDeque};

use codegrid_ir::{ScopedProgram, VerifiedProgram};
use codegrid_model::{Direction, InstructionStackItem, Value};

use crate::thread::{ExecutionPhase, ThreadState};
use crate::{
    outer_thread_state, Coordinate, Memory, MemoryAddress, RuntimeError, RuntimeMetrics, VmConfig,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmStatus {
    Running,
    Halted,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmInitializationError {
    InitialThreadIdOverflow,
}

/// Input append failures are host resource/lifecycle failures, not VM errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputAppendError {
    Terminal,
    Capacity,
}

/// One isolated execution instance over a verified immutable program.
pub struct Vm {
    pub(super) config: VmConfig,
    pub(super) verified_program: VerifiedProgram,
    pub(super) registers: [Value; 10],
    pub(super) memory: Memory,
    pub(super) input: VecDeque<Value>,
    pub(super) output: Vec<Value>,
    pub(super) runtime_program: ScopedProgram,
    pub(super) threads: Vec<ThreadState>,
    pub(super) metrics: RuntimeMetrics,
    pub(super) status: VmStatus,
    pub(super) terminal_errors: Vec<RuntimeError>,
    pub(super) terminal_fault: Option<VmFault>,
}

impl Vm {
    pub fn new(
        program: VerifiedProgram,
        input: impl IntoIterator<Item = Value>,
        config: VmConfig,
    ) -> Result<Self, VmInitializationError> {
        Self::with_initial_memory(program, input, BTreeMap::new(), config)
    }

    pub fn with_initial_memory(
        program: VerifiedProgram,
        input: impl IntoIterator<Item = Value>,
        initial_memory: BTreeMap<MemoryAddress, Value>,
        config: VmConfig,
    ) -> Result<Self, VmInitializationError> {
        let main = &program.program().outer.main;
        let mut threads = Vec::new();

        for (index, cell) in main.cells.iter().enumerate() {
            let Some(direction) = cell.entry else {
                continue;
            };
            let id = u64::try_from(threads.len())
                .map_err(|_| VmInitializationError::InitialThreadIdOverflow)?;
            let position = Coordinate {
                x: index % main.width,
                y: index / main.width,
            };
            threads.push(ThreadState::initial(
                id,
                position,
                direction,
                outer_thread_state(config.seed(), id),
            ));
        }

        if threads.is_empty() {
            threads.push(ThreadState::initial(
                0,
                Coordinate { x: 0, y: 0 },
                Direction::Right,
                outer_thread_state(config.seed(), 0),
            ));
        }

        Ok(Self {
            verified_program: program.clone(),
            runtime_program: program.program().outer.clone(),
            config,
            registers: [0; 10],
            memory: Memory::from_cells(initial_memory),
            input: input.into_iter().collect(),
            output: Vec::new(),
            threads,
            metrics: RuntimeMetrics::default(),
            status: VmStatus::Running,
            terminal_errors: Vec::new(),
            terminal_fault: None,
        })
    }

    pub const fn status(&self) -> VmStatus {
        self.status
    }

    pub const fn committed_ticks(&self) -> u64 {
        self.metrics.global_tick()
    }

    pub const fn config(&self) -> VmConfig {
        self.config
    }

    /// Append explicit data at a completed step boundary without executing code.
    ///
    /// Steps are synchronous and retain no unfinished tick between calls, so
    /// exclusive access guarantees a committed boundary (including rollback).
    /// Reserve the complete append before publishing any bytes. Evaluators own
    /// their queue ceilings; this operation only reports allocation failure.
    pub fn append_input(&mut self, values: &[Value]) -> Result<(), InputAppendError> {
        if self.status != VmStatus::Running {
            return Err(InputAppendError::Terminal);
        }
        self.input
            .try_reserve(values.len())
            .map_err(|_| InputAppendError::Capacity)?;
        self.input.extend(values.iter().copied());
        Ok(())
    }

    pub fn snapshot(&self) -> VmSnapshot {
        VmSnapshot {
            status: self.status,
            committed_ticks: self.metrics.global_tick(),
            registers: self.registers,
            memory: self.memory.clone(),
            input: self.input.iter().copied().collect(),
            output: self.output.clone(),
            runtime_program: self.runtime_program.clone(),
            threads: self.threads.iter().map(ThreadSnapshot::from).collect(),
            metrics: self.metrics.clone(),
            errors: self.terminal_errors.clone(),
            fault: self.terminal_fault,
        }
    }

    pub fn snapshot_view(&self) -> VmSnapshotView<'_> {
        VmSnapshotView { vm: self }
    }
}

/// Read-only borrowed access to the current VM state.
///
/// Adapters can inspect or serialize state without cloning the owned snapshot.
pub struct VmSnapshotView<'a> {
    vm: &'a Vm,
}

impl<'a> VmSnapshotView<'a> {
    pub const fn status(&self) -> VmStatus {
        self.vm.status
    }

    pub const fn committed_ticks(&self) -> u64 {
        self.vm.metrics.global_tick()
    }

    pub const fn registers(&self) -> &'a [Value; 10] {
        &self.vm.registers
    }

    pub const fn memory(&self) -> &'a Memory {
        &self.vm.memory
    }

    pub fn input(&self) -> impl Iterator<Item = &'a Value> + '_ {
        self.vm.input.iter()
    }

    pub fn output(&self) -> &[Value] {
        &self.vm.output
    }

    pub const fn runtime_program(&self) -> &'a ScopedProgram {
        &self.vm.runtime_program
    }

    pub fn threads(&self) -> impl Iterator<Item = ThreadSnapshotView<'a>> + '_ {
        self.vm
            .threads
            .iter()
            .map(|thread| ThreadSnapshotView { thread })
    }

    pub const fn metrics(&self) -> &'a RuntimeMetrics {
        &self.vm.metrics
    }

    pub fn errors(&self) -> &'a [RuntimeError] {
        &self.vm.terminal_errors
    }

    pub const fn fault(&self) -> Option<VmFault> {
        self.vm.terminal_fault
    }
}

/// Read-only borrowed access to one thread in a [`VmSnapshotView`].
pub struct ThreadSnapshotView<'a> {
    thread: &'a ThreadState,
}

impl ThreadSnapshotView<'_> {
    pub const fn id(&self) -> u64 {
        self.thread.id
    }

    pub const fn board(&self) -> codegrid_ir::BoardId {
        self.thread.board
    }

    pub const fn position(&self) -> crate::Coordinate {
        self.thread.position
    }

    pub const fn direction(&self) -> Direction {
        self.thread.direction
    }

    pub const fn register_pointer(&self) -> u8 {
        self.thread.register_pointer
    }

    pub const fn status_flag(&self) -> u8 {
        self.thread.status_flag
    }

    pub fn page(&self) -> &crate::Page {
        &self.thread.page
    }

    pub fn data_stack(&self) -> &[Value] {
        &self.thread.data_stack
    }

    pub fn instruction_stack(&self) -> &[InstructionStackItem] {
        &self.thread.instruction_stack
    }

    pub const fn private_registers(&self) -> Option<&[Value; 10]> {
        self.thread.private_registers.as_ref()
    }

    pub fn call_frames(&self) -> impl Iterator<Item = CallFrameSnapshot> + '_ {
        self.thread
            .call_stack
            .iter()
            .map(|frame| CallFrameSnapshot {
                caller_board: frame.caller_board,
                call_position: frame.call_position,
                saved_direction: frame.saved_direction,
                saved_registers: frame.saved_registers,
                saved_register_pointer: frame.saved_register_pointer,
                saved_status_flag: frame.saved_status_flag,
            })
    }

    pub fn phase(&self) -> ThreadPhaseSnapshot {
        match self.thread.phase {
            ExecutionPhase::Normal => ThreadPhaseSnapshot::Normal,
            ExecutionPhase::AfterCall => ThreadPhaseSnapshot::AfterCall,
            ExecutionPhase::Repeat { total, completed } => {
                ThreadPhaseSnapshot::Repeat { total, completed }
            }
            ExecutionPhase::Fold {
                fold_id,
                internal_position,
                internal_direction,
                saved_outer_direction,
            } => ThreadPhaseSnapshot::Fold {
                fold_id,
                internal_position,
                internal_direction,
                saved_outer_direction,
            },
            ExecutionPhase::FoldResume {
                saved_outer_direction,
            } => ThreadPhaseSnapshot::FoldResume {
                saved_outer_direction,
            },
            ExecutionPhase::Terminated => ThreadPhaseSnapshot::Terminated,
        }
    }

    pub fn random_state(&self) -> u64 {
        self.thread.rng.state()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmFault {
    MetricCounterOverflow(crate::MetricCounterOverflow),
    GlobalTickOverflow,
    InternalInvariantViolation,
}

impl VmFault {
    pub fn error_number(self) -> Option<&'static str> {
        let code = match self {
            Self::MetricCounterOverflow(_) => "metric_counter_overflow",
            Self::GlobalTickOverflow => "global_tick_overflow",
            Self::InternalInvariantViolation => "internal_invariant_violation",
        };
        codegrid_model::error_number("fault", code)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VmSnapshot {
    pub status: VmStatus,
    pub committed_ticks: u64,
    pub registers: [Value; 10],
    pub memory: Memory,
    pub input: Vec<Value>,
    pub output: Vec<Value>,
    /// The mutable outer copy, including any committed self-modifying writes.
    pub runtime_program: ScopedProgram,
    pub threads: Vec<ThreadSnapshot>,
    pub metrics: RuntimeMetrics,
    pub errors: Vec<RuntimeError>,
    pub fault: Option<VmFault>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreadPhaseSnapshot {
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
pub struct CallFrameSnapshot {
    pub caller_board: codegrid_ir::BoardId,
    pub call_position: Coordinate,
    pub saved_direction: Direction,
    pub saved_registers: Option<[Value; 10]>,
    pub saved_register_pointer: u8,
    pub saved_status_flag: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreadSnapshot {
    pub id: u64,
    pub board: codegrid_ir::BoardId,
    pub position: Coordinate,
    pub direction: Direction,
    pub register_pointer: u8,
    pub status_flag: u8,
    pub private_registers: Option<[Value; 10]>,
    pub page: crate::Page,
    pub data_stack: Vec<Value>,
    pub instruction_stack: Vec<InstructionStackItem>,
    pub call_stack: Vec<CallFrameSnapshot>,
    pub phase: ThreadPhaseSnapshot,
    pub random_state: u64,
}

impl From<&ThreadState> for ThreadSnapshot {
    fn from(thread: &ThreadState) -> Self {
        let phase = match thread.phase {
            ExecutionPhase::Normal => ThreadPhaseSnapshot::Normal,
            ExecutionPhase::AfterCall => ThreadPhaseSnapshot::AfterCall,
            ExecutionPhase::Repeat { total, completed } => {
                ThreadPhaseSnapshot::Repeat { total, completed }
            }
            ExecutionPhase::Fold {
                fold_id,
                internal_position,
                internal_direction,
                saved_outer_direction,
            } => ThreadPhaseSnapshot::Fold {
                fold_id,
                internal_position,
                internal_direction,
                saved_outer_direction,
            },
            ExecutionPhase::FoldResume {
                saved_outer_direction,
            } => ThreadPhaseSnapshot::FoldResume {
                saved_outer_direction,
            },
            ExecutionPhase::Terminated => ThreadPhaseSnapshot::Terminated,
        };

        Self {
            id: thread.id,
            board: thread.board,
            position: thread.position,
            direction: thread.direction,
            register_pointer: thread.register_pointer,
            status_flag: thread.status_flag,
            private_registers: thread.private_registers,
            page: thread.page.clone(),
            data_stack: thread.data_stack.clone(),
            instruction_stack: thread.instruction_stack.clone(),
            call_stack: thread
                .call_stack
                .iter()
                .map(|frame| CallFrameSnapshot {
                    caller_board: frame.caller_board,
                    call_position: frame.call_position,
                    saved_direction: frame.saved_direction,
                    saved_registers: frame.saved_registers,
                    saved_register_pointer: frame.saved_register_pointer,
                    saved_status_flag: frame.saved_status_flag,
                })
                .collect(),
            phase,
            random_state: thread.rng.state(),
        }
    }
}

#[cfg(test)]
include!("state_tests.rs");
