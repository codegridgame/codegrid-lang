use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::num::NonZeroU64;

use crate::effects::{resolve_shared_conflicts, SharedEffectBatch};
use crate::thread::ThreadState;
use crate::{
    Coordinate, ExecutionScope, InstructionKind, MemoryLocationId, MetricCounterOverflow,
    RuntimeError, RuntimeMetricSummary, StaticCellId, StepFootprint, ThreadSnapshot, Vm, VmFault,
    VmStatus,
};
use codegrid_ir::{Board, BoardId, CodeGridId, ScopedProgram};
use codegrid_model::{PrimaryInstruction, Value};

mod full;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepResult {
    /// One-based identifier of the attempted atomic transition.
    pub attempted_transition_id: u64,
    /// Number of atomic transitions committed after this attempt completes.
    pub committed_steps: u64,
    /// Committed-step summary after this attempt.
    pub metrics: RuntimeMetricSummary,
    /// Ordered execution and committed-state events for this transition.
    /// Failed or rolled-back attempts return no events.
    pub events: Vec<VmEvent>,
    pub status: VmStatus,
    pub newly_emitted_output: Vec<Value>,
    pub errors: Vec<RuntimeError>,
    pub fault: Option<VmFault>,
}

/// Diagnostics and raw observations produced for one candidate transition.
///
/// The footprint contains no host prices or budgets. Events and newly emitted
/// output become observable only from [`PreparedStep::commit`]. `work_units`
/// reports the dispatches consumed while evaluating this candidate; it is not
/// retained in VM state or cumulative metrics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepReport {
    pub footprint: StepFootprint,
    /// Outer and Custom thread dispatches consumed by this candidate.
    pub work_units: u64,
    /// Custom-scope thread dispatches consumed by this candidate, including
    /// dispatches in nested Custom invocations and excluding Outer dispatches.
    pub custom_internal_dispatches: u64,
    /// Candidate status if the pending transition is committed.
    pub candidate_status: VmStatus,
    pub errors: Vec<RuntimeError>,
    pub fault: Option<VmFault>,
}

/// A host-neutral event emitted by a successfully committed transition.
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
    StepLimitReached,
    InvalidStepLimit,
    WorkLimitReached,
}

/// Committed output from a bounded sequence of VM transitions.
///
/// If a work ceiling interrupts a step, earlier completed steps remain in
/// these deltas while the interrupted step contributes no state or metrics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunResult {
    pub outcome: RunOutcome,
    pub events: Vec<VmEvent>,
    pub newly_emitted_output: Vec<Value>,
    pub work_limit_exceeded: Option<WorkLimitExceeded>,
}

/// A deterministic host work ceiling interrupted an uncommitted transition.
///
/// The VM checks the ceiling before each dispatch. Therefore an interruption
/// means `maximum_work_units` dispatches were consumed, so callers can account
/// for the attempted candidate exactly from these values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkLimitExceeded {
    pub maximum_work_units: u64,
    /// The Custom-scope subset of `maximum_work_units`, including logical
    /// dispatches skipped after the VM proves a deterministic Custom cycle.
    pub custom_internal_dispatches: u64,
}

/// A prepared, exclusively borrowed VM transition awaiting one decision.
///
/// Keeping this value alive holds the VM's mutable borrow, so the host cannot
/// step the VM or change its input before deciding. Dropping the value aborts
/// the pending transition. `commit` and `abort` consume it, making a second
/// completion impossible in safe Rust.
///
/// ```compile_fail
/// use codegrid_vm::Vm;
/// fn cannot_step_while_pending(vm: &mut Vm) {
///     let pending = vm.prepare_step();
///     vm.step();
///     pending.abort();
/// }
/// ```
///
/// ```compile_fail
/// use codegrid_vm::Vm;
/// fn cannot_replace_input_while_pending(vm: &mut Vm) {
///     let pending = vm.prepare_step();
///     vm.replace_input(&[1]);
///     pending.commit();
/// }
/// ```
///
/// ```compile_fail
/// use codegrid_vm::Vm;
/// fn can_complete_only_once(vm: &mut Vm) {
///     let pending = vm.prepare_step();
///     let _ = pending.commit();
///     let _ = pending.abort();
/// }
/// ```
pub struct PreparedStep<'a> {
    vm: &'a mut Vm,
    before: Option<VmCheckpoint>,
    result: Option<StepResult>,
    report: StepReport,
}

struct VmCheckpoint {
    status: VmStatus,
    registers: [Value; 10],
    memory: crate::Memory,
    input: VecDeque<Value>,
    output: Vec<Value>,
    runtime_program: ScopedProgram,
    threads: Vec<ThreadState>,
    metrics: crate::RuntimeMetrics,
    terminal_errors: Vec<RuntimeError>,
    terminal_fault: Option<VmFault>,
}

impl VmCheckpoint {
    fn capture(vm: &Vm) -> Self {
        Self {
            status: vm.status,
            registers: vm.registers,
            memory: vm.memory.clone(),
            input: vm.input.clone(),
            output: vm.output.clone(),
            runtime_program: vm.runtime_program.clone(),
            threads: vm.threads.clone(),
            metrics: vm.metrics.clone(),
            terminal_errors: vm.terminal_errors.clone(),
            terminal_fault: vm.terminal_fault,
        }
    }
}

impl PreparedStep<'_> {
    /// Returns candidate observations and diagnostics without exposing staged
    /// events or output.
    pub const fn report(&self) -> &StepReport {
        &self.report
    }

    /// Returns a borrowed read-only view of the VM state with this candidate
    /// transition applied.
    ///
    /// The view is valid only while this prepared transition is alive. It may
    /// be inspected to make a host decision before commit or abort, but must
    /// not be serialized or published as committed state. This view contains
    /// only generic VM state and does not apply host-specific policy.
    pub fn candidate_snapshot_view(&self) -> crate::VmSnapshotView<'_> {
        self.vm.snapshot_view()
    }

    /// Makes the prepared transition and its events/output visible.
    pub fn commit(mut self) -> StepResult {
        self.before.take();
        self.result
            .take()
            .expect("a pending step can be completed only once")
    }

    /// Discards every staged VM change and returns diagnostics for the
    /// attempted candidate. The report does not contain events or output.
    pub fn abort(mut self) -> StepReport {
        if let Some(before) = self.before.take() {
            restore_checkpoint(self.vm, before);
        }
        self.result.take();
        self.report.clone()
    }
}

impl Drop for PreparedStep<'_> {
    fn drop(&mut self) {
        if let Some(before) = self.before.take() {
            restore_checkpoint(self.vm, before);
        }
    }
}

fn restore_checkpoint(vm: &mut Vm, checkpoint: VmCheckpoint) {
    vm.status = checkpoint.status;
    vm.metrics = checkpoint.metrics;
    vm.registers = checkpoint.registers;
    vm.memory = checkpoint.memory;
    vm.input = checkpoint.input;
    vm.output = checkpoint.output;
    vm.runtime_program = checkpoint.runtime_program;
    vm.threads = checkpoint.threads;
    vm.terminal_errors = checkpoint.terminal_errors;
    vm.terminal_fault = checkpoint.terminal_fault;
}

#[derive(Clone, Copy)]
struct ExecutionWorkBudget {
    maximum: Option<u64>,
    consumed: u64,
    custom_internal_dispatches: u64,
}

impl ExecutionWorkBudget {
    const fn unlimited() -> Self {
        Self {
            maximum: None,
            consumed: 0,
            custom_internal_dispatches: 0,
        }
    }

    const fn limited(maximum: NonZeroU64) -> Self {
        Self {
            maximum: Some(maximum.get()),
            consumed: 0,
            custom_internal_dispatches: 0,
        }
    }

    const fn is_limited(&self) -> bool {
        self.maximum.is_some()
    }

    fn consume(&mut self, scope: ExecutionScope) -> Result<bool, MetricCounterOverflow> {
        if self.maximum.is_some_and(|maximum| self.consumed >= maximum) {
            return Ok(false);
        }
        let custom_internal_dispatches = match scope {
            ExecutionScope::Outer => None,
            ExecutionScope::Custom { .. } => Some(
                self.custom_internal_dispatches
                    .checked_add(1)
                    .ok_or(MetricCounterOverflow::CustomInternalDispatches)?,
            ),
        };
        self.consumed = self.consumed.saturating_add(1);
        if let Some(count) = custom_internal_dispatches {
            self.custom_internal_dispatches = count;
        }
        Ok(true)
    }

    fn exceeded(&self) -> Option<WorkLimitExceeded> {
        self.maximum
            .filter(|maximum| self.consumed >= *maximum)
            .map(|maximum_work_units| WorkLimitExceeded {
                maximum_work_units,
                custom_internal_dispatches: self.custom_internal_dispatches,
            })
    }

    fn exhaust_custom_cycle(&mut self) -> Result<(), MetricCounterOverflow> {
        let Some(maximum) = self.maximum else {
            return Ok(());
        };
        if self.consumed >= maximum {
            return Ok(());
        }
        let remaining = maximum - self.consumed;
        self.custom_internal_dispatches = self
            .custom_internal_dispatches
            .checked_add(remaining)
            .ok_or(MetricCounterOverflow::CustomInternalDispatches)?;
        self.consumed = maximum;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CellVisitRun {
    scope: ExecutionScope,
    thread_id: u64,
    cell: StaticCellId,
    count: u64,
}

#[derive(Default)]
struct CellVisitLog {
    runs: Vec<CellVisitRun>,
}

impl CellVisitLog {
    fn push(&mut self, scope: ExecutionScope, thread_id: u64, cell: StaticCellId) {
        self.push_run(CellVisitRun {
            scope,
            thread_id,
            cell,
            count: 1,
        });
    }

    fn append(&mut self, other: Self) {
        for run in other.runs {
            self.push_run(run);
        }
    }

    fn push_run(&mut self, run: CellVisitRun) {
        if let Some(last) = self.runs.last_mut() {
            let next_step = match last.scope {
                ExecutionScope::Outer => None,
                ExecutionScope::Custom {
                    caller_thread_id,
                    custom_id,
                    internal_step,
                } => internal_step
                    .checked_add(last.count)
                    .map(|step| (caller_thread_id, custom_id, step)),
            };
            let same_scope_sequence = match (next_step, run.scope) {
                (
                    Some((last_caller, last_custom, expected_step)),
                    ExecutionScope::Custom {
                        caller_thread_id,
                        custom_id,
                        internal_step,
                    },
                ) => {
                    last_caller == caller_thread_id
                        && last_custom == custom_id
                        && expected_step == internal_step
                }
                _ => false,
            };
            if same_scope_sequence && last.thread_id == run.thread_id && last.cell == run.cell {
                if let Some(count) = last.count.checked_add(run.count) {
                    last.count = count;
                    return;
                }
            }
        }
        self.runs.push(run);
    }

    fn into_events(self) -> Vec<VmEvent> {
        let mut events = Vec::new();
        for run in self.runs {
            for offset in 0..run.count {
                let scope = match run.scope {
                    ExecutionScope::Outer => ExecutionScope::Outer,
                    ExecutionScope::Custom {
                        caller_thread_id,
                        custom_id,
                        internal_step,
                    } => ExecutionScope::Custom {
                        caller_thread_id,
                        custom_id,
                        internal_step: internal_step
                            .checked_add(offset)
                            .expect("recorded Custom event steps fit u64"),
                    },
                };
                events.push(VmEvent::CellReached {
                    scope,
                    thread_id: run.thread_id,
                    cell: run.cell,
                });
            }
        }
        events
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct StackUsage {
    data: u64,
    instruction: u64,
    call: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CustomStackSample {
    usage: StackUsage,
    caller_depths: BTreeMap<u64, u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CustomStackSampleRun {
    end_step: u64,
    sample: CustomStackSample,
}

#[derive(Default)]
struct CustomStackSamples {
    /// Run-length encoded, one-based Custom internal-step samples. Gaps mean
    /// the all-zero sample, which cannot raise the outer stack high-water mark.
    runs: BTreeMap<u64, CustomStackSampleRun>,
}

impl CustomStackSamples {
    fn add_usage(&mut self, internal_step: u64, usage: StackUsage) -> Result<(), VmFault> {
        if usage == StackUsage::default() {
            return Ok(());
        }
        let mut sample = self.sample_at(internal_step);
        sample.usage = add_stack_usage(sample.usage, usage)?;
        self.set_point(internal_step, sample);
        Ok(())
    }

    fn add_caller_depth(&mut self, internal_step: u64, caller_thread_id: u64, depth: u64) {
        let mut sample = self.sample_at(internal_step);
        if sample
            .caller_depths
            .get(&caller_thread_id)
            .is_some_and(|current| *current >= depth)
        {
            return;
        }
        sample.caller_depths.insert(caller_thread_id, depth);
        self.set_point(internal_step, sample);
    }

    fn sample_at(&self, internal_step: u64) -> CustomStackSample {
        self.runs
            .range(..=internal_step)
            .next_back()
            .filter(|(_, run)| internal_step <= run.end_step)
            .map_or_else(CustomStackSample::default, |(_, run)| run.sample.clone())
    }

    fn set_point(&mut self, internal_step: u64, sample: CustomStackSample) {
        if let Some((&start, run)) = self.runs.range(..=internal_step).next_back() {
            if internal_step <= run.end_step {
                let old = run.clone();
                self.runs.remove(&start);
                if start < internal_step {
                    self.runs.insert(
                        start,
                        CustomStackSampleRun {
                            end_step: internal_step - 1,
                            sample: old.sample.clone(),
                        },
                    );
                }
                if internal_step < old.end_step {
                    self.runs.insert(
                        internal_step + 1,
                        CustomStackSampleRun {
                            end_step: old.end_step,
                            sample: old.sample,
                        },
                    );
                }
            }
        }
        self.runs.insert(
            internal_step,
            CustomStackSampleRun {
                end_step: internal_step,
                sample,
            },
        );
        self.coalesce_at(internal_step);
    }

    fn coalesce_at(&mut self, mut start: u64) {
        if let Some((&previous_start, previous)) = self.runs.range(..start).next_back() {
            if previous.end_step.checked_add(1) == Some(start) {
                let current = self
                    .runs
                    .get(&start)
                    .expect("the updated Custom stack sample exists");
                if previous.sample == current.sample {
                    let merged = CustomStackSampleRun {
                        end_step: current.end_step,
                        sample: current.sample.clone(),
                    };
                    self.runs.remove(&previous_start);
                    self.runs.remove(&start);
                    self.runs.insert(previous_start, merged);
                    start = previous_start;
                }
            }
        }
        let Some(current) = self.runs.get(&start).cloned() else {
            return;
        };
        let next_start = current.end_step.checked_add(1);
        let Some(next_start) = next_start else {
            return;
        };
        if let Some(next) = self.runs.get(&next_start) {
            if current.sample == next.sample {
                let merged = CustomStackSampleRun {
                    end_step: next.end_step,
                    sample: current.sample,
                };
                self.runs.remove(&start);
                self.runs.remove(&next_start);
                self.runs.insert(start, merged);
            }
        }
    }
}

#[derive(Default)]
struct StepDraft {
    effects: SharedEffectBatch,
    events: CellVisitLog,
    committed_events: Vec<VmEvent>,
    errors: Vec<RuntimeError>,
    halt_requested: bool,
    work_limit_exceeded: bool,
    fault: Option<VmFault>,
    custom_stack_samples: CustomStackSamples,
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
    /// Evaluates and stages one outer transition for an explicit host
    /// decision. Inspect its footprint report, then commit or abort exactly once.
    pub fn prepare_step(&mut self) -> PreparedStep<'_> {
        let mut work_budget = ExecutionWorkBudget::unlimited();
        self.prepare_with_budget(&mut work_budget)
            .expect("an unlimited execution budget cannot be exhausted")
    }

    /// Evaluates and stages one outer transition under a deterministic host
    /// work ceiling. An interrupted attempt creates no pending transition.
    pub fn prepare_step_with_work_limit(
        &mut self,
        maximum_work_units: NonZeroU64,
    ) -> Result<PreparedStep<'_>, WorkLimitExceeded> {
        let mut work_budget = ExecutionWorkBudget::limited(maximum_work_units);
        self.prepare_with_budget(&mut work_budget)
    }

    /// Evaluates and atomically commits one outer transition.
    pub fn step(&mut self) -> StepResult {
        let mut work_budget = ExecutionWorkBudget::unlimited();
        self.step_with_budget(&mut work_budget)
            .expect("an unlimited execution budget cannot be exhausted")
    }

    /// Evaluates one outer transition with a deterministic host work ceiling.
    ///
    /// One work unit is one outer or Custom thread dispatch. Exceeding the
    /// ceiling rolls back the entire attempted transition and leaves VM state
    /// and raw observations unchanged.
    pub fn step_with_work_limit(
        &mut self,
        maximum_work_units: NonZeroU64,
    ) -> Result<StepResult, WorkLimitExceeded> {
        let mut work_budget = ExecutionWorkBudget::limited(maximum_work_units);
        self.step_with_budget(&mut work_budget)
    }

    /// Attempts the same atomic transition and reports actual thread dispatches,
    /// including work discarded by rollback. Host accounting is independent
    /// of VM observations. Runtime-error attempts retain their attempted
    /// observations; work-interrupted attempts retain none.
    pub fn step_with_work_accounting(
        &mut self,
        maximum_work_units: NonZeroU64,
    ) -> (Result<StepResult, WorkLimitExceeded>, u64) {
        let mut work_budget = ExecutionWorkBudget::limited(maximum_work_units);
        let result = self.step_with_budget(&mut work_budget);
        (result, work_budget.consumed)
    }

    fn step_with_budget(
        &mut self,
        work_budget: &mut ExecutionWorkBudget,
    ) -> Result<StepResult, WorkLimitExceeded> {
        self.step_with_budget_and_footprint(work_budget)
            .map(|(result, _)| result)
    }

    fn step_with_budget_and_footprint(
        &mut self,
        work_budget: &mut ExecutionWorkBudget,
    ) -> Result<(StepResult, StepFootprint), WorkLimitExceeded> {
        if self.status != VmStatus::Running {
            return Ok((
                StepResult {
                    attempted_transition_id: self.metrics.committed_steps(),
                    committed_steps: self.metrics.committed_steps(),
                    metrics: self.metrics.summary(),
                    events: Vec::new(),
                    status: self.status,
                    newly_emitted_output: Vec::new(),
                    errors: self.terminal_errors.clone(),
                    fault: self.terminal_fault,
                },
                StepFootprint::default(),
            ));
        }

        let Some(attempted_transition_id) = self.metrics.committed_steps().checked_add(1) else {
            self.status = VmStatus::Error;
            self.terminal_fault = Some(VmFault::CommittedStepOverflow);
            return Ok((
                StepResult {
                    attempted_transition_id: u64::MAX,
                    committed_steps: self.metrics.committed_steps(),
                    metrics: self.metrics.summary(),
                    events: Vec::new(),
                    status: self.status,
                    newly_emitted_output: Vec::new(),
                    errors: Vec::new(),
                    fault: self.terminal_fault,
                },
                StepFootprint::default(),
            ));
        };

        let mut attempted_metrics = self.metrics.begin_step_attempt();
        let initial_stack_fault = observe_stacks(&mut attempted_metrics, &self.threads);

        // Every thread reads these step-start snapshots. Effects are committed
        // only after all local failures and shared conflicts have been found.
        let registers = self.registers;
        let input: VecDeque<Value> = self.input.clone();
        let memory = self.memory.clone();
        let runtime_start = self.runtime_program.clone();
        let verified_program = self.verified_program.clone();
        let mut staged_threads = self.threads.clone();
        let mut draft = StepDraft {
            fault: initial_stack_fault,
            ..StepDraft::default()
        };

        for thread in &mut staged_threads {
            full::evaluate_thread(
                thread,
                attempted_transition_id,
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
            if draft.work_limit_exceeded || draft.fault.is_some() {
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
            attempted_transition_id,
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
                    for sample_run in draft.custom_stack_samples.runs.values() {
                        let mut aligned = outer_usage;
                        for (caller_id, sampled_depth) in &sample_run.sample.caller_depths {
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
                            }
                        }
                        if draft.fault.is_some() {
                            break;
                        }
                        let aligned = match add_stack_usage(aligned, sample_run.sample.usage) {
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

        let footprint = attempted_metrics.step_footprint();

        if draft.fault.is_some() {
            self.metrics.merge_step_attempt(attempted_metrics);
            self.status = VmStatus::Error;
            self.terminal_fault = draft.fault;
            return Ok((
                StepResult {
                    attempted_transition_id,
                    committed_steps: self.metrics.committed_steps(),
                    metrics: self.metrics.summary(),
                    events: Vec::new(),
                    status: self.status,
                    newly_emitted_output: Vec::new(),
                    errors: Vec::new(),
                    fault: self.terminal_fault,
                },
                footprint,
            ));
        }

        if !draft.errors.is_empty() {
            self.metrics.merge_step_attempt(attempted_metrics);
            self.status = VmStatus::Error;
            self.terminal_errors = draft.errors.clone();
            return Ok((
                StepResult {
                    attempted_transition_id,
                    committed_steps: self.metrics.committed_steps(),
                    metrics: self.metrics.summary(),
                    events: Vec::new(),
                    status: self.status,
                    newly_emitted_output: Vec::new(),
                    errors: draft.errors,
                    fault: None,
                },
                footprint,
            ));
        }

        if self.metrics.committed_steps() == u64::MAX {
            self.status = VmStatus::Error;
            self.terminal_fault = Some(VmFault::CommittedStepOverflow);
            return Ok((
                StepResult {
                    attempted_transition_id,
                    committed_steps: self.metrics.committed_steps(),
                    metrics: self.metrics.summary(),
                    events: Vec::new(),
                    status: self.status,
                    newly_emitted_output: Vec::new(),
                    errors: Vec::new(),
                    fault: self.terminal_fault,
                },
                footprint,
            ));
        }

        let output_start = self.output.len();
        commit_effects(
            &mut self.registers,
            &mut staged_threads,
            &mut self.memory,
            &mut self.input,
            &mut self.output,
            &mut self.runtime_program,
            &draft.effects,
        );
        let mut events = std::mem::take(&mut draft.events).into_events();
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
        let committed = attempted_metrics.commit_step().is_ok();
        self.metrics.merge_step_attempt(attempted_metrics);
        if !committed {
            // The explicit preflight above makes this unreachable unless the
            // metrics implementation changes independently.
            self.status = VmStatus::Error;
            self.terminal_fault = Some(VmFault::MetricCounterOverflow(
                MetricCounterOverflow::CommittedSteps,
            ));
        } else if draft.halt_requested {
            self.status = VmStatus::Halted;
        }

        Ok((
            StepResult {
                attempted_transition_id,
                committed_steps: self.metrics.committed_steps(),
                metrics: self.metrics.summary(),
                events: if committed { events } else { Vec::new() },
                status: self.status,
                newly_emitted_output: self.output[output_start..].to_vec(),
                errors: Vec::new(),
                fault: self.terminal_fault,
            },
            footprint,
        ))
    }

    fn prepare_with_budget(
        &mut self,
        work_budget: &mut ExecutionWorkBudget,
    ) -> Result<PreparedStep<'_>, WorkLimitExceeded> {
        let before = VmCheckpoint::capture(self);
        let (result, footprint) = self.step_with_budget_and_footprint(work_budget)?;
        let report = StepReport {
            footprint,
            work_units: work_budget.consumed,
            custom_internal_dispatches: work_budget.custom_internal_dispatches,
            candidate_status: result.status,
            errors: result.errors.clone(),
            fault: result.fault,
        };
        Ok(PreparedStep {
            vm: self,
            before: Some(before),
            result: Some(result),
            report,
        })
    }

    /// Runs at most `max_steps` atomic transitions using [`Vm::step`].
    pub fn run(&mut self, max_steps: u64) -> RunOutcome {
        if max_steps == 0 {
            return RunOutcome::InvalidStepLimit;
        }
        for _ in 0..max_steps {
            match self.step().status {
                VmStatus::Running => {}
                VmStatus::Halted => return RunOutcome::Halted,
                VmStatus::Error => return RunOutcome::Error,
            }
        }
        match self.status {
            VmStatus::Halted => RunOutcome::Halted,
            VmStatus::Error => RunOutcome::Error,
            VmStatus::Running => RunOutcome::StepLimitReached,
        }
    }

    /// Runs a bounded slice under one shared work ceiling across all attempted
    /// transitions. Earlier complete steps remain committed if a later step
    /// exceeds the budget; the interrupted step is rolled back atomically.
    pub fn run_with_work_limit(
        &mut self,
        max_steps: u64,
        maximum_work_units: NonZeroU64,
    ) -> Result<RunOutcome, WorkLimitExceeded> {
        if max_steps == 0 {
            return Ok(RunOutcome::InvalidStepLimit);
        }
        let mut work_budget = ExecutionWorkBudget::limited(maximum_work_units);
        for _ in 0..max_steps {
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
            VmStatus::Running => RunOutcome::StepLimitReached,
        })
    }

    /// Runs bounded transitions and retains committed events/output when a work
    /// ceiling yields partway through the requested run.
    pub fn run_with_work_limit_detailed(
        &mut self,
        max_steps: u64,
        maximum_work_units: NonZeroU64,
    ) -> RunResult {
        if max_steps == 0 {
            return RunResult {
                outcome: RunOutcome::InvalidStepLimit,
                events: Vec::new(),
                newly_emitted_output: Vec::new(),
                work_limit_exceeded: None,
            };
        }

        let mut work_budget = ExecutionWorkBudget::limited(maximum_work_units);
        let mut events = Vec::new();
        let mut newly_emitted_output = Vec::new();
        for _ in 0..max_steps {
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
            outcome: RunOutcome::StepLimitReached,
            events,
            newly_emitted_output,
            work_limit_exceeded: None,
        }
    }
}

fn record_cell_visit(
    metrics: &mut crate::RuntimeMetrics,
    draft: &mut StepDraft,
    thread: &ThreadState,
    cell: StaticCellId,
    scope: ExecutionScope,
) {
    metrics.record_cell(cell);
    draft.events.push(scope, thread.id, cell);
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
                    internal_step,
                    ..
                },
            thread_id,
            ..
        } => (*caller_thread_id, 1, *internal_step, *thread_id),
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
            internal_step,
        } => (0, internal_step, caller_thread_id, custom_id.get()),
        ExecutionScope::Outer => (1, 0, 0, 0),
    }
}

fn record_operation(
    metrics: &mut crate::RuntimeMetrics,
    kind: InstructionKind,
    draft: &mut StepDraft,
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
    threads: &mut [ThreadState],
    memory: &mut crate::Memory,
    input: &mut VecDeque<Value>,
    output: &mut Vec<Value>,
    runtime: &mut ScopedProgram,
    effects: &SharedEffectBatch,
) {
    for write in &effects.register_writes {
        commit_register(
            registers,
            threads,
            effects,
            write.thread_id,
            write.register,
            write.value,
        );
    }
    if let Some(read) = effects.input_reads.iter().find(|read| read.value.is_some()) {
        if let Some(value) = read.value {
            commit_register(
                registers,
                threads,
                effects,
                read.thread_id,
                read.register,
                value,
            );
        }
    }
    if let Some(read) = effects
        .caller_stack_reads
        .iter()
        .find(|read| read.value.is_some())
    {
        if let Some(value) = read.value {
            commit_register(
                registers,
                threads,
                effects,
                read.thread_id,
                read.register,
                value,
            );
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

fn commit_register(
    registers: &mut [Value; 10],
    threads: &mut [ThreadState],
    effects: &SharedEffectBatch,
    thread_id: u64,
    register: u8,
    value: Value,
) {
    if effects.private_register_threads.contains(&thread_id) {
        if let Some(bank) = threads
            .iter_mut()
            .find(|thread| thread.id == thread_id)
            .and_then(|thread| thread.private_registers.as_mut())
        {
            bank[usize::from(register)] = value;
        }
    } else {
        registers[usize::from(register)] = value;
    }
}

#[cfg(test)]
mod stack_high_water_formula_tests {
    use super::{
        add_stack_usage, replace_caller_stack_sample, stack_usage, stack_usage_with_growth,
    };
    use crate::thread::{CallFrame, ThreadState};
    use crate::{Coordinate, RuntimeMetrics, StackPeaks};
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
                saved_registers: None,
                saved_register_pointer: 0,
                saved_status_flag: 0,
            })
            .collect();
        thread
    }

    #[test]
    fn aligned_custom_stack_footprint_uses_one_caller_sample() {
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
            .expect("outer stack totals fit the candidate counters");
        assert_eq!((outer.data, outer.instruction, outer.call), (4, 4, 2));

        // At an aligned Custom boundary, replace the caller's single outer
        // sample with its internal-step depth, then add every Custom thread.
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

        let mut candidate = RuntimeMetrics::default().begin_step_attempt();
        candidate.observe_stack_usage(attempted.data, attempted.instruction, attempted.call);
        assert_eq!(
            candidate.step_footprint().stack_peaks,
            StackPeaks {
                data: 8,
                instruction: 5,
                call: 3,
            }
        );
    }
}

#[cfg(test)]
mod work_budget_tests {
    use super::ExecutionWorkBudget;
    use crate::{ExecutionScope, MetricCounterOverflow};
    use codegrid_model::Slot;

    #[test]
    fn custom_dispatch_counter_reports_overflow_without_wrapping() {
        let mut budget = ExecutionWorkBudget::unlimited();
        budget.custom_internal_dispatches = u64::MAX;

        let result = budget.consume(ExecutionScope::Custom {
            caller_thread_id: 0,
            custom_id: Slot::new(0).expect("zero is a valid Custom ID"),
            internal_step: 1,
        });

        assert_eq!(result, Err(MetricCounterOverflow::CustomInternalDispatches));
        assert_eq!(budget.custom_internal_dispatches, u64::MAX);
        assert_eq!(budget.consumed, 0);
    }
}
