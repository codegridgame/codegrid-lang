use crate::{
    metrics::{aggregate, constraints_exceeded, dynamic_metrics, rating, static_metrics, Metrics},
    result::*,
    schema::ValidatedLevel,
    validate::{validate_primary, validate_program},
};
use codegrid_ir::{ScopedProgram, VerifiedProgram};
use codegrid_model::BoundaryMode;
use codegrid_vm::{mix64, Vm, VmConfig, VmEvent, VmStatus};
use std::num::NonZeroU64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvaluationMode {
    Debug,
    Official,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionSafetyProfile {
    pub id: String,
    pub version: u32,
    pub max_output_bytes: NonZeroU64,
    pub max_state_units: NonZeroU64,
    /// Conservative encoded feedback/result reservation supplied by the host.
    pub max_feedback_bytes: NonZeroU64,
    pub per_test_ticks: NonZeroU64,
    pub cumulative_work: NonZeroU64,
    pub per_call_work: NonZeroU64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluationConfig {
    pub boundary_mode: BoundaryMode,
    pub shuffle_seed: u64,
    pub custom_execution_limit: NonZeroU64,
    pub safety: ExecutionSafetyProfile,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvaluationProgress {
    Pending,
    Complete(EvaluationResult),
}
/// The selected order, hidden test position, VM state and traces stay private.
pub struct EvaluationSession {
    level: ValidatedLevel,
    program: VerifiedProgram,
    config: EvaluationConfig,
    mode: EvaluationMode,
    order: Vec<usize>,
    position: usize,
    vm: Option<Vm>,
    matched: usize,
    work: u64,
    metrics: Metrics,
    pending_metrics: Option<Metrics>,
    visible: Vec<VisibleTestResult>,
    hidden_failure: Option<TestOutcome>,
    failure: Option<EvaluationStatus>,
    result: Option<EvaluationResult>,
    feedback_bytes: u64,
    generated_diagnostic: Option<GeneratedRejectionDiagnostic>,
}
/// Trusted diagnostic retained inside the opaque session, never projected.
#[allow(dead_code)]
struct GeneratedRejectionDiagnostic {
    source_index: usize,
    scope: codegrid_vm::ExecutionScope,
    cell: codegrid_vm::StaticCellId,
    primary: Option<codegrid_model::PrimaryInstruction>,
}
pub fn result_base_bound(level_id: &str, profile_id: &str) -> Option<u64> {
    16384u64.checked_add(
        (level_id.len() as u64)
            .checked_add(profile_id.len() as u64)?
            .checked_mul(6)?,
    )
}
pub fn feedback_bound(test: &VisibleTestResult) -> Option<u64> {
    feedback_parts_bound(
        test.input.len(),
        test.expected_output.len(),
        test.actual_output.len(),
        &test.outcome,
    )
}
fn feedback_parts_bound(
    input: usize,
    expected: usize,
    actual: usize,
    outcome: &TestOutcome,
) -> Option<u64> {
    let codes = match outcome {
        TestOutcome::RuntimeError(codes) => codes
            .iter()
            .try_fold(0u64, |n, c| n.checked_add(c.len() as u64)?.checked_add(4))?,
        _ => 0,
    };
    256u64.checked_add(
        (input as u64)
            .checked_add(expected as u64)?
            .checked_add(actual as u64)?
            .checked_add(codes)?
            .checked_mul(6)?,
    )
}
pub fn result_representation_bound(result: &EvaluationResult) -> Option<u64> {
    result.visible_tests.iter().try_fold(
        result_base_bound(&result.level_id, &result.config.safety.id)?,
        |n, t| n.checked_add(feedback_bound(t)?),
    )
}
pub fn shuffled_indices(mut indices: Vec<usize>, seed: u64) -> Vec<usize> {
    let mut state = seed;
    for i in (1..indices.len()).rev() {
        let bound = (i + 1) as u64;
        let threshold = bound.wrapping_neg() % bound;
        let draw = loop {
            state = mix64(state);
            if state >= threshold {
                break state;
            }
        };
        indices.swap(i, (draw % bound) as usize);
    }
    indices
}
pub fn start_evaluation(
    level: ValidatedLevel,
    program: VerifiedProgram,
    mode: EvaluationMode,
    config: EvaluationConfig,
) -> EvaluationSession {
    let order = shuffled_indices(
        level
            .tests()
            .iter()
            .enumerate()
            .filter(|(_, t)| mode == EvaluationMode::Official || t.visible)
            .map(|(i, _)| i)
            .collect(),
        config.shuffle_seed,
    );
    let metrics = static_metrics(&program);
    let mut session = EvaluationSession {
        level,
        program,
        config,
        mode,
        order,
        position: 0,
        vm: None,
        matched: 0,
        work: 0,
        metrics,
        pending_metrics: None,
        visible: vec![],
        hidden_failure: None,
        failure: None,
        result: None,
        feedback_bytes: 0,
        generated_diagnostic: None,
    };
    if let Err(reason) = validate_program(&session.level, &session.program) {
        session.finish(EvaluationStatus::ProgramRejected(reason));
    } else if constraints_exceeded(&session.metrics, session.level.constraints()) {
        session.failure = Some(EvaluationStatus::ConstraintExceeded);
        if mode == EvaluationMode::Official {
            session.finish(EvaluationStatus::ConstraintExceeded);
        }
    }
    session
}
pub fn evaluate(
    level: ValidatedLevel,
    program: VerifiedProgram,
    mode: EvaluationMode,
    config: EvaluationConfig,
) -> EvaluationResult {
    let budget = config.safety.per_call_work;
    let mut session = start_evaluation(level, program, mode, config);
    loop {
        if let EvaluationProgress::Complete(result) = session.advance(budget) {
            return result;
        }
    }
}
impl EvaluationSession {
    pub fn result(&self) -> Option<&EvaluationResult> {
        self.result.as_ref()
    }
    pub fn cancel(&mut self) {
        if self.result.is_none() {
            self.finish(EvaluationStatus::Cancelled);
        }
    }
    fn finish(&mut self, status: EvaluationStatus) {
        let passed = status == EvaluationStatus::Passed;
        self.result = Some(EvaluationResult {
            level_id: self.level.level_id().into(),
            level_version: self.level.level_version(),
            evaluator_contract: "exactio-v1/2026-09-30".into(),
            mode: self.mode,
            config: self.config.clone(),
            vm_seed: mix64(self.config.shuffle_seed ^ 0x43474C564D303031),
            status,
            hidden_failure: self.hidden_failure.clone(),
            visible_tests: std::mem::take(&mut self.visible),
            partial_metrics: self
                .pending_metrics
                .clone()
                .unwrap_or_else(|| self.metrics.clone()),
            final_metrics: passed.then(|| self.metrics.clone()),
            rating: if passed {
                rating(&self.metrics, self.level.scoring())
            } else {
                None
            },
            constraints: self
                .level
                .constraints()
                .iter()
                .map(|(name, limit)| {
                    let value = self
                        .pending_metrics
                        .as_ref()
                        .unwrap_or(&self.metrics)
                        .get(crate::metrics::constraint_metric(name))
                        .copied()
                        .unwrap_or(0);
                    ConstraintResult {
                        name: name.clone(),
                        limit: *limit,
                        value,
                        passed: value <= *limit,
                    }
                })
                .collect(),
            scoring: if passed {
                self.level
                    .scoring()
                    .iter()
                    .map(|(name, target)| ScoringResult {
                        name: name.clone(),
                        target: *target,
                        value: self.metrics.get(name).copied().unwrap_or(0),
                        rating: rating(
                            &self.metrics,
                            &std::collections::BTreeMap::from([(name.clone(), *target)]),
                        ),
                    })
                    .collect()
            } else {
                vec![]
            },
        });
        self.vm = None;
    }
    pub fn advance(&mut self, work_budget: NonZeroU64) -> EvaluationProgress {
        let mut remaining = work_budget
            .get()
            .min(self.config.safety.per_call_work.get());
        while self.result.is_none() {
            if result_base_bound(self.level.level_id(), &self.config.safety.id)
                .and_then(|n| n.checked_add(self.feedback_bytes))
                .is_none_or(|n| n > self.config.safety.max_feedback_bytes.get())
            {
                self.finish(EvaluationStatus::ResourceLimitExceeded);
                break;
            }
            if self.position == self.order.len() {
                self.finish(self.failure.clone().unwrap_or(EvaluationStatus::Passed));
                break;
            }
            if self.work == self.config.safety.cumulative_work.get() {
                self.finish(EvaluationStatus::ResourceLimitExceeded);
                break;
            }
            if remaining == 0 {
                break;
            }
            let index = self.order[self.position];
            let test = &self.level.tests()[index];
            if self.vm.is_none() {
                let config = VmConfig::new(
                    self.config.boundary_mode,
                    mix64(self.config.shuffle_seed ^ 0x43474C564D303031),
                    self.config.custom_execution_limit,
                );
                match Vm::new(self.program.clone(), test.input.iter().copied(), config) {
                    Ok(vm) => self.vm = Some(vm),
                    Err(_) => {
                        self.finish(EvaluationStatus::Fault(FaultReason::VmInitialization));
                        break;
                    }
                }
                self.matched = 0;
            }
            let vm = self.vm.as_mut().unwrap();
            if retained_state_units(vm, &self.program)
                .is_none_or(|units| units > self.config.safety.max_state_units.get())
            {
                self.finish(EvaluationStatus::ResourceLimitExceeded);
                break;
            }
            if vm.committed_ticks() >= self.config.safety.per_test_ticks.get() {
                self.finish(EvaluationStatus::ResourceLimitExceeded);
                break;
            }
            let limit = remaining.min(self.config.safety.cumulative_work.get() - self.work);
            let (step, used) = vm.step_with_work_accounting(NonZeroU64::new(limit).unwrap());
            remaining -= used;
            self.work += used;
            let step = match step {
                Ok(step) => step,
                Err(_) => {
                    if self.work == self.config.safety.cumulative_work.get()
                        || limit == self.config.safety.per_call_work.get()
                    {
                        self.finish(EvaluationStatus::ResourceLimitExceeded);
                    }
                    break;
                }
            };
            let state_exceeded = retained_state_units(vm, &self.program)
                .is_none_or(|units| units > self.config.safety.max_state_units.get());
            let output_exceeded = vm.snapshot_view().output().len() as u64
                > self.config.safety.max_output_bytes.get();
            if step.fault.is_some() {
                self.finish(EvaluationStatus::Fault(FaultReason::VmFault));
                break;
            }
            let visible = test.visible;
            let mut current = self.metrics.clone();
            if visible {
                match aggregate(&self.metrics, &dynamic_metrics(&step.metrics)) {
                    Some(value) => current = value,
                    None => {
                        self.finish(EvaluationStatus::Fault(FaultReason::NumericOverflow));
                        break;
                    }
                }
            }
            self.pending_metrics = Some(current.clone());
            if step.status != VmStatus::Error {
                let mut rejection = None;
                for event in &step.events {
                    if let VmEvent::CodeChanged {
                        new, scope, cell, ..
                    } = event
                    {
                        if let Err(reason) = validate_primary(self.level.rules(), *new, true) {
                            self.generated_diagnostic = Some(GeneratedRejectionDiagnostic {
                                source_index: index,
                                scope: *scope,
                                cell: *cell,
                                primary: *new,
                            });
                            rejection = Some(reason);
                            break;
                        }
                    }
                }
                if rejection.is_none() {
                    rejection =
                        check_scope(self.level.rules(), vm.snapshot_view().runtime_program());
                }
                if let Some(mut reason) = rejection {
                    if !visible {
                        reason.path.clear();
                    }
                    self.metrics = current;
                    self.finish(EvaluationStatus::ProgramRejected(reason));
                    break;
                }
            }
            let outcome = if step.status == VmStatus::Error {
                Some(TestOutcome::RuntimeError(
                    step.errors.iter().map(|e| e.code().into()).collect(),
                ))
            } else {
                let mut wrong = false;
                for byte in &step.newly_emitted_output {
                    if test.expected_output.get(self.matched) != Some(byte) {
                        wrong = true;
                        break;
                    }
                    self.matched += 1;
                }
                if wrong {
                    Some(TestOutcome::WrongOutput)
                } else if !test.expected_output.is_empty()
                    && self.matched == test.expected_output.len()
                {
                    Some(TestOutcome::Passed)
                } else if step.status == VmStatus::Halted {
                    Some(if test.expected_output.is_empty() {
                        TestOutcome::Passed
                    } else {
                        TestOutcome::IncompleteOutput
                    })
                } else {
                    None
                }
            };
            let constraint = constraints_exceeded(&current, self.level.constraints());
            if let Some(outcome) = outcome {
                if outcome != TestOutcome::Passed {
                    let status = if matches!(outcome, TestOutcome::RuntimeError(_)) {
                        EvaluationStatus::RuntimeError
                    } else {
                        EvaluationStatus::TestFailed
                    };
                    if self.failure.is_none() {
                        self.failure = Some(status);
                    }
                }
                if !visible && outcome != TestOutcome::Passed {
                    self.hidden_failure =
                        Some(if matches!(outcome, TestOutcome::RuntimeError(_)) {
                            TestOutcome::RuntimeError(vec![])
                        } else {
                            outcome.clone()
                        });
                }
                if visible {
                    let bound = feedback_parts_bound(
                        test.input.len(),
                        test.expected_output.len(),
                        vm.snapshot_view().output().len(),
                        &outcome,
                    )
                    .and_then(|n| n.checked_add(self.feedback_bytes));
                    if bound
                        .and_then(|n| {
                            result_base_bound(self.level.level_id(), &self.config.safety.id)?
                                .checked_add(n)
                        })
                        .is_none_or(|n| n > self.config.safety.max_feedback_bytes.get())
                    {
                        self.metrics = current;
                        self.finish(EvaluationStatus::ResourceLimitExceeded);
                        break;
                    }
                    let feedback = VisibleTestResult {
                        source_index: index,
                        input: test.input.clone(),
                        expected_output: test.expected_output.clone(),
                        actual_output: vm.snapshot_view().output().to_vec(),
                        outcome,
                    };
                    self.feedback_bytes = bound.unwrap();
                    self.visible.push(feedback);
                }
                self.metrics = current.clone();
                self.pending_metrics = None;
                self.vm = None;
                self.position += 1;
            }
            if constraint && self.failure.is_none() {
                self.failure = Some(EvaluationStatus::ConstraintExceeded);
            }

            if self.mode == EvaluationMode::Official && self.failure.is_some() {
                self.metrics = current;
                self.finish(self.failure.clone().unwrap());
                break;
            }
            if state_exceeded || output_exceeded {
                self.finish(EvaluationStatus::ResourceLimitExceeded);
                break;
            }
        }
        self.result
            .clone()
            .map_or(EvaluationProgress::Pending, EvaluationProgress::Complete)
    }
}
pub(crate) fn check_scope(
    rules: &crate::schema::ProgramRules,
    scoped: &ScopedProgram,
) -> Option<crate::validate::ProgramRejection> {
    for board in std::iter::once(&scoped.main).chain(scoped.functions.values()) {
        for cell in &board.cells {
            if let Err(reason) = validate_primary(rules, cell.primary, true) {
                return Some(reason);
            }
        }
        for fold in board.folded_blocks.values() {
            for p in &fold.cells {
                if let Err(reason) = validate_primary(rules, *p, true) {
                    return Some(reason);
                }
            }
        }
    }
    None
}

/// Logical retained-state units: code cells in immutable and mutable programs,
/// registers, queued bytes, threads, stack/frame entries, metric identities,
/// memory cells, and bytes in every retained unbounded Page/address integer.
pub fn retained_state_units(vm: &Vm, program: &VerifiedProgram) -> Option<u64> {
    fn code(scoped: &ScopedProgram) -> u64 {
        std::iter::once(&scoped.main)
            .chain(scoped.functions.values())
            .map(|b| {
                b.cells.len() as u64
                    + b.folded_blocks
                        .values()
                        .map(|f| f.cells.len() as u64)
                        .sum::<u64>()
            })
            .sum()
    }
    let view = vm.snapshot_view();
    let mut total = 10u64.checked_add(code(&program.program().outer))?;
    for custom in program.program().customs.values() {
        total = total.checked_add(code(&custom.program))?;
    }
    total = total.checked_add(code(view.runtime_program()))?;
    total = total
        .checked_add(view.input().count() as u64)?
        .checked_add(view.output().len() as u64)?;
    total = total.checked_add(view.memory().allocated_cells() as u64)?;
    for address in view.memory().allocated_addresses() {
        total = total.checked_add(address.bits().div_ceil(8))?;
    }
    for thread in view.threads() {
        total = total
            .checked_add(1)?
            .checked_add(thread.page().bits().div_ceil(8))?
            .checked_add(thread.data_stack().len() as u64)?
            .checked_add(thread.instruction_stack().len() as u64)?;
        if thread.private_registers().is_some() {
            total = total.checked_add(10)?;
        }
        for frame in thread.call_frames() {
            total = total.checked_add(2)?;
            if frame.saved_registers.is_some() {
                total = total.checked_add(10)?;
            }
        }
    }
    let metrics = view.metrics();
    total = total
        .checked_add(metrics.used_cell_count() as u64)?
        .checked_add(metrics.used_memory_address_count() as u64)?
        .checked_add(metrics.instruction_variety().len() as u64)?;
    for location in metrics.used_memory_addresses() {
        total = total.checked_add(location.address.bits().div_ceil(8))?;
    }
    total.checked_add(view.errors().len() as u64)
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    use codegrid_ir::{Board, Cell, Program, ScopedProgram, IR_FORMAT_VERSION};
    use codegrid_model::{
        AttachmentInstruction as A, Direction, PrimaryInstruction as P, ShiftDirection,
    };
    use std::collections::BTreeMap;
    #[test]
    fn trusted_generated_diagnostic_retains_scope_cell_and_primary_without_public_projection() {
        let level = crate::schema::load_level_json(
            include_bytes!("../../../fixtures/levels/generated-outer.json"),
            100000,
        )
        .unwrap();
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: Board {
                    width: 6,
                    height: 1,
                    cells: vec![
                        Cell::entry(Direction::Right),
                        Cell::instruction(P::Read, None),
                        Cell::instruction(P::Decode, None),
                        Cell::instruction(P::Direction(Direction::Right), Some(A::WriteCode)),
                        Cell::instruction(P::Output, None),
                        Cell::instruction(P::Halt, None),
                    ],
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::new(),
        })
        .unwrap();
        let config = EvaluationConfig {
            boundary_mode: BoundaryMode::Exit,
            shuffle_seed: 0,
            custom_execution_limit: NonZeroU64::new(100).unwrap(),
            safety: ExecutionSafetyProfile {
                id: "test".into(),
                version: 1,
                max_output_bytes: NonZeroU64::new(100).unwrap(),
                max_state_units: NonZeroU64::new(10000).unwrap(),
                max_feedback_bytes: NonZeroU64::new(100000).unwrap(),
                per_test_ticks: NonZeroU64::new(100).unwrap(),
                cumulative_work: NonZeroU64::new(1000).unwrap(),
                per_call_work: NonZeroU64::new(100).unwrap(),
            },
        };
        let mut session = start_evaluation(level, program, EvaluationMode::Official, config);
        let result = match session.advance(NonZeroU64::new(100).unwrap()) {
            EvaluationProgress::Complete(r) => r,
            _ => panic!("fixture must terminate"),
        };
        let diagnostic = session.generated_diagnostic.as_ref().unwrap();
        assert_eq!(diagnostic.scope, codegrid_vm::ExecutionScope::Outer);
        assert_eq!(diagnostic.cell.position.x, 3);
        assert_eq!(diagnostic.source_index, 0);
        assert_eq!(diagnostic.primary, Some(P::Shift(ShiftDirection::Left)));
        assert!(
            matches!(result.status,EvaluationStatus::ProgramRejected(ref reason) if reason.path.is_empty())
        );
        assert!(result.visible_tests.is_empty());
    }
}
