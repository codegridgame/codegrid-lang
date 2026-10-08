//! Version-2 host lifecycle; all scene behavior is delegated to level core.
use crate::common::{
    bounded_json, invalid_config, invalid_handle, level_code, parse_versioned_request, positive,
    program_size, response_too_large, sha256, Operation, NEXT_SESSION,
};
use crate::profile::parse_decimal;
use crate::{project_scene_result, ApiError, SafetyProfileV2};
use codegrid_ir::VerifiedProgram;
use codegrid_level_core::{
    scene_evaluate::*,
    scene_feedback::SceneFeedbackError,
    scene_session::{scene_definition_units, SceneLimits},
    *,
};
use codegrid_model::BoundaryMode;
use serde_json::{json, Value};
use std::{collections::BTreeMap, num::NonZeroU64};

struct Retained<T> {
    value: T,
    bytes: u64,
    scene_units: u64,
    provenance: Value,
}
/// Isolated API-2 session. API-1 requests and profiles are never reinterpreted.
pub struct LevelApiV2 {
    profile: SafetyProfileV2,
    levels: BTreeMap<String, Retained<ValidatedSceneLevel>>,
    programs: BTreeMap<String, Retained<VerifiedProgram>>,
    evaluations: BTreeMap<String, Retained<SceneEvaluationSession>>,
    owner: u64,
    next: u64,
    retained_bytes: u64,
    scene_units: u64,
    stopped: bool,
    seed_source: Option<Box<dyn FnMut() -> u64>>,
}
impl LevelApiV2 {
    pub fn new(profile: SafetyProfileV2) -> Result<Self, ApiError> {
        profile.validate()?;
        let owner = NEXT_SESSION
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |n| n.checked_add(1),
            )
            .map_err(|_| {
                ApiError::new("level_api.handle_exhausted", "Session identity exhausted")
            })?;
        Ok(Self {
            profile,
            levels: BTreeMap::new(),
            programs: BTreeMap::new(),
            evaluations: BTreeMap::new(),
            owner,
            next: 1,
            retained_bytes: 0,
            scene_units: 0,
            stopped: false,
            seed_source: None,
        })
    }
    pub fn with_seed_source(mut self, source: impl FnMut() -> u64 + 'static) -> Self {
        self.seed_source = Some(Box::new(source));
        self
    }
    pub fn profile(&self) -> &SafetyProfileV2 {
        &self.profile
    }
    fn handle(&mut self, bytes: u64, units: u64) -> Result<String, ApiError> {
        if (self.levels.len() + self.programs.len() + self.evaluations.len()) as u64
            >= self.profile.max_handles
            || self
                .retained_bytes
                .checked_add(bytes)
                .is_none_or(|n| n > self.profile.max_state_bytes)
            || self
                .scene_units
                .checked_add(units)
                .is_none_or(|n| n > self.profile.scene_limits.max_scene_state_units)
        {
            return Err(resource("Retained state or handle ceiling reached"));
        }
        let value = self
            .owner
            .checked_shl(32)
            .filter(|_| self.owner <= u32::MAX as u64)
            .and_then(|n| n.checked_add(self.next))
            .filter(|_| self.next <= u32::MAX as u64)
            .ok_or_else(|| {
                ApiError::new("level_api.handle_exhausted", "Handle identity exhausted")
            })?;
        self.next = self.next.checked_add(1).ok_or_else(|| {
            ApiError::new("level_api.handle_exhausted", "Handle identity exhausted")
        })?;
        self.retained_bytes += bytes;
        self.scene_units += units;
        Ok(value.to_string())
    }
    pub fn request_json(&mut self, text: &str) -> String {
        let result = if text.len() as u64
            > self
                .profile
                .max_level_bytes
                .max(self.profile.max_source_bytes)
                .saturating_mul(8)
                .saturating_add(4096)
        {
            Err(resource("Request exceeds byte ceiling"))
        } else {
            parse_versioned_request(text, 2).and_then(|r| {
                if self.stopped {
                    Err(ApiError::new("level_api.shutdown", "Session is shut down"))
                } else {
                    self.request(r.operation)
                }
            })
        };
        match result {
            Ok(text) => text,
            Err(error) => bounded_json(&envelope("error", json!({"error":{"code":error.code,"error_number":error.error_number(),"message":error.message}})), self.profile.max_response_bytes)
                .unwrap_or_else(|| envelope("error", json!({"error":{"code":"level_api.response_too_large","error_number":codegrid_model::error_number("level","level_api.response_too_large"),"message":"Complete response exceeds trusted byte ceiling"}})).to_string()),
        }
    }
    fn response(&self, status: &str, fields: Value) -> Result<String, ApiError> {
        bounded_json(&envelope(status, fields), self.profile.max_response_bytes)
            .ok_or_else(response_too_large)
    }
    fn request(&mut self, operation: Operation) -> Result<String, ApiError> {
        match operation {
            Operation::Capabilities => self.response("ok", json!({"capabilities":{
                "api_version":2,"level_format_versions":[1],"profile_versions":[2],"scene_protocol_version":1,
                "evaluation_types":["ExactIO","Environment"],
                "scene_types":codegrid_level_core::scenes::SELECTED_SCENES.iter().map(|scene|scene.id()).collect::<Vec<_>>(),
                "scene_feedback":true,"evaluator_contract":"scene-v1/2026-10-05","evaluator_build":env!("CODEGRID_LEVEL_BUILD_ID")}})),
            Operation::LoadLevel { level_json } => {
                if level_json.len() as u64 > self.profile.max_level_bytes { return Err(resource("Level JSON exceeds trusted byte ceiling")); }
                let limits = LoadLimits { max_level_bytes:self.profile.max_level_bytes as usize,
                    max_tests:self.profile.max_tests as usize, max_total_test_bytes:self.profile.max_total_test_bytes as usize };
                match load_scene_level_json(level_json.as_bytes(), limits) {
                    Ok(level) => {
                        let units = scene_definition_units(&level).ok_or_else(|| resource("Scene definition accounting overflow"))?;
                        let handle = self.handle(level_json.len() as u64, units)?;
                        self.levels.insert(handle.clone(), Retained { value:level, bytes:level_json.len() as u64, scene_units:units, provenance:json!(sha256(level_json.as_bytes())) });
                        self.response("ok", json!({"handle":handle}))
                    }
                    Err(error) if matches!(error.reason, "InputSizeExceeded" | "TestCountExceeded" | "TestDataSizeExceeded") => Err(resource("Trusted level loading ceiling reached")),
                    Err(error) => {
                        if (error.path.len() as u64).checked_mul(6).and_then(|n|n.checked_add(512))
                            .is_none_or(|n|n > self.profile.max_response_bytes) {
                            return Err(response_too_large());
                        }
                         self.response("level_rejected", json!({"error":{"code":level_code(error.category),"error_number":codegrid_model::error_number("level",level_code(error.category)),"category":error.category,"reason":error.reason,"path":error.path}}))
                    }
                }
            }
            Operation::CompileProgram { source } => {
                if source.len() as u64 > self.profile.max_source_bytes { return Err(resource("Source exceeds byte ceiling")); }
                match codegrid_compiler::compile(&source) {
                    Ok(program) => {
                        let (cells, boards) = program_size(&program);
                        if cells > self.profile.max_program_cells || boards > self.profile.max_program_boards { return Err(resource("Compiled program exceeds trusted ceiling")); }
                        let handle = self.handle(source.len() as u64, 0)?;
                        self.programs.insert(handle.clone(), Retained { value:program, bytes:source.len() as u64, scene_units:0, provenance:json!(sha256(source.as_bytes())) });
                        self.response("ok",json!({"handle":handle}))
                    }
                    Err(diagnostics) => {
                        let bound = diagnostics.iter().try_fold(256u64, |n, d| {
                            n.checked_add(256)?.checked_add((d.code.len() as u64)
                                .checked_add(d.message.len() as u64)?.checked_mul(6)?)
                        });
                        if bound.is_none_or(|n| n > self.profile.max_response_bytes) {
                            return Err(response_too_large());
                        }
                         self.response("source_rejected", json!({"diagnostics":diagnostics.into_iter().map(|d|json!({"code":d.code,"error_number":codegrid_model::error_number("source",d.code).or_else(||codegrid_model::error_number("ir",d.code)),"message":d.message,"severity":format!("{:?}",d.severity),"span":{"start":d.span.start.to_string(),"end":d.span.end.to_string()}})).collect::<Vec<_>>()}))
                    }
                }
            }
            Operation::StartEvaluation { level, program, mode, boundary_mode, shuffle_seed, custom_execution_limit } => {
                let mode = match mode.as_str() { "Debug"=>EvaluationMode::Debug,"Official"=>EvaluationMode::Official,_=>return Err(invalid_config()) };
                let boundary_mode = match boundary_mode.as_str() { "Exit"=>BoundaryMode::Exit,"Wrap"=>BoundaryMode::Wrap,_=>return Err(invalid_config()) };
                let custom_execution_limit = positive(&custom_execution_limit)?;
                let level = self.levels.get(&level).ok_or_else(invalid_handle)?;
                let program = self.programs.get(&program).ok_or_else(invalid_handle)?;
                let input_bytes = level.bytes.checked_add(program.bytes).ok_or_else(||resource("Retained representation accounting overflow"))?;
                let available = self.profile.max_state_bytes.checked_sub(self.retained_bytes).and_then(|n|n.checked_sub(input_bytes)).ok_or_else(||resource("Retained state ceiling reached"))?;
                // Refuse at start when no feedback budget can cover the result
                // baseline, matching the v1 surface instead of returning a
                // handle that can only finish as ResourceLimitExceeded.
                let base = codegrid_level_core::evaluate::result_base_bound(level.value.level_id(), &self.profile.profile_id)
                    .ok_or_else(||resource("Result representation accounting overflow"))?;
                let feedback_budget = available.min(self.profile.max_response_bytes).min(self.profile.scene_limits.max_scene_feedback_bytes);
                if feedback_budget < base { return Err(resource("Complete result reservation exceeds retained state ceiling")); }
                let mut scene_limits: SceneLimits = self.profile.scene_limits.to_core()?;
                let available_units = self.profile.scene_limits.max_scene_state_units.checked_sub(self.scene_units).and_then(NonZeroU64::new).ok_or_else(||resource("Scene state ceiling reached"))?;
                if scene_definition_units(&level.value).is_none_or(|n| n > available_units.get()) { return Err(resource("Scene definition copies exceed state ceiling")); }
                scene_limits.max_scene_state_units = available_units;
                scene_limits.max_scene_feedback_bytes = positive(&feedback_budget.to_string())?;
                let provenance = json!({"level_sha256":level.provenance,"source_sha256":program.provenance,"profile_sha256":sha256(self.profile.wire_value().to_string().as_bytes())});
                let level = level.value.clone(); let program = program.value.clone();
                let shuffle_seed = match shuffle_seed { Some(s)=>parse_decimal(&s).ok_or_else(invalid_config)?, None=>self.seed_source.as_mut().ok_or_else(||ApiError::new("level_api.seed_required","Omitted seed requires an explicit host seed source"))?() };
                let config = EvaluationConfig { boundary_mode, shuffle_seed, custom_execution_limit,
                    safety: ExecutionSafetyProfile { id:self.profile.profile_id.clone(), version:2,
                        max_output_bytes:nz(self.profile.max_output_bytes)?, max_state_units:nz(self.profile.max_state_units)?,
                        max_feedback_bytes:nz(feedback_budget)?, per_test_ticks:nz(self.profile.max_ticks_per_test)?,
                        per_call_work:nz(self.profile.max_work_per_call)?, cumulative_work:nz(self.profile.max_total_work)? } };
                let bytes = input_bytes.checked_add(feedback_budget).ok_or_else(||resource("Retained state accounting overflow"))?;
                let handle = self.handle(bytes, available_units.get())?;
                self.evaluations.insert(handle.clone(), Retained { value:start_scene_evaluation(level,program,mode,config,scene_limits),bytes,scene_units:available_units.get(),provenance });
                self.response("ok",json!({"handle":handle}))
            }
            Operation::AdvanceEvaluation { evaluation, work_budget } => {
                let work = positive(&work_budget)?;
                if work.get() > self.profile.max_work_per_call { return Err(resource("Work budget exceeds per-call ceiling")); }
                let scene_work = nz(self.profile.scene_limits.max_scene_work_per_call)?;
                let record = self.evaluations.get_mut(&evaluation).ok_or_else(invalid_handle)?;
                match record.value.advance(work, scene_work) {
                    SceneEvaluationProgress::Pending => self.response("pending",json!({"evaluation":evaluation})),
                    SceneEvaluationProgress::Complete(result) => project_scene_result(&result,&record.provenance,self.profile.max_response_bytes),
                }
            }
            Operation::EvaluationResult { evaluation } => {
                let record = self.evaluations.get(&evaluation).ok_or_else(invalid_handle)?;
                match record.value.result() { Some(result)=>project_scene_result(result,&record.provenance,self.profile.max_response_bytes),None=>self.response("pending",json!({"evaluation":evaluation})) }
            }
            Operation::SceneFeedback { evaluation_handle, after_sequence, max_events } => {
                let after = parse_decimal(&after_sequence).ok_or_else(invalid_config)?;
                let max = positive(&max_events)?;
                if max.get() > self.profile.scene_limits.max_scene_events_per_evaluation { return Err(invalid_config()); }
                let record = self.evaluations.get_mut(&evaluation_handle).ok_or_else(invalid_handle)?;
                let read = record.value.prepare_scene_feedback(after,max).map_err(feedback_error)?;
                let response = feedback_response(&read,self.profile.max_response_bytes)?;
                read.commit();
                Ok(response)
            }
            Operation::Release { kind, handle } => {
                let record = match kind.as_str() {
                    "level"=>self.levels.remove(&handle).map(|r|(r.bytes,r.scene_units)),
                    "program"=>self.programs.remove(&handle).map(|r|(r.bytes,r.scene_units)),
                    "evaluation"=>self.evaluations.remove(&handle).map(|mut r|{r.value.cancel();(r.bytes,r.scene_units)}),
                    _=>None,
                }.ok_or_else(invalid_handle)?;
                self.retained_bytes -= record.0; self.scene_units -= record.1;
                self.response("ok",json!({}))
            }
            Operation::Shutdown => {
                self.levels.clear();self.programs.clear();self.evaluations.clear();self.retained_bytes=0;self.scene_units=0;self.stopped=true;
                self.response("ok",json!({}))
            }
        }
    }
}
fn nz(value: u64) -> Result<NonZeroU64, ApiError> {
    NonZeroU64::new(value).ok_or_else(invalid_config)
}
fn resource(message: &str) -> ApiError {
    ApiError::new("level_api.resource_limit", message)
}
fn feedback_error(error: SceneFeedbackError) -> ApiError {
    match error {
        SceneFeedbackError::ResourceLimit => resource("Scene feedback retention ceiling reached"),
        SceneFeedbackError::InvalidCursor => ApiError::new(
            "level_api.invalid_request",
            "Scene feedback cursor is stale or was not delivered",
        ),
        SceneFeedbackError::NumericOverflow => {
            ApiError::new("level.metric_overflow", "Scene feedback counter overflow")
        }
        _ => invalid_config(),
    }
}
fn envelope(status: &str, fields: Value) -> Value {
    let mut value = json!({"schema":"codegrid.level.response","api_version":2,"status":status});
    if let (Some(target), Some(fields)) = (value.as_object_mut(), fields.as_object()) {
        target.extend(fields.clone());
    }
    value
}

// The core already encoded/billed immutable event bodies at publication.
// Reserve the complete envelope before copying those bytes or confirming delivery.
fn feedback_response(
    read: &codegrid_level_core::scene_feedback::SceneFeedbackRead<'_>,
    limit: u64,
) -> Result<String, ApiError> {
    let prefix = r#"{"schema":"codegrid.level.response","api_version":2,"status":"ok","events":["#;
    let page = read.page();
    let suffix = format!(
        "],\"next_sequence\":\"{}\",\"has_more\":{}}}",
        page.next_sequence, page.has_more
    );
    let bound = read
        .encoded_event_texts()
        .try_fold((prefix.len() + suffix.len()) as u64, |n, event| {
            n.checked_add(event.len() as u64)
        })
        .and_then(|n| n.checked_add(page.events.len().saturating_sub(1) as u64))
        .filter(|n| *n <= limit)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(response_too_large)?;
    let mut response = String::new();
    response
        .try_reserve_exact(bound)
        .map_err(|_| resource("Response allocation failed"))?;
    response.push_str(prefix);
    for (index, event) in read.encoded_event_texts().enumerate() {
        if index != 0 {
            response.push(',');
        }
        response.push_str(event);
    }
    response.push_str(&suffix);
    Ok(response)
}

#[cfg(test)]
mod feedback_response_tests {
    use super::*;
    use codegrid_level_core::{scene_feedback::*, scenes::SceneKind};
    #[test]
    fn cached_envelope_overflow_does_not_confirm_delivery_and_exact_fit_succeeds() {
        let mut store =
            SceneFeedbackStore::new(EvaluationMode::Debug, nz(10).unwrap(), nz(100_000).unwrap());
        store
            .stage(vec![SceneEventDraft {
                source_index: 0,
                scene_type: SceneKind::Robot,
                tick: Some(0),
                frame_index: None,
                actor: None,
                payload: SceneEventPayload::InputAppended {
                    bytes: vec![255; 1000],
                },
            }])
            .unwrap()
            .commit();
        {
            let read = store.prepare_read(0, nz(10).unwrap()).unwrap();
            assert_eq!(
                feedback_response(&read, 512).unwrap_err().code,
                "level_api.response_too_large"
            );
        }
        assert!(matches!(
            store.read(1, nz(1).unwrap()),
            Err(SceneFeedbackError::InvalidCursor)
        ));
        let read = store.prepare_read(0, nz(10).unwrap()).unwrap();
        let response = feedback_response(&read, 100_000).unwrap();
        assert!(feedback_response(&read, response.len() as u64 - 1).is_err());
        assert_eq!(
            feedback_response(&read, response.len() as u64).unwrap(),
            response
        );
        let value: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(
            value["events"],
            serde_json::to_value(&read.page().events).unwrap()
        );
        read.commit();
        assert!(store.read(1, nz(1).unwrap()).unwrap().events.is_empty());
    }
}
