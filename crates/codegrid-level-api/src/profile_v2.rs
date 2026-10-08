//! Strict trusted scene profile conversion. Scene semantics remain in level core.
use crate::profile::{positive, ApiError, CommonLimits};
use codegrid_level_core::scene_session::SceneLimits;
use serde::Deserialize;
use serde_json::{json, Value};
use std::num::NonZeroU64;

/// Trusted API-2 configuration, never accepted from level or program data.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SafetyProfileV2 {
    pub profile_version: u32,
    pub profile_id: String,
    pub provenance: String,
    #[serde(deserialize_with = "positive")]
    pub max_level_bytes: u64,
    #[serde(deserialize_with = "positive")]
    pub max_source_bytes: u64,
    #[serde(deserialize_with = "positive")]
    pub max_tests: u64,
    #[serde(deserialize_with = "positive")]
    pub max_total_test_bytes: u64,
    #[serde(deserialize_with = "positive")]
    pub max_program_cells: u64,
    #[serde(deserialize_with = "positive")]
    pub max_program_boards: u64,
    #[serde(deserialize_with = "positive")]
    pub max_state_bytes: u64,
    #[serde(deserialize_with = "positive")]
    pub max_state_units: u64,
    #[serde(deserialize_with = "positive")]
    pub max_output_bytes: u64,
    #[serde(deserialize_with = "positive")]
    pub max_handles: u64,
    #[serde(deserialize_with = "positive")]
    pub max_response_bytes: u64,
    #[serde(deserialize_with = "positive")]
    pub max_ticks_per_test: u64,
    #[serde(deserialize_with = "positive")]
    pub max_work_per_call: u64,
    #[serde(deserialize_with = "positive")]
    pub max_total_work: u64,
    pub scene_limits: SceneProfileLimits,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneProfileLimits {
    #[serde(deserialize_with = "positive")]
    pub max_input_queue_bytes: u64,
    #[serde(deserialize_with = "positive")]
    pub max_total_input_bytes_per_test: u64,
    #[serde(deserialize_with = "positive")]
    pub max_scene_state_units: u64,
    #[serde(deserialize_with = "positive")]
    pub max_scene_frames_per_test: u64,
    #[serde(deserialize_with = "positive")]
    pub max_scene_work_per_call: u64,
    #[serde(deserialize_with = "positive")]
    pub max_total_scene_work: u64,
    #[serde(deserialize_with = "positive")]
    pub max_scene_events_per_evaluation: u64,
    #[serde(deserialize_with = "positive")]
    pub max_scene_feedback_bytes: u64,
}
impl SafetyProfileV2 {
    pub fn from_json(text: &str) -> Result<Self, ApiError> {
        let profile: Self = serde_json::from_str(text)
            .map_err(|e| ApiError::new("level_api.invalid_profile", e.to_string()))?;
        profile.validate()?;
        Ok(profile)
    }
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.profile_version != 2 {
            return Err(ApiError::new(
                "level_api.unsupported_profile_version",
                "Expected safety profile version 2",
            ));
        }
        self.common_limits().validate()?;
        self.scene_limits.to_core()?;
        Ok(())
    }
    // Validate shared resource ceilings without another public profile format.
    pub(crate) fn common_limits(&self) -> CommonLimits {
        CommonLimits {
            profile_id: self.profile_id.clone(),
            provenance: self.provenance.clone(),
            max_level_bytes: self.max_level_bytes,
            max_source_bytes: self.max_source_bytes,
            max_tests: self.max_tests,
            max_total_test_bytes: self.max_total_test_bytes,
            max_program_cells: self.max_program_cells,
            max_program_boards: self.max_program_boards,
            max_state_bytes: self.max_state_bytes,
            max_state_units: self.max_state_units,
            max_output_bytes: self.max_output_bytes,
            max_handles: self.max_handles,
            max_response_bytes: self.max_response_bytes,
            max_ticks_per_test: self.max_ticks_per_test,
            max_work_per_call: self.max_work_per_call,
            max_total_work: self.max_total_work,
        }
    }
    pub(crate) fn wire_value(&self) -> Value {
        json!({"profile_version":2,"profile_id":self.profile_id,"provenance":self.provenance,
        "max_level_bytes":self.max_level_bytes.to_string(),
        "max_source_bytes":self.max_source_bytes.to_string(),
        "max_tests":self.max_tests.to_string(),
        "max_total_test_bytes":self.max_total_test_bytes.to_string(),
        "max_program_cells":self.max_program_cells.to_string(),
        "max_program_boards":self.max_program_boards.to_string(),
        "max_state_bytes":self.max_state_bytes.to_string(),
        "max_state_units":self.max_state_units.to_string(),
        "max_output_bytes":self.max_output_bytes.to_string(),
        "max_handles":self.max_handles.to_string(),
        "max_response_bytes":self.max_response_bytes.to_string(),
        "max_ticks_per_test":self.max_ticks_per_test.to_string(),
        "max_work_per_call":self.max_work_per_call.to_string(),
        "max_total_work":self.max_total_work.to_string(),
        "scene_limits":{
            "max_input_queue_bytes":self.scene_limits.max_input_queue_bytes.to_string(),
            "max_total_input_bytes_per_test":self.scene_limits.max_total_input_bytes_per_test.to_string(),
            "max_scene_state_units":self.scene_limits.max_scene_state_units.to_string(),
            "max_scene_frames_per_test":self.scene_limits.max_scene_frames_per_test.to_string(),
            "max_scene_work_per_call":self.scene_limits.max_scene_work_per_call.to_string(),
            "max_total_scene_work":self.scene_limits.max_total_scene_work.to_string(),
            "max_scene_events_per_evaluation":self.scene_limits.max_scene_events_per_evaluation.to_string(),
            "max_scene_feedback_bytes":self.scene_limits.max_scene_feedback_bytes.to_string()
        }})
    }
    pub fn identity_json(&self) -> Value {
        json!({"profile_id": self.profile_id, "profile_version": 2})
    }
}
impl SceneProfileLimits {
    pub fn to_core(&self) -> Result<SceneLimits, ApiError> {
        fn limit(value: u64) -> Result<NonZeroU64, ApiError> {
            NonZeroU64::new(value).ok_or_else(|| {
                ApiError::new("level_api.invalid_profile", "Scene limits must be positive")
            })
        }
        Ok(SceneLimits {
            max_input_queue_bytes: limit(self.max_input_queue_bytes)?,
            max_total_input_bytes_per_test: limit(self.max_total_input_bytes_per_test)?,
            max_scene_state_units: limit(self.max_scene_state_units)?,
            max_scene_frames_per_test: limit(self.max_scene_frames_per_test)?,
            max_scene_work_per_call: limit(self.max_scene_work_per_call)?,
            max_total_scene_work: limit(self.max_total_scene_work)?,
            max_scene_events_per_evaluation: limit(self.max_scene_events_per_evaluation)?,
            max_scene_feedback_bytes: limit(self.max_scene_feedback_bytes)?,
        })
    }
}
