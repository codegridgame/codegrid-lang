use codegrid_level_api::{SafetyProfile, SafetyProfileV2};
use serde_json::{json, Value};

const PROFILE: &str = include_str!("../../../examples/scene-host-v2/profile-local-v2.json");
const SCENE_FIELDS: &[&str] = &[
    "max_input_queue_bytes",
    "max_total_input_bytes_per_test",
    "max_scene_state_units",
    "max_scene_frames_per_test",
    "max_scene_work_per_call",
    "max_total_scene_work",
    "max_scene_events_per_evaluation",
    "max_scene_feedback_bytes",
];

#[test]
fn version_paths_remain_explicit() {
    let profile = SafetyProfileV2::from_json(PROFILE).unwrap();
    assert_eq!(profile.identity_json()["profile_version"], 2);
    assert_eq!(
        profile
            .scene_limits
            .to_core()
            .unwrap()
            .max_input_queue_bytes
            .get(),
        65536
    );
    assert!(SafetyProfile::from_json(PROFILE).is_err());
    let old = include_str!("../../../fixtures/levels/profiles/local-v1.json");
    assert!(SafetyProfile::from_json(old).is_ok());
    assert!(SafetyProfileV2::from_json(old).is_err());
    let mut value: Value = serde_json::from_str(PROFILE).unwrap();
    value["profile_version"] = json!(1);
    assert_eq!(
        SafetyProfileV2::from_json(&value.to_string())
            .unwrap_err()
            .code,
        "level_api.unsupported_profile_version"
    );
}

#[test]
fn every_scene_limit_is_required_positive_and_exact() {
    for field in SCENE_FIELDS {
        let base: Value = serde_json::from_str(PROFILE).unwrap();
        for invalid in [
            json!("0"),
            json!("01"),
            json!("+1"),
            json!("1.0"),
            json!("18446744073709551616"),
            json!(1),
            Value::Null,
        ] {
            let mut value = base.clone();
            value["scene_limits"][field] = invalid;
            assert!(
                SafetyProfileV2::from_json(&value.to_string()).is_err(),
                "{field}"
            );
        }
        let mut missing = base.clone();
        missing["scene_limits"]
            .as_object_mut()
            .unwrap()
            .remove(*field);
        assert!(
            SafetyProfileV2::from_json(&missing.to_string()).is_err(),
            "{field}"
        );
        let mut exact = base;
        exact["scene_limits"][field] = json!("18446744073709551615");
        assert!(
            SafetyProfileV2::from_json(&exact.to_string()).is_ok(),
            "{field}"
        );
    }
}

#[test]
fn unknown_and_duplicate_fields_are_rejected_at_both_depths() {
    for value in [
        PROFILE.replacen("{", "{\"profile_version\":2,", 1),
        PROFILE.replace(
            "\"scene_limits\": {",
            "\"scene_limits\": {\"max_scene_state_units\":\"1\",",
        ),
        PROFILE.replacen("{", "{\"unexpected\":true,", 1),
        PROFILE.replace(
            "\"scene_limits\": {",
            "\"scene_limits\": {\"unexpected\":true,",
        ),
    ] {
        assert!(SafetyProfileV2::from_json(&value).is_err());
    }
}

#[test]
fn trusted_rust_configuration_cannot_bypass_validation() {
    let mut profile = SafetyProfileV2::from_json(PROFILE).unwrap();
    profile.scene_limits.max_total_scene_work = 0;
    assert!(profile.validate().is_err());
    assert!(profile.scene_limits.to_core().is_err());
    profile.scene_limits.max_total_scene_work = 1;
    profile.max_response_bytes = 511;
    assert!(profile.validate().is_err());
}
