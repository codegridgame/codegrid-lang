//! Shared multi-case evaluation, visible metrics, and hidden-data-safe results.
use crate::scene_feedback::{
    SceneFeedbackError, SceneFeedbackPage, SceneFeedbackRead, SceneFeedbackStore,
};
use crate::{
    evaluate::{result_base_bound, shuffled_indices},
    scene_protocol::{SceneFailure, SceneOutcome},
    scene_session::{SceneCaseProgress, SceneCaseResult, SceneCaseSession, SceneLimits},
    scenes::SceneKind,
    *,
};
use codegrid_ir::VerifiedProgram;
use codegrid_vm::mix64;
use std::{collections::BTreeMap, num::NonZeroU64, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneComparison {
    Static {
        input: Vec<u8>,
        expected_output: Vec<u8>,
        actual_output: Vec<u8>,
    },
    MechanicalArm {
        expected_output: Vec<u8>,
        actual_output: Vec<u8>,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisibleSceneOutcome {
    Passed,
    WrongOutput,
    IncompleteOutput,
    SceneFailure,
    RuntimeError,
    NotCompleted,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HiddenSceneFailure {
    SceneFailure,
    RuntimeError,
    IncompleteGoal,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VisibleSceneFailure {
    Scene {
        failure: SceneFailure,
        tick: Option<u64>,
        frame_index: Option<u64>,
        pending_actions: Option<usize>,
    },
    RuntimeError {
        codes: Vec<String>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisibleSceneCase {
    pub source_index: usize,
    pub scene_type: SceneKind,
    pub outcome: VisibleSceneOutcome,
    pub failure: Option<VisibleSceneFailure>,
    pub comparison: Option<SceneComparison>,
    pub summary: BTreeMap<&'static str, u64>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SceneEvaluationResult {
    pub level_id: String,
    pub level_version: u32,
    pub level_format_version: u32,
    pub scene_type: SceneKind,
    pub scene_protocol_version: u32,
    pub evaluator_contract: String,
    pub gas_schedule_version: u32,
    pub mode: EvaluationMode,
    pub config: EvaluationConfig,
    pub scene_limits: SceneLimits,
    pub vm_seed: u64,
    pub status: EvaluationStatus,
    pub hidden_failure: Option<HiddenSceneFailure>,
    pub visible_cases: Vec<VisibleSceneCase>,
    pub partial_metrics: Metrics,
    pub final_metrics: Option<Metrics>,
    pub rating: Option<u8>,
    pub constraints: Vec<ConstraintResult>,
    pub scoring: Vec<ScoringResult>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneEvaluationProgress {
    Pending,
    Complete(SceneEvaluationResult),
}

/// No raw hidden world, VM, test order, or failure data is exposed by this type.
pub struct SceneEvaluationSession {
    level: Arc<ValidatedSceneLevel>,
    program: VerifiedProgram,
    config: EvaluationConfig,
    limits: SceneLimits,
    mode: EvaluationMode,
    order: Vec<usize>,
    position: usize,
    case: Option<SceneCaseSession>,
    vm_work: u64,
    scene_work: u64,
    metrics: Metrics,
    visible: Vec<VisibleSceneCase>,
    hidden_failure: Option<HiddenSceneFailure>,
    failure: Option<EvaluationStatus>,
    result: Option<SceneEvaluationResult>,
    feedback_bytes: u64,
    result_units: u64,
    feedback: SceneFeedbackStore,
}

pub fn start_scene_evaluation(
    level: ValidatedSceneLevel,
    program: VerifiedProgram,
    mode: EvaluationMode,
    config: EvaluationConfig,
    limits: SceneLimits,
) -> SceneEvaluationSession {
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
    let feedback = SceneFeedbackStore::new(
        mode,
        limits.max_scene_events_per_evaluation,
        NonZeroU64::new(
            limits
                .max_scene_feedback_bytes
                .get()
                .min(config.safety.max_feedback_bytes.get()),
        )
        .unwrap(),
    );
    let mut session = SceneEvaluationSession {
        level: Arc::new(level),
        program,
        config,
        limits,
        mode,
        order,
        position: 0,
        case: None,
        vm_work: 0,
        scene_work: 0,
        metrics,
        visible: Vec::new(),
        hidden_failure: None,
        failure: None,
        result: None,
        feedback_bytes: 0,
        result_units: 0,
        feedback,
    };
    let baseline = result_base_bound(session.level.level_id(), &session.config.safety.id);
    if baseline.is_none_or(|n| session.feedback.set_other_feedback_bytes(n).is_err()) {
        session.finish(EvaluationStatus::ResourceLimitExceeded);
        return session;
    }
    if let Err(reason) = validate_program_rules(session.level.rules(), &session.program) {
        session.finish(EvaluationStatus::ProgramRejected(reason));
    } else if constraints_exceeded(&session.metrics, session.level.constraints()) {
        session.failure = Some(EvaluationStatus::ConstraintExceeded);
        if mode == EvaluationMode::Official {
            session.finish(EvaluationStatus::ConstraintExceeded);
        }
    }
    session
}
pub fn evaluate_scene(
    level: ValidatedSceneLevel,
    program: VerifiedProgram,
    mode: EvaluationMode,
    config: EvaluationConfig,
    limits: SceneLimits,
) -> SceneEvaluationResult {
    let vm = config.safety.per_call_work;
    let scene = limits.max_scene_work_per_call;
    let mut session = start_scene_evaluation(level, program, mode, config, limits);
    loop {
        if let SceneEvaluationProgress::Complete(result) = session.advance(vm, scene) {
            return result;
        }
    }
}
impl SceneEvaluationSession {
    pub fn prepare_scene_feedback(
        &mut self,
        after_sequence: u64,
        max_events: NonZeroU64,
    ) -> Result<SceneFeedbackRead<'_>, SceneFeedbackError> {
        if self.mode != EvaluationMode::Debug {
            return Err(SceneFeedbackError::InvalidConfiguration);
        }
        let current = match &self.case {
            Some(case) => case.retained_scene_units(),
            None => crate::scene_session::scene_definition_units(&self.level),
        }
        .ok_or(SceneFeedbackError::NumericOverflow)?;
        let retained = current
            .checked_add(self.result_units)
            .and_then(|n| n.checked_add(self.feedback.retained_scene_units()))
            .ok_or(SceneFeedbackError::NumericOverflow)?;
        let available = self
            .limits
            .max_scene_state_units
            .get()
            .checked_sub(retained)
            .ok_or(SceneFeedbackError::ResourceLimit)?;
        self.feedback
            .prepare_read_with_scene_units(after_sequence, max_events, available)
    }
    pub fn scene_feedback(
        &mut self,
        after_sequence: u64,
        max_events: NonZeroU64,
    ) -> Result<SceneFeedbackPage, SceneFeedbackError> {
        Ok(self
            .prepare_scene_feedback(after_sequence, max_events)?
            .commit())
    }
    pub fn result(&self) -> Option<&SceneEvaluationResult> {
        self.result.as_ref()
    }
    pub fn vm_work(&self) -> u64 {
        self.vm_work
    }
    pub fn scene_work(&self) -> u64 {
        self.scene_work
    }
    pub fn cancel(&mut self) {
        if self.result.is_some() {
            return;
        }
        if let Some(case) = &mut self.case {
            let before = case.scene_work();
            if self.mode == EvaluationMode::Debug {
                case.cancel_with_feedback(&mut self.feedback);
            } else {
                case.cancel();
            }
            self.scene_work += case.scene_work() - before;
            self.metrics = case.current_metrics().clone();
            let result = case.result().cloned();
            if let Some(result) = result {
                let index = self.order[self.position];
                let _ = self.retain_visible_case(index, &result);
            }
        }
        self.finish(EvaluationStatus::Cancelled);
    }
    fn finish(&mut self, status: EvaluationStatus) {
        let passed = status == EvaluationStatus::Passed;
        let constraints = self
            .level
            .constraints()
            .iter()
            .map(|(name, limit)| {
                let value = self
                    .metrics
                    .get(constraint_metric(name))
                    .copied()
                    .unwrap_or(0);
                ConstraintResult {
                    name: name.clone(),
                    limit: *limit,
                    value,
                    passed: value <= *limit,
                }
            })
            .collect();
        let scoring = self
            .level
            .scoring()
            .iter()
            .map(|(name, target)| {
                let value = self.metrics.get(name).copied().unwrap_or(0);
                ScoringResult {
                    name: name.clone(),
                    target: *target,
                    value,
                    rating: if passed {
                        rating(&self.metrics, &BTreeMap::from([(name.clone(), *target)]))
                    } else {
                        None
                    },
                }
            })
            .collect();
        self.result = Some(SceneEvaluationResult {
            level_id: self.level.level_id().into(),
            level_version: self.level.level_version(),
            level_format_version: 1,
            scene_type: self.level.kind(),
            scene_protocol_version: 1,
            evaluator_contract: "scene-v1/2026-10-05".into(),
            gas_schedule_version: crate::GAS_SCHEDULE_VERSION,
            mode: self.mode,
            config: self.config.clone(),
            scene_limits: self.limits.clone(),
            vm_seed: mix64(self.config.shuffle_seed ^ 0x43474C564D303031),
            status,
            hidden_failure: self.hidden_failure,
            visible_cases: std::mem::take(&mut self.visible),
            partial_metrics: self.metrics.clone(),
            final_metrics: passed.then(|| self.metrics.clone()),
            rating: passed
                .then(|| rating(&self.metrics, self.level.scoring()))
                .flatten(),
            constraints,
            scoring,
        });
        self.case = None;
    }
    fn progress(&self) -> SceneEvaluationProgress {
        self.result.clone().map_or(
            SceneEvaluationProgress::Pending,
            SceneEvaluationProgress::Complete,
        )
    }
    fn retain_visible_case(&mut self, index: usize, result: &SceneCaseResult) -> bool {
        if !self.level.tests()[index].visible {
            return true;
        }
        let units = self.case.as_ref().unwrap().visible_comparison_units();
        let failure_bytes = result.runtime_errors.iter().try_fold(0u64, |sum, code| {
            sum.checked_add(96)?
                .checked_add((code.len() as u64).checked_mul(6)?)
        });
        let bound = units
            .and_then(|n| n.checked_mul(6))
            .and_then(|n| n.checked_add(failure_bytes?))
            .and_then(|n| n.checked_add(2048))
            .and_then(|n| n.checked_add(self.feedback_bytes));
        let total = bound.and_then(|n| {
            result_base_bound(self.level.level_id(), &self.config.safety.id)?
                .checked_add(n)?
                .checked_add(self.feedback.retained_bytes())
        });
        let state = units
            .and_then(|n| n.checked_add(self.result_units))
            .and_then(|n| {
                self.case
                    .as_ref()?
                    .retained_scene_units()?
                    .checked_add(n)?
                    .checked_add(self.feedback.retained_scene_units())
            });
        if total.is_none_or(|n| {
            n > self.limits.max_scene_feedback_bytes.get()
                || n > self.config.safety.max_feedback_bytes.get()
        }) || state.is_none_or(|n| n > self.limits.max_scene_state_units.get())
        {
            return false;
        }
        let Some(other) = bound.and_then(|n| {
            result_base_bound(self.level.level_id(), &self.config.safety.id)?.checked_add(n)
        }) else {
            return false;
        };
        if self.feedback.set_other_feedback_bytes(other).is_err() {
            return false;
        }
        self.feedback_bytes = bound.unwrap();
        self.result_units += units.unwrap();
        let projection = self.project_case(index, result);
        self.visible.push(projection);
        true
    }
    fn project_case(&self, index: usize, result: &SceneCaseResult) -> VisibleSceneCase {
        let (outcome, failure) = case_terminal_projection(
            &self.level.tests()[index].data,
            result,
            self.case
                .as_ref()
                .and_then(SceneCaseSession::committed_ticks),
            self.case.as_ref().unwrap().pending_actions_count(),
        );
        VisibleSceneCase {
            source_index: index,
            scene_type: self.level.kind(),
            outcome,
            failure,
            comparison: self
                .case
                .as_ref()
                .and_then(SceneCaseSession::visible_comparison),
            summary: result.summary.clone(),
        }
    }
    pub fn advance(
        &mut self,
        vm_budget: NonZeroU64,
        scene_budget: NonZeroU64,
    ) -> SceneEvaluationProgress {
        if self.result.is_some() {
            return self.progress();
        }
        let mut vm_remaining = vm_budget.get().min(self.config.safety.per_call_work.get());
        let mut scene_remaining = scene_budget
            .get()
            .min(self.limits.max_scene_work_per_call.get());
        loop {
            if self.position == self.order.len() {
                self.finish(self.failure.clone().unwrap_or(EvaluationStatus::Passed));
                break;
            }
            if self.case.is_none() {
                if self.vm_work >= self.config.safety.cumulative_work.get()
                    || self.scene_work >= self.limits.max_total_scene_work.get()
                {
                    self.finish(EvaluationStatus::ResourceLimitExceeded);
                    break;
                }
                let Some(state_limit) = self
                    .limits
                    .max_scene_state_units
                    .get()
                    .checked_sub(self.result_units)
                    .and_then(NonZeroU64::new)
                else {
                    self.finish(EvaluationStatus::ResourceLimitExceeded);
                    break;
                };
                let index = self.order[self.position];
                let mut config = self.config.clone();
                let mut limits = self.limits.clone();
                config.safety.cumulative_work =
                    NonZeroU64::new(self.config.safety.cumulative_work.get() - self.vm_work)
                        .unwrap();
                limits.max_total_scene_work =
                    NonZeroU64::new(self.limits.max_total_scene_work.get() - self.scene_work)
                        .unwrap();
                limits.max_scene_state_units = state_limit;
                let case = SceneCaseSession::from_shared_with_metrics(
                    self.level.clone(),
                    self.program.clone(),
                    index,
                    config,
                    limits,
                    self.metrics.clone(),
                );
                match case {
                    Ok(case) => self.case = Some(case),
                    Err(_) => {
                        self.finish(EvaluationStatus::Fault(FaultReason::NumericOverflow));
                        break;
                    }
                }
            }
            if vm_remaining == 0 || scene_remaining == 0 {
                break;
            }
            let case = self.case.as_mut().unwrap();
            let before_vm = case.vm_work();
            let before_scene = case.scene_work();
            let Some(baseline) = result_base_bound(self.level.level_id(), &self.config.safety.id)
                .and_then(|n| n.checked_add(self.feedback_bytes))
            else {
                self.finish(EvaluationStatus::Fault(FaultReason::NumericOverflow));
                break;
            };
            case.set_result_feedback_baseline(baseline);
            let progress = if self.mode == EvaluationMode::Debug {
                case.advance_with_feedback(
                    NonZeroU64::new(vm_remaining).unwrap(),
                    NonZeroU64::new(scene_remaining).unwrap(),
                    &mut self.feedback,
                )
            } else {
                case.advance(
                    NonZeroU64::new(vm_remaining).unwrap(),
                    NonZeroU64::new(scene_remaining).unwrap(),
                )
            };
            self.metrics = case.current_metrics().clone();
            let used_vm = case.vm_work() - before_vm;
            let used_scene = case.scene_work() - before_scene;
            self.vm_work += used_vm;
            self.scene_work += used_scene;
            vm_remaining -= used_vm;
            scene_remaining -= used_scene;
            let SceneCaseProgress::Complete(case_result) = progress else {
                break;
            };
            let index = self.order[self.position];
            let visible = self.level.tests()[index].visible;
            self.metrics = case_result.metrics.clone();
            if visible {
                if !self.retain_visible_case(index, &case_result) {
                    self.finish(EvaluationStatus::ResourceLimitExceeded);
                    break;
                }
            } else if matches!(
                case_result.status,
                EvaluationStatus::TestFailed | EvaluationStatus::RuntimeError
            ) {
                self.hidden_failure = Some(match &case_result.status {
                    EvaluationStatus::RuntimeError => HiddenSceneFailure::RuntimeError,
                    _ if matches!(
                        case_result.outcome,
                        Some(SceneOutcome::Failed(SceneFailure::IncompleteGoal))
                    ) =>
                    {
                        HiddenSceneFailure::IncompleteGoal
                    }
                    _ => HiddenSceneFailure::SceneFailure,
                });
            }
            let mut status = case_result.status;
            if !visible {
                if let EvaluationStatus::ProgramRejected(reason) = &mut status {
                    reason.path.clear();
                }
            }
            if status != EvaluationStatus::Passed && self.failure.is_none() {
                self.failure = Some(status.clone());
            }
            self.case = None;
            self.position += 1;
            if matches!(
                status,
                EvaluationStatus::ResourceLimitExceeded
                    | EvaluationStatus::Cancelled
                    | EvaluationStatus::Fault(_)
                    | EvaluationStatus::ProgramRejected(_)
            ) {
                self.finish(status);
                break;
            }
            if self.mode == EvaluationMode::Official && self.failure.is_some() {
                self.finish(self.failure.clone().unwrap());
                break;
            }
        }
        self.progress()
    }
}

pub(crate) fn case_terminal_projection(
    case_data: &SceneCaseData,
    result: &SceneCaseResult,
    tick: Option<u64>,
    pending_count: usize,
) -> (VisibleSceneOutcome, Option<VisibleSceneFailure>) {
    let outcome = match &result.status {
        EvaluationStatus::RuntimeError => VisibleSceneOutcome::RuntimeError,
        EvaluationStatus::ResourceLimitExceeded
        | EvaluationStatus::Cancelled
        | EvaluationStatus::Fault(_)
        | EvaluationStatus::ProgramRejected(_) => VisibleSceneOutcome::NotCompleted,
        _ => match &result.outcome {
            Some(SceneOutcome::Passed) => VisibleSceneOutcome::Passed,
            Some(SceneOutcome::Failed(SceneFailure::WrongOutput { .. })) => {
                VisibleSceneOutcome::WrongOutput
            }
            Some(SceneOutcome::Failed(SceneFailure::IncompleteGoal))
                if matches!(case_data, SceneCaseData::Static { .. }) =>
            {
                VisibleSceneOutcome::IncompleteOutput
            }
            Some(SceneOutcome::Failed(_)) => VisibleSceneOutcome::SceneFailure,
            _ => VisibleSceneOutcome::NotCompleted,
        },
    };
    let failure = match outcome {
        VisibleSceneOutcome::RuntimeError => Some(VisibleSceneFailure::RuntimeError {
            codes: result.runtime_errors.clone(),
        }),
        VisibleSceneOutcome::WrongOutput
        | VisibleSceneOutcome::IncompleteOutput
        | VisibleSceneOutcome::SceneFailure => match &result.outcome {
            Some(SceneOutcome::Failed(failure)) => {
                let incomplete = matches!(failure, SceneFailure::IncompleteGoal);
                let frames = result.summary.get("frames").copied().filter(|n| *n > 0);
                Some(VisibleSceneFailure::Scene {
                    failure: failure.clone(),
                    tick,
                    frame_index: if incomplete { None } else { frames },
                    pending_actions: (incomplete
                        && !matches!(case_data, SceneCaseData::Static { .. }))
                    .then_some(pending_count),
                })
            }
            _ => None,
        },
        _ => None,
    };
    (outcome, failure)
}
