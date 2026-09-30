use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::num::NonZeroU64;

use crate::effects::{resolve_shared_conflicts, SharedEffectBatch};
use crate::thread::ThreadState;
use crate::{
    Coordinate, ExecutionScope, InstructionKind, MemoryLocationId, MetricCounterOverflow,
    RuntimeError, RuntimeMetricSummary, StaticCellId, ThreadSnapshot, Vm, VmFault, VmStatus,
};
use codegrid_ir::{Board, BoardId, CodeGridId, ScopedProgram};
use codegrid_model::{PrimaryInstruction, Value};

mod full;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepResult {
    /// One-based number of the attempted Global Tick.
    pub attempted_tick: u64,
    /// Number of Global Ticks committed after this attempt completes.
    pub committed_ticks: u64,
    /// Cumulative raw metrics after this attempt, including failed work.
    pub metrics: RuntimeMetricSummary,
    /// Ordered execution and committed-state events for this Global Tick.
    /// Failed or rolled-back attempts return no events.
    pub events: Vec<VmEvent>,
    pub status: VmStatus,
    pub newly_emitted_output: Vec<Value>,
    pub errors: Vec<RuntimeError>,
    pub fault: Option<VmFault>,
}

/// A host-neutral event emitted by a successfully committed Global Tick.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VmEvent {
    /// A static cell reached during outer or Custom execution.
    CellReached {
        scope: ExecutionScope,
        thread_id: u64,
        cell: StaticCellId,
    },
    /// A value removed from the outer input queue by this transaction.
    InputConsumed {
        scope: ExecutionScope,
        thread_id: u64,
        value: Value,
    },
    /// A committed value change in the selected execution scope.
    RegisterChanged {
        scope: ExecutionScope,
        register: u8,
        old: Value,
        new: Value,
    },
    /// A committed sparse-memory value change.
    MemoryChanged {
        scope: ExecutionScope,
        location: MemoryLocationId,
        old: Value,
        new: Value,
    },
    /// A committed self-modifying-code change.
    CodeChanged {
        scope: ExecutionScope,
        cell: StaticCellId,
        old: Option<PrimaryInstruction>,
        new: Option<PrimaryInstruction>,
    },
    /// A committed thread snapshot change within the selected execution scope.
    ThreadChanged {
        scope: ExecutionScope,
        before: ThreadSnapshot,
        after: ThreadSnapshot,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunOutcome {
    Halted,
    Error,
    TickLimitReached,
    InvalidTickLimit,
    WorkLimitReached,
}

/// Committed output from a bounded sequence of Global Ticks.
///
/// If a work ceiling interrupts a tick, earlier completed ticks remain in
/// these deltas while the interrupted tick contributes no state or metrics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunResult {
    pub outcome: RunOutcome,
    pub events: Vec<VmEvent>,
    pub newly_emitted_output: Vec<Value>,
    pub work_limit_exceeded: Option<WorkLimitExceeded>,
}

/// A deterministic host work ceiling interrupted an uncommitted Global Tick.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkLimitExceeded {
    pub maximum_work_units: u64,
}

#[derive(Clone, Copy)]
struct ExecutionWorkBudget {
    maximum: Option<u64>,
    consumed: u64,
}

impl ExecutionWorkBudget {
    const fn unlimited() -> Self {
        Self {
            maximum: None,
            consumed: 0,
        }
    }

    const fn limited(maximum: NonZeroU64) -> Self {
        Self {
            maximum: Some(maximum.get()),
            consumed: 0,
        }
    }

    fn consume(&mut self) -> bool {
        if self.maximum.is_some_and(|maximum| self.consumed >= maximum) {
            return false;
        }
        self.consumed = self.consumed.saturating_add(1);
        true
    }

    fn exceeded(&self) -> Option<WorkLimitExceeded> {
        self.maximum
            .filter(|maximum| self.consumed >= *maximum)
            .map(|maximum_work_units| WorkLimitExceeded { maximum_work_units })
    }
}

#[derive(Default)]
struct TickDraft {
    effects: SharedEffectBatch,
    events: Vec<VmEvent>,
    committed_events: Vec<VmEvent>,
    errors: Vec<RuntimeError>,
    halt_requested: bool,
    work_limit_exceeded: bool,
    fault: Option<VmFault>,
    custom_stack_samples: BTreeMap<u64, StackUsage>,
    custom_caller_stack_samples: BTreeMap<u64, BTreeMap<u64, u64>>,
}

#[derive(Clone, Copy, Default)]
struct StackUsage {
    data: u64,
    instruction: u64,
    call: u64,
}

#[derive(Clone, Copy)]
struct ExecutionMode {
    code_grid: CodeGridId,
    scope: ExecutionScope,
}

impl ExecutionMode {
    fn outer() -> Self {
        Self {
            code_grid: CodeGridId::Outer,
            scope: ExecutionScope::Outer,
        }
    }
}

impl Vm {
    /// Evaluates and atomically commits one outer Global Tick.
    pub fn step(&mut self) -> StepResult {
        let mut work_budget = ExecutionWorkBudget::unlimited();
        self.step_with_budget(&mut work_budget)
            .expect("an unlimited execution budget cannot be exhausted")
    }

    /// Evaluates one outer Global Tick with a deterministic host work ceiling.
    ///
    /// One work unit is one outer or Custom thread dispatch. Exceeding the
    /// ceiling rolls back the entire attempted Global Tick and leaves VM state
    /// and normative metrics unchanged.
    pub fn step_with_work_limit(
        &mut self,
        maximum_work_units: NonZeroU64,
    ) -> Result<StepResult, WorkLimitExceeded> {
        let mut work_budget = ExecutionWorkBudget::limited(maximum_work_units);
        self.step_with_budget(&mut work_budget)
    }

    fn step_with_budget(
        &mut self,
        work_budget: &mut ExecutionWorkBudget,
    ) -> Result<StepResult, WorkLimitExceeded> {
        if self.status != VmStatus::Running {
            return Ok(StepResult {
                attempted_tick: self.metrics.global_tick(),
                committed_ticks: self.metrics.global_tick(),
                metrics: self.metrics.summary(),
                events: Vec::new(),
                status: self.status,
                newly_emitted_output: Vec::new(),
                errors: self.terminal_errors.clone(),
                fault: self.terminal_fault,
            });
        }

        let Some(attempted_tick) = self.metrics.global_tick().checked_add(1) else {
            self.status = VmStatus::Error;
            self.terminal_fault = Some(VmFault::GlobalTickOverflow);
            return Ok(StepResult {
                attempted_tick: u64::MAX,
                committed_ticks: self.metrics.global_tick(),
                metrics: self.metrics.summary(),
                events: Vec::new(),
                status: self.status,
                newly_emitted_output: Vec::new(),
                errors: Vec::new(),
                fault: self.terminal_fault,
            });
        };

        let mut attempted_metrics = self.metrics.begin_tick_attempt();
        let initial_stack_fault = observe_stacks(&mut attempted_metrics, &self.threads);

        // Every thread reads these tick-start snapshots. Effects are committed
        // only after all local failures and shared conflicts have been found.
        let registers = self.registers;
        let input: VecDeque<Value> = self.input.clone();
        let memory = self.memory.clone();
        let runtime_start = self.runtime_program.clone();
        let verified_program = self.verified_program.clone();
        let mut staged_threads = self.threads.clone();
        let mut draft = TickDraft {
            fault: initial_stack_fault,
            ..TickDraft::default()
        };

        for thread in &mut staged_threads {
            full::evaluate_thread(
                thread,
                attempted_tick,
                &registers,
                &memory,
                &input,
                None,
                &runtime_start,
                &verified_program,
                self.config,
                &mut attempted_metrics,
                &mut draft,
                ExecutionMode::outer(),
                work_budget,
            );
            if draft.work_limit_exceeded {
                break;
            }
        }

        if draft.work_limit_exceeded {
            return Err(work_budget
                .exceeded()
                .expect("work limit flag requires an exhausted work budget"));
        }

        draft.errors.extend(resolve_shared_conflicts(
            &draft.effects,
            !input.is_empty(),
            false,
            attempted_tick,
            ExecutionScope::Outer,
        ));
        RuntimeError::sort_canonical(&mut draft.errors);
        if draft.fault.is_none() {
            match stack_usage_with_growth(&self.threads, &staged_threads) {
                Ok(outer_usage) => {
                    attempted_metrics.observe_stack_usage(
                        outer_usage.data,
                        outer_usage.instruction,
                        outer_usage.call,
                    );
                    for (internal_tick, custom_usage) in &draft.custom_stack_samples {
                        let mut aligned = outer_usage;
                        if let Some(caller_samples) =
                            draft.custom_caller_stack_samples.get(internal_tick)
                        {
                            for (caller_id, sampled_depth) in caller_samples {
                                let before_depth =
                                    match stack_depth_for_thread(&self.threads, *caller_id) {
                                        Ok(depth) => depth,
                                        Err(fault) => {
                                            draft.fault = Some(fault);
                                            break;
                                        }
                                    };
                                let after_depth =
                                    match stack_depth_for_thread(&staged_threads, *caller_id) {
                                        Ok(depth) => depth,
                                        Err(fault) => {
                                            draft.fault = Some(fault);
                                            break;
                                        }
                                    };
                                aligned = match replace_caller_stack_sample(
                                    aligned,
                                    before_depth,
                                    after_depth,
                                    *sampled_depth,
                                ) {
                                    Ok(aligned) => aligned,
                                    Err(fault) => {
                                        draft.fault = Some(fault);
                                        break;
                                    }
                                };
                            }
                        }
                        if draft.fault.is_some() {
                            break;
                        }
                        let aligned = match add_stack_usage(aligned, *custom_usage) {
                            Ok(aligned) => aligned,
                            Err(fault) => {
                                draft.fault = Some(fault);
                                break;
                            }
                        };
                        attempted_metrics.observe_stack_usage(
                            aligned.data,
                            aligned.instruction,
                            aligned.call,
                        );
                    }
                    if draft.fault.is_none() {
                        match stack_usage(&staged_threads) {
                            Ok(committed_usage) => attempted_metrics.observe_stack_usage(
                                committed_usage.data,
                                committed_usage.instruction,
                                committed_usage.call,
                            ),
                            Err(fault) => draft.fault = Some(fault),
                        }
                    }
                }
                Err(fault) => draft.fault = Some(fault),
            }
        }

        if draft.fault.is_some() {
            self.metrics.merge_tick_attempt(attempted_metrics);
            self.status = VmStatus::Error;
            self.terminal_fault = draft.fault;
            return Ok(StepResult {
                attempted_tick,
                committed_ticks: self.metrics.global_tick(),
                metrics: self.metrics.summary(),
                events: Vec::new(),
                status: self.status,
                newly_emitted_output: Vec::new(),
                errors: Vec::new(),
                fault: self.terminal_fault,
            });
        }

        if !draft.errors.is_empty() {
            self.metrics.merge_tick_attempt(attempted_metrics);
            self.status = VmStatus::Error;
            self.terminal_errors = draft.errors.clone();
            return Ok(StepResult {
                attempted_tick,
                committed_ticks: self.metrics.global_tick(),
                metrics: self.metrics.summary(),
                events: Vec::new(),
                status: self.status,
                newly_emitted_output: Vec::new(),
                errors: draft.errors,
                fault: None,
            });
        }

        if self.metrics.global_tick() == u64::MAX {
            self.status = VmStatus::Error;
            self.terminal_fault = Some(VmFault::GlobalTickOverflow);
            return Ok(StepResult {
                attempted_tick,
                committed_ticks: self.metrics.global_tick(),
                metrics: self.metrics.summary(),
                events: Vec::new(),
                status: self.status,
                newly_emitted_output: Vec::new(),
                errors: Vec::new(),
                fault: self.terminal_fault,
            });
        }

        let output_start = self.output.len();
        commit_effects(
            &mut self.registers,
            &mut self.memory,
            &mut self.input,
            &mut self.output,
            &mut self.runtime_program,
            &draft.effects,
        );
        let mut events = std::mem::take(&mut draft.events);
        events.sort_by_key(cell_visit_order);
        draft
            .committed_events
            .sort_by_key(committed_state_event_order);
        events.append(&mut draft.committed_events);
        events.extend(committed_state_events(
            ExecutionScope::Outer,
            &registers,
            &self.registers,
            &memory,
            &self.memory,
            &runtime_start,
            &self.runtime_program,
            &self.threads,
            &staged_threads,
            &draft.effects,
        ));
        self.threads = staged_threads;
        let committed = attempted_metrics.commit_global_tick().is_ok();
        self.metrics.merge_tick_attempt(attempted_metrics);
        if !committed {
            // The explicit preflight above makes this unreachable unless the
            // metrics implementation changes independently.
            self.status = VmStatus::Error;
            self.terminal_fault = Some(VmFault::MetricCounterOverflow(
                MetricCounterOverflow::GlobalTick,
            ));
        } else if draft.halt_requested {
            self.status = VmStatus::Halted;
        }

        Ok(StepResult {
            attempted_tick,
            committed_ticks: self.metrics.global_tick(),
            metrics: self.metrics.summary(),
            events: if committed { events } else { Vec::new() },
            status: self.status,
            newly_emitted_output: self.output[output_start..].to_vec(),
            errors: Vec::new(),
            fault: self.terminal_fault,
        })
    }

    /// Runs at most `max_ticks` attempted Global Ticks using [`Vm::step`].
    pub fn run(&mut self, max_ticks: u64) -> RunOutcome {
        if max_ticks == 0 {
            return RunOutcome::InvalidTickLimit;
        }
        for _ in 0..max_ticks {
            match self.step().status {
                VmStatus::Running => {}
                VmStatus::Halted => return RunOutcome::Halted,
                VmStatus::Error => return RunOutcome::Error,
            }
        }
        match self.status {
            VmStatus::Halted => RunOutcome::Halted,
            VmStatus::Error => RunOutcome::Error,
            VmStatus::Running => RunOutcome::TickLimitReached,
        }
    }

    /// Runs a bounded slice under one shared work ceiling across all attempted
    /// Global Ticks. Earlier complete ticks remain committed if a later tick
    /// exceeds the budget; the interrupted tick is rolled back atomically.
    pub fn run_with_work_limit(
        &mut self,
        max_ticks: u64,
        maximum_work_units: NonZeroU64,
    ) -> Result<RunOutcome, WorkLimitExceeded> {
        if max_ticks == 0 {
            return Ok(RunOutcome::InvalidTickLimit);
        }
        let mut work_budget = ExecutionWorkBudget::limited(maximum_work_units);
        for _ in 0..max_ticks {
            let _ = self.step_with_budget(&mut work_budget)?;
            match self.status {
                VmStatus::Running => {}
                VmStatus::Halted => return Ok(RunOutcome::Halted),
                VmStatus::Error => return Ok(RunOutcome::Error),
            }
        }
        Ok(match self.status {
            VmStatus::Halted => RunOutcome::Halted,
            VmStatus::Error => RunOutcome::Error,
            VmStatus::Running => RunOutcome::TickLimitReached,
        })
    }

    /// Runs bounded ticks and retains committed events/output when a work
    /// ceiling yields partway through the requested run.
    pub fn run_with_work_limit_detailed(
        &mut self,
        max_ticks: u64,
        maximum_work_units: NonZeroU64,
    ) -> RunResult {
        if max_ticks == 0 {
            return RunResult {
                outcome: RunOutcome::InvalidTickLimit,
                events: Vec::new(),
                newly_emitted_output: Vec::new(),
                work_limit_exceeded: None,
            };
        }

        let mut work_budget = ExecutionWorkBudget::limited(maximum_work_units);
        let mut events = Vec::new();
        let mut newly_emitted_output = Vec::new();
        for _ in 0..max_ticks {
            let step = match self.step_with_budget(&mut work_budget) {
                Ok(step) => step,
                Err(work_limit_exceeded) => {
                    return RunResult {
                        outcome: RunOutcome::WorkLimitReached,
                        events,
                        newly_emitted_output,
                        work_limit_exceeded: Some(work_limit_exceeded),
                    };
                }
            };
            events.extend(step.events);
            newly_emitted_output.extend(step.newly_emitted_output);
            match step.status {
                VmStatus::Running => {}
                VmStatus::Halted => {
                    return RunResult {
                        outcome: RunOutcome::Halted,
                        events,
                        newly_emitted_output,
                        work_limit_exceeded: None,
                    };
                }
                VmStatus::Error => {
                    return RunResult {
                        outcome: RunOutcome::Error,
                        events,
                        newly_emitted_output,
                        work_limit_exceeded: None,
                    };
                }
            }
        }

        RunResult {
            outcome: RunOutcome::TickLimitReached,
            events,
            newly_emitted_output,
            work_limit_exceeded: None,
        }
    }
}

fn record_cell_visit(
    metrics: &mut crate::RuntimeMetrics,
    draft: &mut TickDraft,
    thread: &ThreadState,
    cell: StaticCellId,
    scope: ExecutionScope,
) {
    metrics.record_cell(cell);
    draft.events.push(VmEvent::CellReached {
        scope,
        thread_id: thread.id,
        cell,
    });
}

fn committed_state_events(
    scope: ExecutionScope,
    before_registers: &[Value; 10],
    registers: &[Value; 10],
    before_memory: &crate::Memory,
    memory: &crate::Memory,
    before_runtime: &ScopedProgram,
    runtime: &ScopedProgram,
    before_threads: &[ThreadState],
    threads: &[ThreadState],
    effects: &SharedEffectBatch,
) -> Vec<VmEvent> {
    let mut events = Vec::new();

    for read in &effects.input_reads {
        if let Some(value) = read.value {
            events.push(VmEvent::InputConsumed {
                scope,
                thread_id: read.thread_id,
                value,
            });
        }
    }

    for (register, (old, new)) in before_registers.iter().zip(registers).enumerate() {
        if old != new {
            events.push(VmEvent::RegisterChanged {
                scope,
                register: register as u8,
                old: *old,
                new: *new,
            });
        }
    }

    let memory_locations = effects
        .memory_writes
        .iter()
        .map(|write| write.location.clone())
        .collect::<BTreeSet<_>>();
    for location in memory_locations {
        let old = before_memory.read(&location.address);
        let new = memory.read(&location.address);
        if old != new {
            events.push(VmEvent::MemoryChanged {
                scope,
                location,
                old,
                new,
            });
        }
    }

    let code_cells = effects
        .code_writes
        .iter()
        .map(|write| write.cell)
        .collect::<BTreeSet<_>>();
    for cell in code_cells {
        let old = runtime_cell(before_runtime, cell.board, cell.position)
            .and_then(|source_cell| source_cell.primary);
        let new = runtime_cell(runtime, cell.board, cell.position)
            .and_then(|source_cell| source_cell.primary);
        if old != new {
            events.push(VmEvent::CodeChanged {
                scope,
                cell,
                old,
                new,
            });
        }
    }

    for (before, after) in before_threads.iter().zip(threads) {
        let before = ThreadSnapshot::from(before);
        let after = ThreadSnapshot::from(after);
        if before != after {
            events.push(VmEvent::ThreadChanged {
                scope,
                before,
                after,
            });
        }
    }

    events
}

fn cell_visit_order(event: &VmEvent) -> (u64, u8, u64, u64) {
    match event {
        VmEvent::CellReached {
            scope: ExecutionScope::Outer,
            thread_id,
            ..
        } => (*thread_id, 0, 0, *thread_id),
        VmEvent::CellReached {
            scope:
                ExecutionScope::Custom {
                    caller_thread_id,
                    internal_tick,
                    ..
                },
            thread_id,
            ..
        } => (*caller_thread_id, 1, *internal_tick, *thread_id),
        _ => (u64::MAX, u8::MAX, u64::MAX, u64::MAX),
    }
}

fn committed_state_event_order(event: &VmEvent) -> (u8, u64, u64, u8) {
    let scope = match event {
        VmEvent::CellReached { scope, .. }
        | VmEvent::InputConsumed { scope, .. }
        | VmEvent::RegisterChanged { scope, .. }
        | VmEvent::MemoryChanged { scope, .. }
        | VmEvent::CodeChanged { scope, .. }
        | VmEvent::ThreadChanged { scope, .. } => *scope,
    };
    match scope {
        ExecutionScope::Custom {
            caller_thread_id,
            custom_id,
            internal_tick,
        } => (0, internal_tick, caller_thread_id, custom_id.get()),
        ExecutionScope::Outer => (1, 0, 0, 0),
    }
}

fn record_operation(
    metrics: &mut crate::RuntimeMetrics,
    kind: InstructionKind,
    draft: &mut TickDraft,
) {
    if let Err(error) = metrics.record_operation(kind) {
        draft.fault = Some(VmFault::MetricCounterOverflow(error));
    }
}

fn observe_stacks(metrics: &mut crate::RuntimeMetrics, threads: &[ThreadState]) -> Option<VmFault> {
    let usage = match stack_usage(threads) {
        Ok(usage) => usage,
        Err(fault) => return Some(fault),
    };
    metrics.observe_stack_usage(usage.data, usage.instruction, usage.call);
    None
}

fn stack_usage(threads: &[ThreadState]) -> Result<StackUsage, VmFault> {
    let mut usage = StackUsage::default();
    for thread in threads {
        if let Some((d, i, c)) = thread.stack_depths() {
            usage.data = usage.data.checked_add(d).ok_or_else(|| {
                VmFault::MetricCounterOverflow(MetricCounterOverflow::DataStackUsage)
            })?;
            usage.instruction = usage.instruction.checked_add(i).ok_or_else(|| {
                VmFault::MetricCounterOverflow(MetricCounterOverflow::InstructionStackUsage)
            })?;
            usage.call = usage.call.checked_add(c).ok_or_else(|| {
                VmFault::MetricCounterOverflow(MetricCounterOverflow::CallStackUsage)
            })?;
        } else {
            return Err(VmFault::MetricCounterOverflow(
                MetricCounterOverflow::DataStackUsage,
            ));
        }
    }
    Ok(usage)
}

fn stack_depth_for_thread(threads: &[ThreadState], thread_id: u64) -> Result<u64, VmFault> {
    let thread = threads
        .iter()
        .find(|thread| thread.id == thread_id)
        .ok_or(VmFault::InternalInvariantViolation)?;
    thread
        .stack_depths()
        .map(|(data, _, _)| data)
        .ok_or(VmFault::MetricCounterOverflow(
            MetricCounterOverflow::DataStackUsage,
        ))
}

fn add_stack_usage(lhs: StackUsage, rhs: StackUsage) -> Result<StackUsage, VmFault> {
    Ok(StackUsage {
        data: lhs
            .data
            .checked_add(rhs.data)
            .ok_or(VmFault::MetricCounterOverflow(
                MetricCounterOverflow::DataStackUsage,
            ))?,
        instruction: lhs.instruction.checked_add(rhs.instruction).ok_or(
            VmFault::MetricCounterOverflow(MetricCounterOverflow::InstructionStackUsage),
        )?,
        call: lhs
            .call
            .checked_add(rhs.call)
            .ok_or(VmFault::MetricCounterOverflow(
                MetricCounterOverflow::CallStackUsage,
            ))?,
    })
}

fn replace_caller_stack_sample(
    mut usage: StackUsage,
    before_depth: u64,
    after_depth: u64,
    sampled_depth: u64,
) -> Result<StackUsage, VmFault> {
    let baseline_depth = before_depth.max(after_depth);
    let Some(without_caller) = usage.data.checked_sub(baseline_depth) else {
        return Err(VmFault::InternalInvariantViolation);
    };
    usage.data =
        without_caller
            .checked_add(sampled_depth)
            .ok_or(VmFault::MetricCounterOverflow(
                MetricCounterOverflow::DataStackUsage,
            ))?;
    Ok(usage)
}

fn stack_usage_with_growth(
    before: &[ThreadState],
    after: &[ThreadState],
) -> Result<StackUsage, VmFault> {
    let mut usage = stack_usage(before)?;
    let mut before_by_id = BTreeMap::new();
    for thread in before {
        before_by_id.insert(
            thread.id,
            thread.stack_depths().ok_or(VmFault::MetricCounterOverflow(
                MetricCounterOverflow::DataStackUsage,
            ))?,
        );
    }

    for thread in after {
        let after_depths = thread.stack_depths().ok_or(VmFault::MetricCounterOverflow(
            MetricCounterOverflow::DataStackUsage,
        ))?;
        let before_depths = before_by_id.get(&thread.id).copied().unwrap_or_default();
        usage.data = usage
            .data
            .checked_add(after_depths.0.saturating_sub(before_depths.0))
            .ok_or(VmFault::MetricCounterOverflow(
                MetricCounterOverflow::DataStackUsage,
            ))?;
        usage.instruction = usage
            .instruction
            .checked_add(after_depths.1.saturating_sub(before_depths.1))
            .ok_or(VmFault::MetricCounterOverflow(
                MetricCounterOverflow::InstructionStackUsage,
            ))?;
        usage.call = usage
            .call
            .checked_add(after_depths.2.saturating_sub(before_depths.2))
            .ok_or(VmFault::MetricCounterOverflow(
                MetricCounterOverflow::CallStackUsage,
            ))?;
    }
    Ok(usage)
}

fn runtime_board(program: &ScopedProgram, id: BoardId) -> Option<&Board> {
    match id {
        BoardId::Main => Some(&program.main),
        BoardId::Function(slot) => program.functions.get(&slot),
    }
}

fn runtime_cell(
    program: &ScopedProgram,
    board: BoardId,
    position: Coordinate,
) -> Option<&codegrid_ir::Cell> {
    runtime_board(program, board)?.cell(position.x, position.y)
}

fn commit_effects(
    registers: &mut [Value; 10],
    memory: &mut crate::Memory,
    input: &mut VecDeque<Value>,
    output: &mut Vec<Value>,
    runtime: &mut ScopedProgram,
    effects: &SharedEffectBatch,
) {
    for write in &effects.register_writes {
        registers[usize::from(write.register)] = write.value;
    }
    if let Some(read) = effects.input_reads.iter().find(|read| read.value.is_some()) {
        if let Some(value) = read.value {
            registers[usize::from(read.register)] = value;
        }
    }
    if let Some(read) = effects
        .caller_stack_reads
        .iter()
        .find(|read| read.value.is_some())
    {
        if let Some(value) = read.value {
            registers[usize::from(read.register)] = value;
        }
    }
    if !effects.input_reads.is_empty() && effects.input_reads[0].value.is_some() {
        input.pop_front();
    }
    output.extend(effects.output_writes.iter().map(|write| write.value));
    for write in &effects.memory_writes {
        memory.write(write.location.address.clone(), write.value);
    }
    for write in &effects.code_writes {
        let board = match write.cell.board {
            BoardId::Main => Some(&mut runtime.main),
            BoardId::Function(slot) => runtime.functions.get_mut(&slot),
        };
        if let Some(board) = board {
            let index = write.cell.position.y * board.width + write.cell.position.x;
            if let Some(cell) = board.cells.get_mut(index) {
                cell.primary = write.primary;
            }
        }
    }
}

#[cfg(test)]
mod stack_high_water_formula_tests {
    use super::{
        add_stack_usage, replace_caller_stack_sample, stack_usage, stack_usage_with_growth,
    };
    use crate::thread::{CallFrame, ThreadState};
    use crate::{Coordinate, RuntimeMetrics};
    use codegrid_ir::BoardId;
    use codegrid_model::{Direction, InstructionStackItem};

    fn thread_with_stacks(id: u64, data: usize, instruction: usize, call: usize) -> ThreadState {
        let mut thread = ThreadState::initial(
            id,
            Coordinate {
                x: id as usize,
                y: 0,
            },
            Direction::Right,
            id,
        );
        thread.data_stack = vec![0; data];
        thread.instruction_stack = vec![InstructionStackItem::Empty; instruction];
        thread.call_stack = (0..call)
            .map(|_| CallFrame {
                caller_board: BoardId::Main,
                call_position: Coordinate { x: 0, y: 0 },
                saved_direction: Direction::Right,
            })
            .collect();
        thread
    }

    #[test]
    fn failed_aligned_custom_tick_preserves_the_aggregate_stack_high_water_formula() {
        let outer_before = vec![
            thread_with_stacks(0, 2, 2, 1),
            thread_with_stacks(1, 1, 1, 0),
        ];
        let staged_outer = vec![
            thread_with_stacks(0, 1, 3, 2),
            thread_with_stacks(1, 2, 0, 0),
        ];

        // Keep the attempted baseline, add each thread's positive staged
        // growth, and do not let staged pops reduce any stack peak.
        let outer = stack_usage_with_growth(&outer_before, &staged_outer)
            .expect("outer stack totals fit the metric counters");
        assert_eq!((outer.data, outer.instruction, outer.call), (4, 4, 2));

        // At an aligned Custom boundary, replace the caller's single outer
        // sample with its internal-tick depth, then add every Custom thread.
        let aligned_outer = replace_caller_stack_sample(outer, 2, 1, 3)
            .expect("caller stack is counted once at the aligned sample");
        let custom_threads = vec![
            thread_with_stacks(10, 1, 1, 1),
            thread_with_stacks(11, 2, 0, 0),
        ];
        let custom = stack_usage(&custom_threads).expect("Custom stack totals fit");
        let attempted =
            add_stack_usage(aligned_outer, custom).expect("aligned Outer and Custom totals fit");
        assert_eq!(
            (attempted.data, attempted.instruction, attempted.call),
            (8, 5, 3)
        );

        // A runtime-error tick merges attempted peaks while retaining any
        // earlier committed high-water values.
        let mut committed_metrics = RuntimeMetrics::default();
        committed_metrics.observe_stack_usage(1, 1, 1);
        let mut failed_attempt = committed_metrics.begin_tick_attempt();
        failed_attempt.observe_stack_usage(attempted.data, attempted.instruction, attempted.call);
        committed_metrics.merge_tick_attempt(failed_attempt);
        assert_eq!(
            (
                committed_metrics.peak_data_stack_usage(),
                committed_metrics.peak_instruction_stack_usage(),
                committed_metrics.peak_call_stack_usage(),
            ),
            (8, 5, 3)
        );
    }
}
