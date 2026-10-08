use serde::Deserialize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
}
impl ApiError {
    pub fn error_number(&self) -> Option<&'static str> {
        codegrid_model::error_number("level", self.code)
    }

    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Immutable trusted host ceilings; never loaded from a player's level or source.
#[derive(Clone, Debug)]
pub(crate) struct CommonLimits {
    pub profile_id: String,
    pub provenance: String,
    pub max_level_bytes: u64,
    pub max_source_bytes: u64,
    pub max_tests: u64,
    pub max_total_test_bytes: u64,
    pub max_program_cells: u64,
    pub max_program_boards: u64,
    pub max_state_bytes: u64,
    pub max_state_units: u64,
    pub max_output_bytes: u64,
    pub max_handles: u64,
    pub max_response_bytes: u64,
    pub max_ticks_per_test: u64,
    pub max_work_per_call: u64,
    pub max_total_work: u64,
}

pub(crate) fn positive<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    let text = String::deserialize(d)?;
    parse_decimal(&text)
        .filter(|n| *n > 0)
        .ok_or_else(|| serde::de::Error::custom("Expected a positive canonical u64 decimal string"))
}
pub(crate) fn parse_decimal(text: &str) -> Option<u64> {
    if text.is_empty()
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    text.parse().ok()
}

impl CommonLimits {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.profile_id.is_empty()
            || self.profile_id.trim() != self.profile_id
            || self.provenance.is_empty()
        {
            return Err(ApiError::new(
                "level_api.invalid_profile",
                "Profile identity and provenance are required",
            ));
        }
        let limits = [
            self.max_level_bytes,
            self.max_source_bytes,
            self.max_tests,
            self.max_total_test_bytes,
            self.max_program_cells,
            self.max_program_boards,
            self.max_state_bytes,
            self.max_state_units,
            self.max_output_bytes,
            self.max_handles,
            self.max_response_bytes,
            self.max_ticks_per_test,
            self.max_work_per_call,
            self.max_total_work,
        ];
        if limits.contains(&0)
            || self.max_response_bytes < 512
            || [
                self.max_level_bytes,
                self.max_tests,
                self.max_total_test_bytes,
            ]
            .iter()
            .any(|n| usize::try_from(*n).is_err())
        {
            return Err(ApiError::new(
                "level_api.invalid_profile",
                "Limits must be positive, fit the host, and allow a bounded error response",
            ));
        }
        Ok(())
    }
}
