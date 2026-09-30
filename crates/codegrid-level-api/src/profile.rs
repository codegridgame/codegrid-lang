use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
}
impl ApiError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Immutable trusted host ceilings; never loaded from a player's level or source.
#[derive(Clone, Debug, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SafetyProfile {
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
}

fn positive<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
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

impl SafetyProfile {
    pub fn from_json(json: &str) -> Result<Self, ApiError> {
        let profile: Self = serde_json::from_str(json)
            .map_err(|e| ApiError::new("level_api.invalid_profile", e.to_string()))?;
        profile.validate()?;
        Ok(profile)
    }
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.profile_version != 1 {
            return Err(ApiError::new(
                "level_api.unsupported_profile_version",
                "Unsupported safety profile version",
            ));
        }
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
    pub fn identity_json(&self) -> Value {
        serde_json::json!({"profile_id":self.profile_id,"profile_version":self.profile_version})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_integer_contract() {
        assert_eq!(parse_decimal("18446744073709551615"), Some(u64::MAX));
        for s in ["", "01", "+1", "-1", "1.0", "18446744073709551616"] {
            assert_eq!(parse_decimal(s), None);
        }
    }
    #[test]
    fn local_profile_and_duplicate_fields() {
        let json = include_str!("../../../fixtures/levels/profiles/local-v1.json");
        assert!(SafetyProfile::from_json(json).is_ok());
        assert!(
            SafetyProfile::from_json(&json.replacen("{", "{\"profile_version\":1,", 1)).is_err()
        );
        assert!(SafetyProfile::from_json(&json.replace("\"1048576\"", "\"0\"")).is_err());
    }
}
