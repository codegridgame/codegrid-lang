//! Worker-compatible browser binding over the shared level API.
use codegrid_level_api::error_number;
use codegrid_level_api::{ApiError, LevelApi, LevelApiV2, SafetyProfile, SafetyProfileV2};
use wasm_bindgen::prelude::*;
const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;
fn error(code: &str, message: &str) -> String {
    serde_json::json!({"api_version":1,"status":"error","error":{"code":code,"error_number":error_number("level",code),"message":message}})
        .to_string()
}
#[wasm_bindgen]
pub struct LevelSession {
    api: LevelApi,
}
impl LevelSession {
    pub fn from_profile_json(profile: &str) -> Result<Self, ApiError> {
        if profile.len() > MAX_TEXT_BYTES {
            return Err(ApiError {
                code: "level_api.resource_limit",
                message: "Profile exceeds adapter byte ceiling".into(),
            });
        }
        Ok(Self {
            api: LevelApi::new(SafetyProfile::from_json(profile)?)?,
        })
    }
    pub fn request_text(&mut self, request: &str) -> String {
        if request.len() > MAX_TEXT_BYTES {
            error(
                "level_api.resource_limit",
                "Request exceeds adapter byte ceiling",
            )
        } else {
            self.api.request_json(request)
        }
    }
    pub fn shutdown_text(&mut self) -> String {
        self.api
            .request_json(r#"{"api_version":1,"operation":"shutdown"}"#)
    }
}
#[wasm_bindgen]
impl LevelSession {
    #[wasm_bindgen(constructor)]
    pub fn new(profile: JsValue) -> Result<LevelSession, JsValue> {
        let profile = bounded_js_string(profile).map_err(|_| {
            JsValue::from_str(&error(
                "level_api.invalid_profile",
                "Expected a profile string within the adapter byte ceiling",
            ))
        })?;
        Self::from_profile_json(&profile).map_err(|e| JsValue::from_str(&error(e.code, &e.message)))
    }
    pub fn request(&mut self, request: JsValue) -> String {
        match bounded_js_string(request) {
            Ok(s) => self.request_text(&s),
            Err(_) => error(
                "level_api.invalid_request",
                "Expected a string within the adapter byte ceiling",
            ),
        }
    }
    pub fn shutdown(&mut self) -> String {
        self.shutdown_text()
    }
}
fn scene_error(code: &str, message: &str) -> String {
    serde_json::json!({"api_version":2,"status":"error","error":{"code":code,"error_number":error_number("level",code),"message":message}}).to_string()
}
#[wasm_bindgen]
pub struct SceneLevelSession {
    api: LevelApiV2,
}
impl SceneLevelSession {
    pub fn from_profile_json(profile: &str) -> Result<Self, ApiError> {
        if profile.len() > MAX_TEXT_BYTES {
            return Err(ApiError {
                code: "level_api.resource_limit",
                message: "Profile exceeds adapter byte ceiling".into(),
            });
        }
        Ok(Self {
            api: LevelApiV2::new(SafetyProfileV2::from_json(profile)?)?,
        })
    }
    pub fn request_text(&mut self, request: &str) -> String {
        if request.len() > MAX_TEXT_BYTES {
            scene_error(
                "level_api.resource_limit",
                "Request exceeds adapter byte ceiling",
            )
        } else {
            self.api.request_json(request)
        }
    }
    pub fn shutdown_text(&mut self) -> String {
        self.api
            .request_json(r#"{"api_version":2,"operation":"shutdown"}"#)
    }
}
#[wasm_bindgen]
impl SceneLevelSession {
    #[wasm_bindgen(constructor)]
    pub fn new(profile: JsValue) -> Result<SceneLevelSession, JsValue> {
        let profile = bounded_js_string(profile).map_err(|_| {
            JsValue::from_str(&scene_error(
                "level_api.invalid_profile",
                "Expected a profile string within the adapter byte ceiling",
            ))
        })?;
        Self::from_profile_json(&profile)
            .map_err(|e| JsValue::from_str(&scene_error(e.code, &e.message)))
    }
    pub fn request(&mut self, request: JsValue) -> String {
        match bounded_js_string(request) {
            Ok(s) => self.request_text(&s),
            Err(_) => scene_error(
                "level_api.invalid_request",
                "Expected a string within the adapter byte ceiling",
            ),
        }
    }
    pub fn shutdown(&mut self) -> String {
        self.shutdown_text()
    }
}
fn bounded_js_string(value: JsValue) -> Result<String, JsValue> {
    if !value.is_string() {
        return Err(JsValue::from_str("Expected a JSON string"));
    }
    let string = js_sys::JsString::from(value);
    // Count encoded bytes without first allocating the Rust UTF-8 copy.
    let mut bytes = 0usize;
    let mut i = 0;
    let length = string.length();
    while i < length {
        let unit = string.char_code_at(i) as u32;
        let width = if unit <= 0x7f {
            1
        } else if unit <= 0x7ff {
            2
        } else if (0xd800..=0xdbff).contains(&unit)
            && i + 1 < length
            && (0xdc00..=0xdfff).contains(&(string.char_code_at(i + 1) as u32))
        {
            i += 1;
            4
        } else {
            3
        };
        bytes = bytes
            .checked_add(width)
            .ok_or_else(|| JsValue::from_str("String exceeds byte ceiling"))?;
        if bytes > MAX_TEXT_BYTES {
            return Err(JsValue::from_str("String exceeds byte ceiling"));
        }
        i += 1;
    }
    Ok(String::from(string))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lifecycle_and_original_profile_validation() {
        let profile = include_str!("../../../fixtures/levels/profiles/local-v1.json");
        let mut session = LevelSession::from_profile_json(profile).unwrap();
        let response: serde_json::Value = serde_json::from_str(
            &session.request_text(r#"{"api_version":1,"operation":"capabilities"}"#),
        )
        .unwrap();
        assert_eq!(response["status"], "ok");
        session.shutdown_text();
        let response: serde_json::Value = serde_json::from_str(
            &session.request_text(r#"{"api_version":1,"operation":"capabilities"}"#),
        )
        .unwrap();
        assert_eq!(response["error"]["code"], "level_api.shutdown");
        assert!(LevelSession::from_profile_json(&profile.replacen(
            "{",
            "{\"profile_version\":1,",
            1
        ))
        .is_err());
    }
}

#[cfg(test)]
mod scene_tests {
    use super::*;
    #[test]
    fn scene_binding_has_independent_version_and_shutdown() {
        let profile = include_str!("../../../examples/scene-host-v2/profile-local-v2.json");
        let mut session = SceneLevelSession::from_profile_json(profile).unwrap();
        let capability: serde_json::Value = serde_json::from_str(
            &session.request_text(r#"{"api_version":2,"operation":"capabilities"}"#),
        )
        .unwrap();
        assert_eq!(capability["api_version"], 2);
        assert_eq!(
            capability["capabilities"]["scene_types"]
                .as_array()
                .unwrap()
                .len(),
            6
        );
        assert!(session
            .request_text(r#"{"api_version":1,"operation":"capabilities"}"#)
            .contains("unsupported_version"));
        session.shutdown_text();
        assert!(session
            .request_text(r#"{"api_version":2,"operation":"capabilities"}"#)
            .contains("level_api.shutdown"));
        assert!(LevelSession::from_profile_json(profile).is_err());
    }
}
