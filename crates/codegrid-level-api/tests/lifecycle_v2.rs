use codegrid_level_api::{LevelApi, LevelApiV2, SafetyProfile, SafetyProfileV2};
use serde_json::{json, Value};
fn api() -> LevelApiV2 {
    LevelApiV2::new(
        SafetyProfileV2::from_json(include_str!(
            "../../../examples/scene-host-v2/profile-local-v2.json"
        ))
        .unwrap(),
    )
    .unwrap()
}
fn request(api: &mut LevelApiV2, mut fields: Value) -> Value {
    fields["api_version"] = json!(2);
    serde_json::from_str(&api.request_json(&fields.to_string())).unwrap()
}
fn loaded(api: &mut LevelApiV2) -> (Value, Value) {
    let mut level: Value = serde_json::from_str(include_str!(
        "../../../examples/scene-level-v2/exactio.json"
    ))
    .unwrap();
    level["program_rules"]["allowed_instructions"] =
        json!(codegrid_level_core::instruction_identifiers());
    let level = request(
        api,
        json!({"operation":"load_level","level_json":level.to_string()}),
    );
    assert_eq!(level["status"], "ok", "{level}");
    let program = request(
        api,
        json!({"operation":"compile_program","source":include_str!("../../../fixtures/levels/echo.cg")}),
    );
    assert_eq!(program["status"], "ok", "{program}");
    (level["handle"].clone(), program["handle"].clone())
}
fn start(api: &mut LevelApiV2, level: Value, program: Value, mode: &str) -> Value {
    request(
        api,
        json!({"operation":"start_evaluation","level":level,"program":program,"mode":mode,"boundary_mode":"Exit","shuffle_seed":"18446744073709551615","custom_execution_limit":"100"}),
    )
}
#[test]
fn complete_debug_lifecycle_and_feedback_acknowledgement() {
    let mut api = api();
    let (level, program) = loaded(&mut api);
    let started = start(&mut api, level.clone(), program.clone(), "Debug");
    assert_eq!(started["status"], "ok", "{started}");
    let evaluation = started["handle"].clone();
    assert_eq!(
        request(
            &mut api,
            json!({"operation":"evaluation_result","evaluation":evaluation})
        )["status"],
        "pending"
    );
    let terminal = request(
        &mut api,
        json!({"operation":"advance_evaluation","evaluation":evaluation,"work_budget":"1000"}),
    );
    assert_eq!(terminal["result"]["status"], "Passed", "{terminal}");
    assert_eq!(
        terminal["result"]["configuration"]["shuffle_seed"],
        "18446744073709551615"
    );
    let page = request(
        &mut api,
        json!({"operation":"scene_feedback","evaluation_handle":evaluation,"after_sequence":"0","max_events":"100"}),
    );
    assert_eq!(page["status"], "ok", "{page}");
    assert_eq!(page["events"][0]["kind"], "CaseStarted");
    assert_eq!(
        page["events"].as_array().unwrap().last().unwrap()["kind"],
        "CaseEnded"
    );
    let replay = request(
        &mut api,
        json!({"operation":"scene_feedback","evaluation_handle":evaluation,"after_sequence":"0","max_events":"100"}),
    );
    assert_eq!(page, replay);
    let empty = request(
        &mut api,
        json!({"operation":"scene_feedback","evaluation_handle":evaluation,"after_sequence":page["next_sequence"],"max_events":"100"}),
    );
    assert_eq!(empty["events"], json!([]));
    let stale = request(
        &mut api,
        json!({"operation":"scene_feedback","evaluation_handle":evaluation,"after_sequence":"0","max_events":"100"}),
    );
    assert_eq!(stale["error"]["code"], "level_api.invalid_request");
    assert_eq!(
        terminal,
        request(
            &mut api,
            json!({"operation":"evaluation_result","evaluation":evaluation})
        )
    );
    for (kind, handle) in [
        ("evaluation", evaluation),
        ("program", program),
        ("level", level),
    ] {
        assert_eq!(
            request(
                &mut api,
                json!({"operation":"release","kind":kind,"handle":handle})
            )["status"],
            "ok"
        );
        assert_eq!(
            request(
                &mut api,
                json!({"operation":"release","kind":kind,"handle":handle})
            )["error"]["code"],
            "level_api.invalid_handle"
        );
    }
}
#[test]
fn official_feedback_is_rejected_and_cross_instance_handles_fail() {
    let mut a = api();
    let mut b = api();
    let (level, program) = loaded(&mut a);
    assert_eq!(
        start(&mut b, level.clone(), program.clone(), "Official")["error"]["code"],
        "level_api.invalid_handle"
    );
    let started = start(&mut a, level, program, "Official");
    let handle = started["handle"].clone();
    let feedback = request(
        &mut a,
        json!({"operation":"scene_feedback","evaluation_handle":handle,"after_sequence":"0","max_events":"1"}),
    );
    assert_eq!(feedback["error"]["code"], "level_api.invalid_configuration");
    let terminal = request(
        &mut a,
        json!({"operation":"advance_evaluation","evaluation":handle,"work_budget":"1000"}),
    );
    assert_eq!(terminal["result"]["status"], "Passed");
}
#[test]
fn version_separation_strict_fields_and_shutdown() {
    let mut a = api();
    let old = serde_json::from_str::<Value>(
        &a.request_json("{\"api_version\":1,\"operation\":\"capabilities\"}"),
    )
    .unwrap();
    assert_eq!(old["api_version"], 2);
    assert_eq!(old["error"]["code"], "level_api.unsupported_version");
    for text in ["{\"api_version\":2,\"api_version\":2,\"operation\":\"capabilities\"}",
        "{\"api_version\":2,\"operation\":\"capabilities\",\"extra\":0}",
        "{\"api_version\":2,\"operation\":\"scene_feedback\",\"evaluation_handle\":\"1\",\"after_sequence\":\"0\"}"] {
        let response: Value = serde_json::from_str(&a.request_json(text)).unwrap();
        assert_eq!(response["error"]["code"],"level_api.invalid_request");
    }
    let mut old = LevelApi::new(
        SafetyProfile::from_json(include_str!(
            "../../../fixtures/levels/profiles/local-v1.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let rejected: Value = serde_json::from_str(
        &old.request_json("{\"api_version\":2,\"operation\":\"capabilities\"}"),
    )
    .unwrap();
    assert_eq!(rejected["error"]["code"], "level_api.unsupported_version");
    assert_eq!(
        request(&mut a, json!({"operation":"shutdown"}))["status"],
        "ok"
    );
    assert_eq!(
        request(&mut a, json!({"operation":"capabilities"}))["error"]["code"],
        "level_api.shutdown"
    );
}
#[test]
fn release_recovers_reserved_scene_capacity() {
    let mut api = api();
    let (level, program) = loaded(&mut api);
    let first = start(&mut api, level.clone(), program.clone(), "Debug");
    assert_eq!(first["status"], "ok");
    let blocked = start(&mut api, level.clone(), program.clone(), "Debug");
    assert_eq!(blocked["error"]["code"], "level_api.resource_limit");
    assert_eq!(
        request(
            &mut api,
            json!({"operation":"release","kind":"evaluation","handle":first["handle"]})
        )["status"],
        "ok"
    );
    assert_eq!(start(&mut api, level, program, "Debug")["status"], "ok");
}

#[test]
fn start_refuses_when_feedback_budget_cannot_cover_result_baseline() {
    let mut profile: Value = serde_json::from_str(include_str!(
        "../../../examples/scene-host-v2/profile-local-v2.json"
    ))
    .unwrap();
    profile["max_response_bytes"] = json!("16384");
    let mut api =
        LevelApiV2::new(SafetyProfileV2::from_json(&profile.to_string()).unwrap()).unwrap();
    let (level, program) = loaded(&mut api);
    // The result baseline always exceeds the old hardcoded 16384 floor because
    // level_id and profile_id are non-empty; the refusal must surface at start.
    let started = start(&mut api, level, program, "Debug");
    assert_eq!(
        started["error"]["code"], "level_api.resource_limit",
        "{started}"
    );
}

#[test]
fn every_advertised_scene_executes_through_api_two() {
    let fixtures = [
        include_str!("../../../examples/scene-level-v2/exactio.json"),
        include_str!("../../../examples/scene-level-v2/robot.json"),
        include_str!("../../../examples/scene-level-v2/mechanical-arm.json"),
    ];
    for text in fixtures {
        let mut api = api();
        let capabilities = request(&mut api, json!({"operation":"capabilities"}));
        let mut value: Value = serde_json::from_str(text).unwrap();
        value["program_rules"]["allowed_instructions"] =
            json!(codegrid_level_core::instruction_identifiers());
        let scene = value["scene_type"].clone();
        assert!(capabilities["capabilities"]["scene_types"]
            .as_array()
            .unwrap()
            .contains(&scene));
        let level = request(
            &mut api,
            json!({"operation":"load_level","level_json":value.to_string()}),
        );
        assert_eq!(level["status"], "ok", "{scene}: {level}");
        let program = request(
            &mut api,
            json!({"operation":"compile_program","source":include_str!("../../../fixtures/levels/echo.cg")}),
        );
        let started = start(
            &mut api,
            level["handle"].clone(),
            program["handle"].clone(),
            "Debug",
        );
        assert_eq!(started["status"], "ok", "{scene}: {started}");
        let terminal = request(
            &mut api,
            json!({"operation":"advance_evaluation","evaluation":started["handle"],"work_budget":"1000"}),
        );
        assert_eq!(terminal["status"], "result", "{scene}: {terminal}");
        assert_eq!(terminal["result"]["scene_type"], scene);
        assert_eq!(
            terminal["result"]["visible_cases"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_ne!(terminal["result"]["status"], "ProgramRejected");
        assert_ne!(terminal["result"]["status"], "Fault");
    }
}
