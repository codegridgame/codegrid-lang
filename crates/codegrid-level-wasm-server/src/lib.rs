//! No-import portable level transport with exact live buffer ownership.
use codegrid_level_api::error_number;
use codegrid_level_api::{LevelApi, LevelApiV2, SafetyProfile, SafetyProfileV2};
use serde::Deserialize;
use serde_json::json;
#[cfg(any(target_arch = "wasm32", test))]
use std::collections::BTreeMap;
pub const LEVEL_ABI_VERSION: u32 = 1;
pub const SCENE_LEVEL_ABI_VERSION: u32 = 2;
const MAX_BUFFER_BYTES: usize = 8 * 1024 * 1024;
#[cfg(any(target_arch = "wasm32", test))]
const MAX_RETAINED_BYTES: usize = 24 * 1024 * 1024;
#[cfg(any(target_arch = "wasm32", test))]
const MAX_BUFFER_COUNT: usize = 1024;
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    Initialize {
        abi_version: u32,
        api_version: u32,
        profile_json: String,
    },
    Request {
        abi_version: u32,
        api_version: u32,
        request_json: String,
    },
    Shutdown {
        abi_version: u32,
        api_version: u32,
    },
}
enum VersionedApi {
    Legacy(LevelApi),
    Scene(LevelApiV2),
}
impl VersionedApi {
    fn version(&self) -> u32 {
        match self {
            Self::Legacy(_) => 1,
            Self::Scene(_) => 2,
        }
    }
    fn request(&mut self, text: &str) -> String {
        match self {
            Self::Legacy(api) => api.request_json(text),
            Self::Scene(api) => api.request_json(text),
        }
    }
}
#[derive(Default)]
pub struct PortableSession {
    api: Option<VersionedApi>,
    closed: bool,
    version: Option<u32>,
}
impl PortableSession {
    fn transport_error(&self, code: &str, message: &str) -> String {
        versioned_error(self.version.unwrap_or(1), code, message)
    }
    pub fn request_text(&mut self, text: &str) -> String {
        if text.len() > MAX_BUFFER_BYTES {
            return self.transport_error(
                "level_api.resource_limit",
                "Transport request exceeds byte ceiling",
            );
        }
        let operation = match serde_json::from_str::<Operation>(text) {
            Ok(operation) => operation,
            Err(_) => {
                return self
                    .transport_error("level_api.invalid_request", "Malformed portable request")
            }
        };
        let (abi, api) = match &operation {
            Operation::Initialize {
                abi_version,
                api_version,
                ..
            }
            | Operation::Request {
                abi_version,
                api_version,
                ..
            }
            | Operation::Shutdown {
                abi_version,
                api_version,
            } => (*abi_version, *api_version),
        };
        if !(1..=2).contains(&abi) || (abi == 2 && api != 2) {
            return self.transport_error(
                "level_abi.unsupported_version",
                "Unsupported level ABI version",
            );
        }
        if api != abi {
            return self.transport_error(
                "level_api.unsupported_version",
                "Unsupported level API version",
            );
        }
        if self.version.is_some_and(|version| version != abi) {
            return self.transport_error(
                "level_abi.unsupported_version",
                "Session transport version is immutable",
            );
        }
        if self.closed {
            return self.transport_error("level_api.shutdown", "Session is shut down");
        }
        let response = match operation {
            Operation::Initialize { profile_json, .. } => {
                if self.api.is_some() {
                    return self.transport_error(
                        "level_abi.already_initialized",
                        "Trusted profile is immutable",
                    );
                }
                let initialized = if abi == 1 {
                    SafetyProfile::from_json(&profile_json)
                        .and_then(LevelApi::new)
                        .map(VersionedApi::Legacy)
                } else {
                    SafetyProfileV2::from_json(&profile_json)
                        .and_then(|profile| {
                            if profile.max_response_bytes > (MAX_BUFFER_BYTES - 16) as u64 {
                                return Err(codegrid_level_api::ApiError {
                                    code: "level_api.invalid_profile",
                                    message: "Response ceiling exceeds portable transport capacity"
                                        .into(),
                                });
                            }
                            LevelApiV2::new(profile)
                        })
                        .map(VersionedApi::Scene)
                };
                match initialized {
                    Ok(api) => {
                        self.version = Some(api.version());
                        self.api = Some(api);
                        json!({"api_version":abi,"status":"ok"}).to_string()
                    }
                    Err(error) => return versioned_error(abi, error.code, &error.message),
                }
            }
            Operation::Request { request_json, .. } => match self.api.as_mut() {
                Some(api) => api.request(&request_json),
                None => {
                    return versioned_error(
                        abi,
                        "level_abi.not_initialized",
                        "Initialize the trusted session first",
                    )
                }
            },
            Operation::Shutdown { .. } => {
                self.version = Some(abi);
                self.closed = true;
                self.api = None;
                json!({"api_version":abi,"status":"ok"}).to_string()
            }
        };
        if response.len().saturating_add(16) > MAX_BUFFER_BYTES {
            return self.transport_error(
                "level_api.response_too_large",
                "Response exceeds adapter byte ceiling",
            );
        }
        if let Some(rest) = response.strip_prefix('{') {
            format!("{{\"abi_version\":{abi},{rest}")
        } else {
            self.transport_error("level_api.fault", "Shared API returned invalid response")
        }
    }
}
fn versioned_error(version: u32, code: &str, message: &str) -> String {
    json!({"abi_version":version,"api_version":version,"status":"error","error":{"code":code,"error_number":error_number("level",code),"message":message}}).to_string()
}

#[cfg(any(target_arch = "wasm32", test))]
struct Buffer {
    bytes: Box<[u8]>,
    length: usize,
    request: bool,
}
#[derive(Default)]
#[cfg(any(target_arch = "wasm32", test))]
pub struct Buffers {
    items: BTreeMap<usize, Buffer>,
    retained: usize,
}
#[cfg(any(target_arch = "wasm32", test))]
impl Buffers {
    fn allocate(&mut self, length: usize, request: bool) -> Option<usize> {
        if length == 0
            || length > MAX_BUFFER_BYTES
            || self.items.len() >= MAX_BUFFER_COUNT
            || self.retained.checked_add(length)? > MAX_RETAINED_BYTES
        {
            return None;
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).ok()?;
        bytes.resize(length, 0);
        let bytes = bytes.into_boxed_slice();
        let pointer = bytes.as_ptr() as usize;
        if pointer == 0 || self.items.contains_key(&pointer) {
            return None;
        }
        self.retained += length;
        self.items.insert(
            pointer,
            Buffer {
                bytes,
                length,
                request,
            },
        );
        Some(pointer)
    }
    fn release(&mut self, pointer: usize, length: usize) -> bool {
        if !self.items.get(&pointer).is_some_and(|b| b.length == length) {
            return false;
        }
        let buffer = self.items.remove(&pointer).expect("checked live buffer");
        self.retained -= buffer.bytes.len();
        true
    }
    fn input(&self, pointer: usize, length: usize) -> Option<&[u8]> {
        let b = self.items.get(&pointer)?;
        if b.request && b.length == length {
            Some(&b.bytes[..length])
        } else {
            None
        }
    }
    fn response(&mut self, pointer: usize, response: &[u8]) -> Option<usize> {
        let b = self.items.get_mut(&pointer)?;
        if b.request || response.len() > b.bytes.len() || response.is_empty() {
            return None;
        }
        b.bytes[..response.len()].copy_from_slice(response);
        b.length = response.len();
        Some(b.length)
    }
}
#[cfg(target_arch = "wasm32")]
thread_local! {static BUFFERS:std::cell::RefCell<Buffers>=std::cell::RefCell::new(Buffers::default());static SESSION:std::cell::RefCell<PortableSession>=std::cell::RefCell::new(PortableSession::default());}
#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn level_abi_version() -> u32 {
    LEVEL_ABI_VERSION
}
#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn level_abi_version_v2() -> u32 {
    SCENE_LEVEL_ABI_VERSION
}
#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn level_alloc(length: u32) -> u32 {
    BUFFERS.with(|b| {
        b.borrow_mut()
            .allocate(length as usize, true)
            .and_then(|p| u32::try_from(p).ok())
            .unwrap_or(0)
    })
}
#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn level_dealloc(pointer: u32, length: u32) -> u32 {
    u32::from(BUFFERS.with(|b| b.borrow_mut().release(pointer as usize, length as usize)))
}
#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn level_request(pointer: u32, length: u32) -> u64 {
    let valid = BUFFERS.with(|b| {
        b.borrow()
            .input(pointer as usize, length as usize)
            .is_some()
    });
    if !valid {
        return 0;
    }
    // Reserve before semantic mutation so allocation failure can be retried safely.
    let Some(response_pointer) = BUFFERS.with(|b| b.borrow_mut().allocate(MAX_BUFFER_BYTES, false))
    else {
        return 0;
    };
    let response = BUFFERS.with(|b| {
        let buffers = b.borrow();
        let input = buffers.input(pointer as usize, length as usize)?;
        Some(match std::str::from_utf8(input) {
            Ok(text) => SESSION.with(|s| s.borrow_mut().request_text(text)),
            Err(_) => SESSION.with(|s| {
                s.borrow()
                    .transport_error("level_api.invalid_request", "Request is not valid UTF-8")
            }),
        })
    });
    let Some(response) = response else {
        BUFFERS.with(|b| b.borrow_mut().release(response_pointer, MAX_BUFFER_BYTES));
        return 0;
    };
    let result = BUFFERS.with(|b| {
        b.borrow_mut()
            .response(response_pointer, response.as_bytes())
    });
    if let Some(length) = result {
        (response_pointer as u64) << 32 | length as u64
    } else {
        BUFFERS.with(|b| b.borrow_mut().release(response_pointer, MAX_BUFFER_BYTES));
        0
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_live_pairs_bounds_and_response_ownership() {
        let mut b = Buffers::default();
        assert!(b.allocate(0, true).is_none());
        assert!(b.allocate(MAX_BUFFER_BYTES + 1, true).is_none());
        let p = b.allocate(8, true).unwrap();
        assert!(b.input(p, 7).is_none());
        assert!(!b.release(p, 7));
        assert!(b.release(p, 8));
        assert!(!b.release(p, 8));
        let p = b.allocate(100, false).unwrap();
        assert_eq!(b.response(p, b"{}"), Some(2));
        assert!(b.input(p, 2).is_none());
        assert!(!b.release(p, 100));
        assert!(b.release(p, 2));
        assert_eq!(b.retained, 0);
    }
    #[test]
    fn lifecycle_version_and_profile_duplicates() {
        let mut s = PortableSession::default();
        let request = json!({"abi_version":1,"api_version":1,"operation":"request","request_json":"{\"api_version\":1,\"operation\":\"capabilities\"}"});
        assert!(s
            .request_text(&request.to_string())
            .contains("not_initialized"));
        let profile = include_str!("../../../fixtures/levels/profiles/local-v1.json");
        let initialize = json!({"abi_version":1,"api_version":1,"operation":"initialize","profile_json":profile});
        assert!(s.request_text(&initialize.to_string()).contains("\"ok\""));
        assert!(s
            .request_text(&initialize.to_string())
            .contains("already_initialized"));
        assert!(s.request_text(&request.to_string()).contains("ExactIO"));
        assert!(s
            .request_text(r#"{"abi_version":2,"api_version":1,"operation":"shutdown"}"#)
            .contains("unsupported_version"));
        s.request_text(r#"{"abi_version":1,"api_version":1,"operation":"shutdown"}"#);
        assert!(s
            .request_text(&initialize.to_string())
            .contains("level_api.shutdown"));
    }
}
#[cfg(test)]
mod strict_tests {
    use super::*;
    #[test]
    fn all_transport_operations_reject_unknown_duplicate_and_missing_fields() {
        for operation in ["initialize", "request", "shutdown"] {
            let extra = match operation {
                "initialize" => ",\"profile_json\":\"{}\"",
                "request" => ",\"request_json\":\"{}\"",
                _ => "",
            };
            for text in [format!("{{\"abi_version\":1,\"api_version\":1,\"operation\":\"{operation}\"{extra},\"extra\":true}}"),format!("{{\"abi_version\":1,\"abi_version\":1,\"api_version\":1,\"operation\":\"{operation}\"{extra}}}"),format!("{{\"abi_version\":1,\"operation\":\"{operation}\"{extra}}}")] {
    assert!(PortableSession::default().request_text(&text).contains("level_api.invalid_request"),"{text}");
   }
        }
    }
}

#[cfg(test)]
mod scene_transport_tests {
    use super::*;
    #[test]
    fn explicit_scene_version_and_immutable_session() {
        let mut session = PortableSession::default();
        let profile = include_str!("../../../examples/scene-host-v2/profile-local-v2.json");
        let initialized: serde_json::Value = serde_json::from_str(&session.request_text(&json!({"abi_version":2,"api_version":2,"operation":"initialize","profile_json":profile}).to_string())).unwrap();
        assert_eq!(initialized["abi_version"], 2);
        assert_eq!(initialized["status"], "ok");
        let capability: serde_json::Value = serde_json::from_str(&session.request_text(&json!({"abi_version":2,"api_version":2,"operation":"request","request_json":"{\"api_version\":2,\"operation\":\"capabilities\"}"}).to_string())).unwrap();
        assert_eq!(
            capability["capabilities"]["scene_types"]
                .as_array()
                .unwrap()
                .len(),
            6
        );
        assert!(session
            .request_text(r#"{"abi_version":1,"api_version":1,"operation":"shutdown"}"#)
            .contains("unsupported_version"));
        assert!(session
            .request_text(r#"{"abi_version":2,"api_version":2,"operation":"shutdown"}"#)
            .contains("\"ok\""));
        assert!(session
            .request_text(r#"{"abi_version":2,"api_version":2,"operation":"shutdown"}"#)
            .contains("level_api.shutdown"));
    }
    #[test]
    fn profile_cannot_commit_feedback_beyond_transport_response_capacity() {
        let mut profile: serde_json::Value = serde_json::from_str(include_str!(
            "../../../examples/scene-host-v2/profile-local-v2.json"
        ))
        .unwrap();
        profile["max_response_bytes"] = json!((MAX_BUFFER_BYTES as u64).to_string());
        let response = PortableSession::default().request_text(&json!({"abi_version":2,"api_version":2,"operation":"initialize","profile_json":profile.to_string()}).to_string());
        assert!(response.contains("level_api.invalid_profile"));
    }
    #[test]
    fn version_two_preserves_original_profile_duplicate_detection() {
        let profile = include_str!("../../../examples/scene-host-v2/profile-local-v2.json");
        let bad = profile.replacen("{", "{\"profile_version\":2,", 1);
        let response = PortableSession::default().request_text(
            &json!({"abi_version":2,"api_version":2,"operation":"initialize","profile_json":bad})
                .to_string(),
        );
        assert!(response.contains("level_api.invalid_profile"));
        assert!(response.contains("\"abi_version\":2"));
    }
}
