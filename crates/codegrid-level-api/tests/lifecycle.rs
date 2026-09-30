use codegrid_level_api::{LevelApi, SafetyProfile};
use serde_json::{json, Value};
fn api() -> LevelApi {
    LevelApi::new(
        SafetyProfile::from_json(include_str!(
            "../../../fixtures/levels/profiles/local-v1.json"
        ))
        .unwrap(),
    )
    .unwrap()
}
fn request(api: &mut LevelApi, mut fields: Value) -> Value {
    fields["api_version"] = json!(1);
    serde_json::from_str(&api.request_json(&fields.to_string())).unwrap()
}
fn loaded(api: &mut LevelApi) -> (String, String) {
    let l = request(
        api,
        json!({"operation":"load_level","level_json":include_str!("../../../fixtures/levels/echo.json")}),
    );
    assert_eq!(l["status"], "ok", "{l}");
    let p = request(
        api,
        json!({"operation":"compile_program","source":include_str!("../../../fixtures/levels/echo.cg")}),
    );
    assert_eq!(p["status"], "ok", "{p}");
    (
        l["handle"].as_str().unwrap().into(),
        p["handle"].as_str().unwrap().into(),
    )
}
#[test]
fn result_lifecycle_exact_seed_and_scoring() {
    let mut api = api();
    let (l, p) = loaded(&mut api);
    let e = request(
        &mut api,
        json!({"operation":"start_evaluation","level":l,"program":p,"mode":"Official","boundary_mode":"Exit","shuffle_seed":"18446744073709551615","custom_execution_limit":"100"}),
    );
    assert_eq!(e["status"], "ok", "{e}");
    let h = e["handle"].clone();
    let pending = request(
        &mut api,
        json!({"operation":"evaluation_result","evaluation":h}),
    );
    assert_eq!(pending["status"], "pending");
    let terminal = loop {
        let r = request(
            &mut api,
            json!({"operation":"advance_evaluation","evaluation":h,"work_budget":"1"}),
        );
        if r["status"] != "pending" {
            break r;
        }
    };
    assert_eq!(terminal["result"]["status"], "Passed", "{terminal}");
    assert_eq!(
        terminal["result"]["configuration"]["shuffle_seed"],
        "18446744073709551615"
    );
    assert_eq!(terminal["result"]["final_metrics"]["ticks"], "6");
    assert_eq!(terminal["result"]["rating"], 3);
    assert_eq!(
        request(
            &mut api,
            json!({"operation":"evaluation_result","evaluation":h})
        ),
        terminal
    );
    assert_eq!(
        request(
            &mut api,
            json!({"operation":"release","kind":"evaluation","handle":h})
        )["status"],
        "ok"
    );
    assert_eq!(
        request(
            &mut api,
            json!({"operation":"evaluation_result","evaluation":h})
        )["error"]["code"],
        "level_api.invalid_handle"
    );
}
#[test]
fn isolation_versions_unknown_duplicates_and_shutdown() {
    let mut a = api();
    let (l, p) = loaded(&mut a);
    let mut b = api();
    assert_eq!(
        request(
            &mut b,
            json!({"operation":"start_evaluation","level":l,"program":p,"mode":"Official","boundary_mode":"Exit","shuffle_seed":"0","custom_execution_limit":"1"})
        )["error"]["code"],
        "level_api.invalid_handle"
    );
    for raw in [
        r#"{"api_version":1,"operation":"capabilities","unknown":0}"#,
        r#"{"api_version":1,"api_version":1,"operation":"capabilities"}"#,
    ] {
        let r: Value = serde_json::from_str(&a.request_json(raw)).unwrap();
        assert_eq!(r["status"], "error");
    }
    let r: Value =
        serde_json::from_str(&a.request_json(r#"{"api_version":99,"operation":"capabilities"}"#))
            .unwrap();
    assert_eq!(r["error"]["code"], "level_api.unsupported_version");
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
fn replay_hashes_and_execution_ceiling_identity_are_present() {
    let mut a = api();
    let (l, p) = loaded(&mut a);
    let e = request(
        &mut a,
        json!({"operation":"start_evaluation","level":l,"program":p,"mode":"Debug","boundary_mode":"Exit","shuffle_seed":"0","custom_execution_limit":"1"}),
    );
    let r = request(
        &mut a,
        json!({"operation":"advance_evaluation","evaluation":e["handle"],"work_budget":"1000"}),
    );
    for field in ["source_sha256", "level_sha256", "profile_sha256"] {
        let digest = r["result"]["replay"][field].as_str().unwrap();
        assert_eq!(digest.len(), 64);
        assert!(digest.bytes().all(|b| b.is_ascii_hexdigit()));
    }
    assert_eq!(
        r["result"]["configuration"]["safety"]["max_total_work"],
        "100000000"
    );
    let future: Value = serde_json::from_str(
        &a.request_json(r#"{"api_version":9,"operation":"future_operation","future":true}"#),
    )
    .unwrap();
    assert_eq!(future["error"]["code"], "level_api.unsupported_version");
    let mut profile = a.profile().clone();
    profile.max_total_work = u64::MAX;
    assert!(profile.validate().is_ok());
}
#[test]
fn rejection_seed_source_response_and_resource_limits() {
    let mut a = api();
    assert_eq!(
        request(
            &mut a,
            json!({"operation":"compile_program","source":"bad"})
        )["status"],
        "source_rejected"
    );
    assert_eq!(
        request(&mut a, json!({"operation":"load_level","level_json":"{}"}))["status"],
        "level_rejected"
    );
    let (l, p) = loaded(&mut a);
    let r = json!({"operation":"start_evaluation","level":l,"program":p,"mode":"Debug","boundary_mode":"Exit","custom_execution_limit":"1"});
    assert_eq!(
        request(&mut a, r.clone())["error"]["code"],
        "level_api.seed_required"
    );
    let mut a = a.with_seed_source(|| u64::MAX);
    assert_eq!(request(&mut a, r)["status"], "ok");
    let mut profile = a.profile().clone();
    profile.max_response_bytes = 512;
    let mut b = LevelApi::new(profile).unwrap();
    let (l, p) = loaded(&mut b);
    let e = request(
        &mut b,
        json!({"operation":"start_evaluation","level":l,"program":p,"mode":"Official","boundary_mode":"Exit","shuffle_seed":"0","custom_execution_limit":"1"}),
    );
    let r = request(
        &mut b,
        json!({"operation":"advance_evaluation","evaluation":e["handle"],"work_budget":"1000"}),
    );
    assert_eq!(r["error"]["code"], "level_api.response_too_large");
}

#[test]
fn trusted_load_source_handle_and_state_ceilings_are_host_errors() {
    let base = api().profile().clone();
    for kind in [
        "level_bytes",
        "test_count",
        "test_bytes",
        "source_bytes",
        "state",
    ] {
        let mut profile = base.clone();
        match kind {
            "level_bytes" => profile.max_level_bytes = 1,
            "test_count" => profile.max_tests = 1,
            "test_bytes" => profile.max_total_test_bytes = 1,
            "source_bytes" => profile.max_source_bytes = 1,
            _ => profile.max_state_bytes = 1,
        };
        let mut a = LevelApi::new(profile).unwrap();
        let r = if kind == "source_bytes" {
            request(
                &mut a,
                json!({"operation":"compile_program","source":include_str!("../../../fixtures/levels/echo.cg")}),
            )
        } else {
            request(
                &mut a,
                json!({"operation":"load_level","level_json":include_str!("../../../fixtures/levels/echo.json")}),
            )
        };
        assert_eq!(r["status"], "error", "{kind}: {r}");
        assert_eq!(
            r["error"]["code"], "level_api.resource_limit",
            "{kind}: {r}"
        );
    }
    let mut profile = base;
    profile.max_handles = 1;
    let mut a = LevelApi::new(profile).unwrap();
    assert_eq!(
        request(
            &mut a,
            json!({"operation":"compile_program","source":include_str!("../../../fixtures/levels/echo.cg")})
        )["status"],
        "ok"
    );
    assert_eq!(
        request(
            &mut a,
            json!({"operation":"compile_program","source":include_str!("../../../fixtures/levels/echo.cg")})
        )["error"]["code"],
        "level_api.resource_limit"
    );
}
#[test]
fn release_inputs_preserves_evaluation_and_release_pending_invalidates_it() {
    let mut a = api();
    let (l, p) = loaded(&mut a);
    let e=request(&mut a,json!({"operation":"start_evaluation","level":l,"program":p,"mode":"Official","boundary_mode":"Exit","shuffle_seed":"0","custom_execution_limit":"100"}))["handle"].clone();
    assert_eq!(
        request(
            &mut a,
            json!({"operation":"release","kind":"level","handle":l})
        )["status"],
        "ok"
    );
    assert_eq!(
        request(
            &mut a,
            json!({"operation":"release","kind":"program","handle":p})
        )["status"],
        "ok"
    );
    let r = request(
        &mut a,
        json!({"operation":"advance_evaluation","evaluation":e,"work_budget":"1000"}),
    );
    assert_eq!(r["result"]["status"], "Passed");
    let (l, p) = loaded(&mut a);
    let e=request(&mut a,json!({"operation":"start_evaluation","level":l,"program":p,"mode":"Official","boundary_mode":"Exit","shuffle_seed":"0","custom_execution_limit":"100"}))["handle"].clone();
    assert_eq!(
        request(
            &mut a,
            json!({"operation":"release","kind":"evaluation","handle":e})
        )["status"],
        "ok"
    );
    assert_eq!(
        request(
            &mut a,
            json!({"operation":"evaluation_result","evaluation":e})
        )["error"]["code"],
        "level_api.invalid_handle"
    );
}
#[test]
fn hidden_results_and_pending_projection_disclose_no_private_payload_or_metrics() {
    let mut a = api();
    let l=request(&mut a,json!({"operation":"load_level","level_json":include_str!("../../../fixtures/levels/hidden-wrong-official.json")}))["handle"].clone();
    let p=request(&mut a,json!({"operation":"compile_program","source":include_str!("../../../fixtures/levels/echo.cg")}))["handle"].clone();
    let e=request(&mut a,json!({"operation":"start_evaluation","level":l,"program":p,"mode":"Official","boundary_mode":"Exit","shuffle_seed":"0","custom_execution_limit":"100"}))["handle"].clone();
    let terminal = loop {
        let r = request(
            &mut a,
            json!({"operation":"advance_evaluation","evaluation":e,"work_budget":"1"}),
        );
        if r["status"] == "pending" {
            assert_eq!(r.as_object().unwrap().len(), 4);
            assert!(r.get("input").is_none());
        } else {
            break r;
        }
    };
    let result = &terminal["result"];
    assert_eq!(result["status"], "TestFailed");
    assert!(result["final_metrics"].is_null());
    assert!(result["rating"].is_null());
    assert_eq!(result["hidden_failure"]["category"], "HiddenTestFailed");
    for visible in result["visible_tests"].as_array().unwrap() {
        assert_eq!(visible["source_index"], "0");
        assert_eq!(visible["input"], json!([9]));
        assert_eq!(visible["expected_output"], json!([9]));
        assert_eq!(visible["actual_output"], json!([9]));
    }
    for forbidden in ["snapshot", "events", "trace", "order", "hidden_index"] {
        assert!(result.get(forbidden).is_none());
    }
    let cost = result["partial_metrics"]["cost"]
        .as_str()
        .unwrap_or("0")
        .parse::<u64>()
        .unwrap();
    assert!(cost <= 2);
}
#[test]
fn reservations_are_released_and_small_responses_stay_complete() {
    let mut profile = api().profile().clone();
    profile.max_state_bytes = 20000;
    let mut a = LevelApi::new(profile).unwrap();
    let (l, p) = loaded(&mut a);
    let start = json!({"operation":"start_evaluation","level":l,"program":p,"mode":"Official","boundary_mode":"Exit","shuffle_seed":"0","custom_execution_limit":"100"});
    let first = request(&mut a, start.clone());
    assert_eq!(first["status"], "ok", "{first}");
    let second = request(&mut a, start.clone());
    assert_eq!(second["error"]["code"], "level_api.resource_limit");
    assert_eq!(
        request(
            &mut a,
            json!({"operation":"release","kind":"evaluation","handle":first["handle"]})
        )["status"],
        "ok"
    );
    assert_eq!(request(&mut a, start)["status"], "ok");
    let mut profile = api().profile().clone();
    profile.max_response_bytes = 512;
    let mut a = LevelApi::new(profile).unwrap();
    let raw = a.request_json(
        &json!({"api_version":1,"operation":"compile_program","source":"bad\n".repeat(1000)})
            .to_string(),
    );
    assert!(raw.len() <= 512);
    let r: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(r["error"]["code"], "level_api.response_too_large");
}
