//! One continuous VM per scene case, with atomic world/input publication.
use crate::evaluate::{check_scope, retained_state_units};
use crate::scene_feedback::{
    SceneActor, SceneEventDraft, SceneEventPayload, SceneFeedbackError, SceneFeedbackStore,
};
use crate::scene_protocol::{SceneMachine, SceneOutcome};
use crate::{
    aggregate, constraints_exceeded, dynamic_metrics, static_metrics, validate_primary,
    validate_program_rules, EvaluationConfig, EvaluationStatus, FaultReason, Metrics,
    SceneCaseData, ValidatedSceneLevel,
};
use codegrid_ir::VerifiedProgram;
use codegrid_vm::{mix64, Vm, VmConfig, VmEvent, VmStatus};
use std::{
    collections::{BTreeMap, VecDeque},
    num::NonZeroU64,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SceneLimits {
    pub max_input_queue_bytes: NonZeroU64,
    pub max_total_input_bytes_per_test: NonZeroU64,
    pub max_scene_state_units: NonZeroU64,
    pub max_scene_frames_per_test: NonZeroU64,
    pub max_scene_work_per_call: NonZeroU64,
    pub max_total_scene_work: NonZeroU64,
    pub max_scene_events_per_evaluation: NonZeroU64,
    pub max_scene_feedback_bytes: NonZeroU64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SceneCaseResult {
    pub status: EvaluationStatus,
    pub outcome: Option<SceneOutcome>,
    pub summary: BTreeMap<&'static str, u64>,
    pub metrics: Metrics,
    pub runtime_errors: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneCaseProgress {
    Pending,
    Complete(SceneCaseResult),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneCaseStartError {
    InvalidSourceIndex,
    NumericOverflow,
}

/// Trusted Rust session data; host projections must redact hidden case details.
pub struct SceneCaseSession {
    level: std::sync::Arc<ValidatedSceneLevel>,
    program: VerifiedProgram,
    index: usize,
    config: EvaluationConfig,
    limits: SceneLimits,
    definition_units: u64,
    init_state_bound: u64,
    init_work_bound: u64,
    init_input_bound: u64,
    machine: Option<SceneMachine>,
    vm: Option<Vm>,
    pending_output: VecDeque<u8>,
    pending_halt: bool,
    total_input: u64,
    vm_work: u64,
    scene_work: u64,
    metrics: Metrics,
    base_metrics: Metrics,
    include_metrics: bool,
    result: Option<SceneCaseResult>,
    feedback_started: bool,
    feedback_ended: bool,
    feedback_end_bound: u64,
    feedback_reserved_work: u64,
    feedback_result_base: u64,
}

/// Definition ownership is charged separately from independent mutable worlds.
pub fn scene_case_definition_units(case: &crate::SceneCase) -> Option<u64> {
    case.definition_units()
}
pub fn scene_definition_units(level: &ValidatedSceneLevel) -> Option<u64> {
    Some(level.definition_units())
}
// Bounds include the independent world and its initial observation, without
// constructing either. Robot start effects can only add one patrol entry and
// two trigger entries per actor; shared entries make these bounds conservative.
fn initialization_bounds(level: &ValidatedSceneLevel, index: usize) -> Option<(u64, u64, u64)> {
    let case = level.tests().get(index)?;
    if let crate::SceneConfig::Robot { map, actors } = level.config() {
        let actors = u64::from(*actors);
        let objects = map.cells.iter().try_fold(0u64, |n, c| {
            n.checked_add(u64::from(c.trigger.is_some()))?
                .checked_add(u64::from(c.door.is_some()))
        })?;
        let mut patrols = 0u64;
        let mut triggers = 0u64;
        for start in &map.starts {
            let cell = map.cells.get(usize::from(start.position))?;
            patrols = patrols.checked_add(u64::from(cell.patrol))?;
            triggers = triggers.checked_add(u64::from(cell.trigger.is_some()))?;
        }
        let input = actors.checked_mul(2)?;
        let state = (map.cells.len() as u64)
            .checked_add(objects)?
            .checked_add(actors)?
            .checked_add(patrols)?
            .checked_add(triggers.checked_mul(2)?)?
            .checked_add(input)?;
        let work = state
            .checked_add(map.cells.len() as u64)?
            .checked_add(actors)?
            .checked_add(patrols.checked_mul(2)?)?
            .checked_add(triggers.checked_mul(4)?)?
            .checked_add(input)?;
        return Some((state, work, input));
    }
    let units = level.case_definition_units(index)?;
    let input = match &case.data {
        SceneCaseData::Static { input, .. } => input.len() as u64,
        SceneCaseData::MechanicalArm { .. } => 2,
        SceneCaseData::Robot => return None,
    };
    Some((
        units.checked_add(input)?.checked_add(32)?,
        units.checked_mul(8)?.checked_add(512)?,
        input,
    ))
}
impl SceneCaseSession {
    pub fn new(
        level: ValidatedSceneLevel,
        program: VerifiedProgram,
        index: usize,
        config: EvaluationConfig,
        limits: SceneLimits,
    ) -> Result<Self, SceneCaseStartError> {
        Self::from_shared(std::sync::Arc::new(level), program, index, config, limits)
    }
    pub fn from_shared(
        level: std::sync::Arc<ValidatedSceneLevel>,
        program: VerifiedProgram,
        index: usize,
        config: EvaluationConfig,
        limits: SceneLimits,
    ) -> Result<Self, SceneCaseStartError> {
        let base_metrics = static_metrics(&program);
        Self::from_shared_with_metrics(level, program, index, config, limits, base_metrics)
    }
    pub fn from_shared_with_metrics(
        level: std::sync::Arc<ValidatedSceneLevel>,
        program: VerifiedProgram,
        index: usize,
        config: EvaluationConfig,
        limits: SceneLimits,
        base_metrics: Metrics,
    ) -> Result<Self, SceneCaseStartError> {
        if level.tests().get(index).is_none() {
            return Err(SceneCaseStartError::InvalidSourceIndex);
        }
        let definition_units =
            scene_definition_units(&level).ok_or(SceneCaseStartError::NumericOverflow)?;
        let (world_bound, init_work_bound, init_input_bound) =
            initialization_bounds(&level, index).ok_or(SceneCaseStartError::NumericOverflow)?;
        let init_state_bound = definition_units
            .checked_add(world_bound)
            .ok_or(SceneCaseStartError::NumericOverflow)?;
        let metrics = base_metrics.clone();
        let include_metrics = level.tests()[index].visible;
        let feedback_end_bound = (program
            .program()
            .outer
            .main
            .cells
            .iter()
            .filter(|c| c.entry.is_some())
            .count() as u64)
            .checked_mul(512)
            .and_then(|n| n.checked_add(4096))
            .ok_or(SceneCaseStartError::NumericOverflow)?;
        let mut session = Self {
            level,
            program,
            index,
            config,
            limits,
            definition_units,
            init_state_bound,
            init_work_bound,
            init_input_bound,
            machine: None,
            vm: None,
            pending_output: VecDeque::new(),
            pending_halt: false,
            total_input: 0,
            vm_work: 0,
            scene_work: 0,
            metrics,
            base_metrics,
            include_metrics,
            result: None,
            feedback_started: false,
            feedback_ended: false,
            feedback_end_bound,
            feedback_reserved_work: 0,
            feedback_result_base: 0,
        };
        if let Err(reason) = validate_program_rules(session.level.rules(), &session.program) {
            session.finish(EvaluationStatus::ProgramRejected(reason), Vec::new());
        } else if constraints_exceeded(&session.metrics, session.level.constraints()) {
            session.finish(EvaluationStatus::ConstraintExceeded, Vec::new());
        }
        Ok(session)
    }
    pub(crate) fn committed_ticks(&self) -> Option<u64> {
        self.vm.as_ref().map(Vm::committed_ticks)
    }
    pub(crate) fn current_metrics(&self) -> &Metrics {
        &self.metrics
    }
    pub(crate) fn retained_scene_units(&self) -> Option<u64> {
        self.definition_units
            .checked_add(
                self.machine
                    .as_ref()
                    .map_or(0, SceneMachine::retained_units),
            )?
            .checked_add(self.pending_output.len() as u64)
    }
    pub(crate) fn visible_comparison_units(&self) -> Option<u64> {
        if !self.level.tests()[self.index].visible {
            return Some(0);
        }
        match &self.level.tests()[self.index].data {
            SceneCaseData::Static { input, expected } => (input.len() as u64)
                .checked_add(expected.len() as u64)?
                .checked_add(
                    self.vm
                        .as_ref()
                        .map_or(0, |vm| vm.snapshot_view().output().len() as u64),
                ),
            SceneCaseData::MechanicalArm { expected, .. } => (expected.len() as u64).checked_add(
                self.machine
                    .as_ref()
                    .and_then(SceneMachine::actual_output)
                    .map_or(0, |a| a.len() as u64),
            ),
            _ => Some(0),
        }
    }
    pub(crate) fn visible_comparison(&self) -> Option<crate::scene_evaluate::SceneComparison> {
        if !self.level.tests()[self.index].visible {
            return None;
        }
        match &self.level.tests()[self.index].data {
            SceneCaseData::Static { input, expected } => {
                Some(crate::scene_evaluate::SceneComparison::Static {
                    input: input.clone(),
                    expected_output: expected.clone(),
                    actual_output: self
                        .vm
                        .as_ref()
                        .map_or_else(Vec::new, |vm| vm.snapshot_view().output().to_vec()),
                })
            }
            SceneCaseData::MechanicalArm { expected, .. } => {
                Some(crate::scene_evaluate::SceneComparison::MechanicalArm {
                    expected_output: expected.clone(),
                    actual_output: self
                        .machine
                        .as_ref()
                        .and_then(SceneMachine::actual_output)
                        .unwrap_or(&[])
                        .to_vec(),
                })
            }
            _ => None,
        }
    }
    pub(crate) fn pending_actions_count(&self) -> usize {
        self.machine
            .as_ref()
            .map_or(0, |m| m.pending_actions().len())
    }
    pub fn result(&self) -> Option<&SceneCaseResult> {
        self.result.as_ref()
    }
    pub fn vm_work(&self) -> u64 {
        self.vm_work
    }
    pub fn scene_work(&self) -> u64 {
        self.scene_work
    }
    pub fn total_input_bytes(&self) -> u64 {
        self.total_input
    }
    pub fn cancel(&mut self) {
        if self.result.is_none() {
            self.finish(EvaluationStatus::Cancelled, Vec::new());
        }
    }
    fn finish(&mut self, status: EvaluationStatus, runtime_errors: Vec<String>) {
        self.result = Some(SceneCaseResult {
            status,
            outcome: self.machine.as_ref().map(|m| m.outcome().clone()),
            summary: self
                .machine
                .as_ref()
                .map_or_else(BTreeMap::new, SceneMachine::summary),
            metrics: self.metrics.clone(),
            runtime_errors,
        });
        self.pending_output.clear();
    }
    fn progress(&self) -> SceneCaseProgress {
        self.result
            .clone()
            .map_or(SceneCaseProgress::Pending, SceneCaseProgress::Complete)
    }
    fn reserve_scene_work(&mut self, bound: u64, remaining: u64) -> bool {
        if bound > self.limits.max_scene_work_per_call.get()
            || bound
                > self
                    .limits
                    .max_total_scene_work
                    .get()
                    .saturating_sub(self.scene_work)
                    .saturating_sub(self.feedback_reserved_work)
        {
            self.finish(EvaluationStatus::ResourceLimitExceeded, Vec::new());
            false
        } else {
            remaining >= bound
        }
    }
    fn publish_metrics(&mut self) -> bool {
        if !self.include_metrics {
            self.metrics = self.base_metrics.clone();
            return true;
        }
        let Some(vm) = &self.vm else { return true };
        let dynamic = dynamic_metrics(&vm.snapshot_view().metrics().summary());
        let Some(metrics) = aggregate(&self.base_metrics, &dynamic) else {
            self.finish(
                EvaluationStatus::Fault(FaultReason::NumericOverflow),
                Vec::new(),
            );
            return false;
        };
        self.metrics = metrics;
        true
    }
    pub(crate) fn set_result_feedback_baseline(&mut self, baseline: u64) {
        self.feedback_result_base = baseline;
    }
    fn event_draft(
        &self,
        payload: SceneEventPayload,
        actor: Option<SceneActor>,
        tick: Option<u64>,
        frame: Option<u64>,
    ) -> SceneEventDraft {
        SceneEventDraft {
            source_index: self.index,
            scene_type: self.level.kind(),
            tick,
            frame_index: frame,
            actor,
            payload,
        }
    }
    fn terminal_payload(
        &self,
        machine: Option<&SceneMachine>,
        status: EvaluationStatus,
        codes: Vec<String>,
    ) -> SceneEventPayload {
        let result = SceneCaseResult {
            status,
            outcome: machine.map(|m| m.outcome().clone()),
            summary: machine.map_or_else(BTreeMap::new, SceneMachine::summary),
            metrics: Metrics::new(),
            runtime_errors: codes,
        };
        let (outcome, failure) = crate::scene_evaluate::case_terminal_projection(
            &self.level.tests()[self.index].data,
            &result,
            self.committed_ticks(),
            machine.map_or(0, |m| m.pending_actions().len()),
        );
        SceneEventPayload::CaseEnded { outcome, failure }
    }
    fn result_feedback_bound(
        &self,
        candidate: Option<&SceneMachine>,
        codes: &[String],
    ) -> Option<u64> {
        let fixed = match &self.level.tests()[self.index].data {
            SceneCaseData::Static { input, expected } => (input.len() as u64)
                .checked_add(expected.len() as u64)?
                .checked_add(
                    self.vm
                        .as_ref()
                        .map_or(0, |vm| vm.snapshot_view().output().len() as u64),
                )?,
            SceneCaseData::MechanicalArm { expected, .. } => (expected.len() as u64).checked_add(
                candidate
                    .and_then(SceneMachine::actual_output)
                    .map_or(0, |a| a.len() as u64),
            )?,
            _ => 0,
        };
        codes.iter().try_fold(
            self.feedback_result_base
                .checked_add(2048)?
                .checked_add(fixed.checked_mul(6)?)?,
            |sum, code| {
                sum.checked_add(96)?
                    .checked_add((code.len() as u64).checked_mul(6)?)
            },
        )
    }
    fn feedback_error(&mut self, error: SceneFeedbackError) {
        self.finish(
            if error == SceneFeedbackError::NumericOverflow {
                EvaluationStatus::Fault(FaultReason::NumericOverflow)
            } else {
                EvaluationStatus::ResourceLimitExceeded
            },
            Vec::new(),
        );
    }
    fn bill_scene(&mut self, work: u64, remaining: &mut u64) -> bool {
        if work > *remaining
            || self
                .scene_work
                .checked_add(work)
                .is_none_or(|n| n > self.limits.max_total_scene_work.get())
        {
            self.finish(
                EvaluationStatus::Fault(FaultReason::NumericOverflow),
                Vec::new(),
            );
            return false;
        }
        self.scene_work += work;
        *remaining -= work;
        true
    }
    fn publish_end(&mut self, feedback: &mut SceneFeedbackStore, remaining: &mut u64) -> bool {
        if self.feedback_ended {
            return true;
        }
        if !self.feedback_started {
            feedback.abandon_case_end();
            self.feedback_reserved_work = 0;
            self.feedback_ended = true;
            return true;
        }
        if *remaining < self.feedback_end_bound {
            return false;
        }
        let result = self.result.as_ref().unwrap();
        let Some(bound) = self.result_feedback_bound(self.machine.as_ref(), &result.runtime_errors)
        else {
            self.feedback_error(SceneFeedbackError::NumericOverflow);
            return true;
        };
        if let Err(error) = feedback.set_other_feedback_bytes_for_end(bound) {
            self.feedback_error(error);
            feedback.abandon_case_end();
            self.feedback_reserved_work = 0;
            self.feedback_ended = true;
            return true;
        }
        let event = self.event_draft(
            self.terminal_payload(
                self.machine.as_ref(),
                result.status.clone(),
                result.runtime_errors.clone(),
            ),
            None,
            self.committed_ticks(),
            None,
        );
        let (batch, work) = feedback.stage_with_accounting(vec![event]);
        if !self.bill_scene(work, remaining) {
            return true;
        }
        match batch {
            Ok(batch) => {
                if self
                    .retained_scene_units()
                    .and_then(|n| n.checked_add(batch.current_scene_units()))
                    .and_then(|n| n.checked_add(batch.retained_scene_units()))
                    .is_none_or(|n| n > self.limits.max_scene_state_units.get())
                {
                    drop(batch);
                    self.feedback_error(SceneFeedbackError::ResourceLimit);
                } else {
                    batch.commit();
                }
            }
            Err(error) => self.feedback_error(error),
        }
        self.feedback_reserved_work = 0;
        self.feedback_ended = true;
        true
    }
    pub fn advance(
        &mut self,
        vm_budget: NonZeroU64,
        scene_budget: NonZeroU64,
    ) -> SceneCaseProgress {
        self.advance_inner(vm_budget, scene_budget, None)
    }
    pub(crate) fn advance_with_feedback(
        &mut self,
        vm_budget: NonZeroU64,
        scene_budget: NonZeroU64,
        feedback: &mut SceneFeedbackStore,
    ) -> SceneCaseProgress {
        if self.level.tests()[self.index].visible {
            self.advance_inner(vm_budget, scene_budget, Some(feedback))
        } else {
            self.advance_inner(vm_budget, scene_budget, None)
        }
    }
    pub(crate) fn cancel_with_feedback(&mut self, feedback: &mut SceneFeedbackStore) {
        if self.feedback_ended {
            return;
        }
        self.finish(EvaluationStatus::Cancelled, Vec::new());
        let mut remaining = self.limits.max_scene_work_per_call.get();
        let _ = self.publish_end(feedback, &mut remaining);
    }
    fn advance_inner(
        &mut self,
        vm_budget: NonZeroU64,
        scene_budget: NonZeroU64,
        mut feedback: Option<&mut SceneFeedbackStore>,
    ) -> SceneCaseProgress {
        let mut vm_remaining = vm_budget.get().min(self.config.safety.per_call_work.get());
        let mut scene_remaining = scene_budget
            .get()
            .min(self.limits.max_scene_work_per_call.get());
        while self.result.is_none() {
            if self.machine.is_none() {
                let bound = if feedback.is_some() {
                    self.feedback_reserved_work = self.feedback_end_bound;
                    if self.feedback_end_bound > self.limits.max_scene_work_per_call.get() {
                        self.finish(EvaluationStatus::ResourceLimitExceeded, Vec::new());
                        break;
                    }
                    self.init_input_bound
                        .checked_mul(12)
                        .and_then(|n| n.checked_add(4096))
                        .and_then(|n| self.init_work_bound.checked_add(n))
                } else {
                    Some(self.init_work_bound)
                };
                let Some(bound) = bound else {
                    self.feedback_error(SceneFeedbackError::NumericOverflow);
                    break;
                };
                if !self.reserve_scene_work(bound, scene_remaining) {
                    break;
                }
                let old_events = feedback.as_ref().map_or(0, |s| s.retained_scene_units());
                if self
                    .init_state_bound
                    .checked_add(old_events)
                    .is_none_or(|n| n > self.limits.max_scene_state_units.get())
                {
                    self.finish(EvaluationStatus::ResourceLimitExceeded, Vec::new());
                    break;
                }
                // Construction starts only after reserving world/input state
                // and the non-resumable initialization work.
                let Some(mut machine) = SceneMachine::new(&self.level, self.index) else {
                    self.finish(
                        EvaluationStatus::Fault(FaultReason::VmInitialization),
                        Vec::new(),
                    );
                    break;
                };
                let work = machine.initialization_work();
                let input = machine.take_initial_input();
                if self
                    .definition_units
                    .checked_add(machine.retained_units())
                    .and_then(|n| n.checked_add(input.len() as u64))
                    .and_then(|n| n.checked_add(old_events))
                    .is_none_or(|n| n > self.limits.max_scene_state_units.get())
                    || input.len() as u64 > self.limits.max_input_queue_bytes.get()
                    || input.len() as u64 > self.limits.max_total_input_bytes_per_test.get()
                {
                    self.finish(EvaluationStatus::ResourceLimitExceeded, Vec::new());
                    break;
                }
                if !self.bill_scene(work, &mut scene_remaining) {
                    break;
                }
                let mut batch = None;
                if let Some(store) = feedback.as_deref_mut() {
                    let Some(bytes) = self.result_feedback_bound(Some(&machine), &[]) else {
                        self.feedback_error(SceneFeedbackError::NumericOverflow);
                        break;
                    };
                    if let Err(error) = store
                        .set_other_feedback_bytes(bytes)
                        .and_then(|_| store.reserve_case_end(self.feedback_end_bound))
                    {
                        self.feedback_error(error);
                        break;
                    }
                    let mut events = vec![self.event_draft(
                        SceneEventPayload::CaseStarted {
                            observation: (!input.is_empty()).then(|| input.clone()),
                        },
                        None,
                        None,
                        None,
                    )];
                    if !input.is_empty() {
                        events.push(self.event_draft(
                            SceneEventPayload::InputAppended {
                                bytes: input.clone(),
                            },
                            None,
                            None,
                            None,
                        ));
                    }
                    let initial_pass = machine.outcome() == &SceneOutcome::Passed;
                    if initial_pass {
                        events.push(self.event_draft(
                            self.terminal_payload(
                                Some(&machine),
                                EvaluationStatus::Passed,
                                Vec::new(),
                            ),
                            None,
                            None,
                            None,
                        ));
                    }
                    let (prepared, encoded) = store.stage_with_accounting(events);
                    if !self.bill_scene(encoded, &mut scene_remaining) {
                        break;
                    }
                    match prepared {
                        Ok(prepared) => batch = Some(prepared),
                        Err(error) => {
                            self.feedback_error(error);
                            break;
                        }
                    }
                    if self
                        .definition_units
                        .checked_add(machine.retained_units())
                        .and_then(|n| n.checked_add(batch.as_ref().unwrap().current_scene_units()))
                        .and_then(|n| n.checked_add(batch.as_ref().unwrap().retained_scene_units()))
                        .and_then(|n| n.checked_add(u64::from(!initial_pass)))
                        .is_none_or(|n| n > self.limits.max_scene_state_units.get())
                    {
                        drop(batch);
                        self.feedback_error(SceneFeedbackError::ResourceLimit);
                        break;
                    }
                }
                let input_count = input.len() as u64;
                let vm = Vm::new(
                    self.program.clone(),
                    input,
                    VmConfig::new(
                        self.config.boundary_mode,
                        mix64(self.config.shuffle_seed ^ 0x43474C564D303031),
                        self.config.custom_execution_limit,
                    ),
                );
                let Ok(vm) = vm else {
                    drop(batch);
                    self.finish(
                        EvaluationStatus::Fault(FaultReason::VmInitialization),
                        Vec::new(),
                    );
                    break;
                };
                if retained_state_units(&vm, &self.program)
                    .is_none_or(|n| n > self.config.safety.max_state_units.get())
                {
                    drop(batch);
                    self.finish(EvaluationStatus::ResourceLimitExceeded, Vec::new());
                    break;
                }
                let initial_pass = machine.outcome() == &SceneOutcome::Passed;
                if let Some(batch) = batch {
                    batch.commit();
                    self.feedback_started = true;
                    self.feedback_ended = initial_pass;
                }
                self.total_input = input_count;
                self.vm = Some(vm);
                self.machine = Some(machine);
                if !self.publish_metrics() {
                    break;
                }
                if initial_pass {
                    self.feedback_reserved_work = 0;
                    self.finish(EvaluationStatus::Passed, Vec::new());
                    break;
                }
            }
            if let Some(byte) = self.pending_output.front().copied() {
                let machine = self.machine.as_ref().unwrap();
                if machine.will_complete_frame()
                    && machine.frames() >= self.limits.max_scene_frames_per_test.get()
                {
                    self.finish(EvaluationStatus::ResourceLimitExceeded, Vec::new());
                    break;
                }
                let bound = machine
                    .transition_work_bound()
                    .and_then(|n| n.checked_add(machine.retained_units()))
                    .and_then(|n| {
                        if feedback.is_some() {
                            n.checked_add(8192)?.checked_add(self.feedback_end_bound)
                        } else {
                            Some(n)
                        }
                    });
                let Some(bound) = bound else {
                    self.feedback_error(SceneFeedbackError::NumericOverflow);
                    break;
                };
                if !self.reserve_scene_work(bound, scene_remaining) {
                    break;
                }
                let old_events = feedback.as_ref().map_or(0, |s| s.retained_scene_units());
                let machine = self.machine.as_ref().unwrap();
                if machine
                    .retained_units()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(self.definition_units))
                    .and_then(|n| n.checked_add(old_events))
                    .and_then(|n| n.checked_add(self.pending_output.len() as u64 + 16))
                    .is_none_or(|n| n > self.limits.max_scene_state_units.get())
                {
                    self.finish(EvaluationStatus::ResourceLimitExceeded, Vec::new());
                    break;
                }
                let copy_work = machine.retained_units();
                let mut candidate = machine.clone();
                let transition = if feedback.is_some() {
                    candidate.consume_debug(byte)
                } else {
                    candidate.consume(byte)
                };
                let Ok(transition) = transition else {
                    self.feedback_error(SceneFeedbackError::NumericOverflow);
                    break;
                };
                let Some(work) = copy_work.checked_add(transition.work) else {
                    self.feedback_error(SceneFeedbackError::NumericOverflow);
                    break;
                };
                if !self.bill_scene(work, &mut scene_remaining) {
                    break;
                }
                let dynamic = dynamic_metrics(
                    &self
                        .vm
                        .as_ref()
                        .unwrap()
                        .snapshot_view()
                        .metrics()
                        .summary(),
                );
                let Some(metrics) = (if self.include_metrics {
                    aggregate(&self.base_metrics, &dynamic)
                } else {
                    Some(self.base_metrics.clone())
                }) else {
                    self.feedback_error(SceneFeedbackError::NumericOverflow);
                    break;
                };
                let constraint = constraints_exceeded(&metrics, self.level.constraints());
                if !constraint && self.pending_halt && self.pending_output.len() == 1 {
                    candidate.halt();
                }
                let terminal = candidate.outcome() != &SceneOutcome::Running;
                let status = if terminal {
                    Some(match candidate.outcome() {
                        SceneOutcome::Failed(_) => EvaluationStatus::TestFailed,
                        SceneOutcome::Passed if constraint => EvaluationStatus::ConstraintExceeded,
                        SceneOutcome::Passed => EvaluationStatus::Passed,
                        _ => unreachable!("terminal scene"),
                    })
                } else if constraint {
                    Some(EvaluationStatus::ConstraintExceeded)
                } else {
                    None
                };
                let observation = if terminal || constraint || self.pending_halt {
                    Vec::new()
                } else {
                    transition.observation
                };
                let input_total = self.total_input.checked_add(observation.len() as u64);
                let vm = self.vm.as_ref().unwrap();
                let peak = self
                    .definition_units
                    .checked_add(self.machine.as_ref().unwrap().retained_units())
                    .and_then(|n| n.checked_add(candidate.retained_units()))
                    .and_then(|n| {
                        n.checked_add(observation.len() as u64 + self.pending_output.len() as u64)
                    })
                    .and_then(|n| n.checked_add(old_events));
                if peak.is_none_or(|n| n > self.limits.max_scene_state_units.get())
                    || (vm.snapshot_view().input().count() as u64)
                        .checked_add(observation.len() as u64)
                        .is_none_or(|n| n > self.limits.max_input_queue_bytes.get())
                    || input_total
                        .is_none_or(|n| n > self.limits.max_total_input_bytes_per_test.get())
                    || retained_state_units(vm, &self.program)
                        .and_then(|n| n.checked_add(observation.len() as u64))
                        .is_none_or(|n| n > self.config.safety.max_state_units.get())
                {
                    self.finish(EvaluationStatus::ResourceLimitExceeded, Vec::new());
                    break;
                }
                let mut batch = None;
                if let Some(store) = feedback.as_deref_mut() {
                    let Some(bytes) = self.result_feedback_bound(Some(&candidate), &[]) else {
                        self.feedback_error(SceneFeedbackError::NumericOverflow);
                        break;
                    };
                    let update = if status.is_some() {
                        store.set_other_feedback_bytes_for_end(bytes)
                    } else {
                        store.set_other_feedback_bytes(bytes)
                    };
                    if let Err(error) = update {
                        self.feedback_error(error);
                        break;
                    }
                    let frame = transition.completed_frame.then(|| candidate.frames());
                    let mut events = transition
                        .events
                        .into_iter()
                        .map(|e| {
                            self.event_draft(e.payload, e.actor, self.committed_ticks(), frame)
                        })
                        .collect::<Vec<_>>();
                    if !observation.is_empty() {
                        events.push(self.event_draft(
                            SceneEventPayload::InputAppended {
                                bytes: observation.clone(),
                            },
                            None,
                            self.committed_ticks(),
                            frame,
                        ));
                    }
                    if let Some(status) = &status {
                        events.push(self.event_draft(
                            self.terminal_payload(Some(&candidate), status.clone(), Vec::new()),
                            None,
                            self.committed_ticks(),
                            frame,
                        ));
                    }
                    let (prepared, encoded) = store.stage_with_accounting(events);
                    if !self.bill_scene(encoded, &mut scene_remaining) {
                        break;
                    }
                    match prepared {
                        Ok(prepared) => batch = Some(prepared),
                        Err(error) => {
                            self.feedback_error(error);
                            break;
                        }
                    }
                    if peak
                        .and_then(|n| n.checked_add(batch.as_ref().unwrap().retained_scene_units()))
                        .and_then(|n| n.checked_add(u64::from(status.is_none())))
                        .is_none_or(|n| n > self.limits.max_scene_state_units.get())
                    {
                        drop(batch);
                        self.feedback_error(SceneFeedbackError::ResourceLimit);
                        break;
                    }
                }
                if !observation.is_empty()
                    && self
                        .vm
                        .as_mut()
                        .unwrap()
                        .append_input(&observation)
                        .is_err()
                {
                    drop(batch);
                    self.finish(EvaluationStatus::ResourceLimitExceeded, Vec::new());
                    break;
                }
                if let Some(batch) = batch {
                    batch.commit();
                    if status.is_some() {
                        self.feedback_ended = true;
                        self.feedback_reserved_work = 0;
                    }
                }
                self.total_input = input_total.unwrap();
                self.machine = Some(candidate);
                self.metrics = metrics;
                self.pending_output.pop_front();
                if let Some(status) = status {
                    self.finish(status, Vec::new());
                    break;
                }
                continue;
            }
            if self.pending_halt {
                self.machine.as_mut().unwrap().halt();
                let status = if self.machine.as_ref().unwrap().outcome() == &SceneOutcome::Passed {
                    EvaluationStatus::Passed
                } else {
                    EvaluationStatus::TestFailed
                };
                self.finish(status, Vec::new());
                break;
            }
            let vm = self.vm.as_ref().unwrap();
            if vm.committed_ticks() >= self.config.safety.per_test_ticks.get()
                || self.vm_work >= self.config.safety.cumulative_work.get()
            {
                self.finish(EvaluationStatus::ResourceLimitExceeded, Vec::new());
                break;
            }
            if vm_remaining == 0 {
                break;
            }
            let budget = vm_remaining.min(self.config.safety.cumulative_work.get() - self.vm_work);
            let (step, used) = self
                .vm
                .as_mut()
                .unwrap()
                .step_with_work_accounting(NonZeroU64::new(budget).unwrap());
            self.vm_work += used;
            vm_remaining -= used;
            let Ok(step) = step else {
                if self.vm_work == self.config.safety.cumulative_work.get()
                    || budget == self.config.safety.per_call_work.get()
                {
                    self.finish(EvaluationStatus::ResourceLimitExceeded, Vec::new());
                }
                break;
            };
            if step.fault.is_some() {
                self.finish(EvaluationStatus::Fault(FaultReason::VmFault), Vec::new());
                break;
            }
            if !self.publish_metrics() {
                break;
            }
            if step.status == VmStatus::Error {
                self.finish(
                    EvaluationStatus::RuntimeError,
                    step.errors.iter().map(|e| e.code().into()).collect(),
                );
                break;
            }
            let rejection = step
                .events
                .iter()
                .find_map(|e| match e {
                    VmEvent::CodeChanged { new, .. } => {
                        validate_primary(self.level.rules(), *new, true).err()
                    }
                    _ => None,
                })
                .or_else(|| {
                    check_scope(
                        self.level.rules(),
                        self.vm.as_ref().unwrap().snapshot_view().runtime_program(),
                    )
                });
            if let Some(reason) = rejection {
                self.finish(EvaluationStatus::ProgramRejected(reason), Vec::new());
                break;
            }
            let vm = self.vm.as_ref().unwrap();
            if retained_state_units(vm, &self.program)
                .is_none_or(|n| n > self.config.safety.max_state_units.get())
                || vm.snapshot_view().output().len() as u64
                    > self.config.safety.max_output_bytes.get()
            {
                self.finish(EvaluationStatus::ResourceLimitExceeded, Vec::new());
                break;
            }
            self.pending_output = step.newly_emitted_output.into();
            self.pending_halt = step.status == VmStatus::Halted;
            if self.pending_output.is_empty()
                && constraints_exceeded(&self.metrics, self.level.constraints())
            {
                self.finish(EvaluationStatus::ConstraintExceeded, Vec::new());
                break;
            }
        }
        if self.result.is_some() {
            if let Some(store) = feedback {
                if !self.publish_end(store, &mut scene_remaining) {
                    return SceneCaseProgress::Pending;
                }
            }
        }
        self.progress()
    }
}
