use codegrid_level_core::{load_scene_level_json, LoadLimits, SceneConfig};
use serde_json::{json, Value};

fn limits() -> LoadLimits {
    LoadLimits {
        max_level_bytes: 1_000_000,
        max_tests: 20,
        max_total_test_bytes: 1_000_000,
    }
}
fn load(
    value: &Value,
) -> Result<codegrid_level_core::ValidatedSceneLevel, codegrid_level_core::LevelError> {
    load_scene_level_json(value.to_string().as_bytes(), limits())
}
fn example(name: &str) -> Value {
    let source = match name {
        "exactio" => include_str!("../../../examples/scene-level-v2/exactio.json"),
        "robot" => include_str!("../../../examples/scene-level-v2/robot.json"),
        "mechanical-arm" => include_str!("../../../examples/scene-level-v2/mechanical-arm.json"),
        _ => panic!("internal fixture name"),
    };
    serde_json::from_str(source).unwrap()
}

#[test]
fn documented_author_examples_have_typed_definitions() {
    for name in ["exactio", "robot", "mechanical-arm"] {
        let v = example(name);
        let level = load(&v).unwrap();
        assert_eq!(level.kind().id(), v["scene_type"].as_str().unwrap());
        assert_eq!(level.tests().len(), 1);
    }
    let robot = load(&example("robot")).unwrap();
    let SceneConfig::Robot { map, .. } = robot.config() else {
        panic!("robot configuration")
    };
    assert_eq!(map.cells.len(), 256);
    assert!(map.cells[1].patrol);
    assert_eq!(map.starts[0].direction, 1);
    assert_eq!(map.cells[1].color, 0);
}

#[test]
fn documented_invalid_examples_return_exact_paths() {
    let cases = [
        (
            include_str!("../../../examples/scene-level-v2/exactio-invalid.json"),
            "UnknownField",
            "$.scene_config.unexpected",
        ),
        (
            include_str!("../../../examples/scene-level-v2/robot-invalid.json"),
            "DuplicateMapIndex",
            "$.scene_config.map.objects[2].index",
        ),
        (
            include_str!("../../../examples/scene-level-v2/mechanical-arm-invalid.json"),
            "InvalidRobotState",
            "$.evaluation.tests[0].expected_output[0]",
        ),
    ];
    for (source, reason, path) in cases {
        let error = load_scene_level_json(source.as_bytes(), limits()).unwrap_err();
        assert_eq!(error.category, "LevelInvalid");
        assert_eq!((error.reason, error.path.as_str()), (reason, path));
    }
}

#[test]
fn shared_envelope_validation_remains_strict_and_exact() {
    let mut v = example("exactio");
    v["constraints"]["max_ticks"] = json!(u64::MAX);
    assert_eq!(load(&v).unwrap().constraints()["max_ticks"], u64::MAX);
    v["evaluation_type"] = json!("Environment");
    assert_eq!(load(&v).unwrap_err().reason, "SceneFamilyMismatch");
    let source = example("exactio")
        .to_string()
        .replace("\"scene_config\":{}", "\"scene_config\":{\"x\":0,\"x\":1}");
    assert_eq!(
        load_scene_level_json(source.as_bytes(), limits())
            .unwrap_err()
            .reason,
        "DuplicateField"
    );
    let mut v = example("exactio");
    v["evaluation"]["tests"][0]["visible"] = json!(false);
    assert_eq!(load(&v).unwrap_err().reason, "VisibleTestRequired");
    v["format_version"] = json!(3);
    v["evaluation"] = json!({"future": []});
    assert_eq!(load(&v).unwrap_err().category, "UnsupportedFormatVersion");
    // Out-of-range and negative integers keep the v1 dedicated identity.
    let mut v = example("exactio");
    v["format_version"] = json!(18446744073709551616u128);
    assert_eq!(load(&v).unwrap_err().category, "UnsupportedFormatVersion");
    let mut v = example("exactio");
    v["format_version"] = json!(-1);
    assert_eq!(load(&v).unwrap_err().category, "UnsupportedFormatVersion");
    let original = example("exactio").to_string();
    assert_eq!(
        codegrid_level_core::load_level_json(original.as_bytes(), 100_000)
            .unwrap_err()
            .category,
        "LevelInvalid"
    );
}

#[test]
fn robot_trigger_door_pairing_uses_sparse_same_id_records_and_exact_paths() {
    let mut v = example("robot");
    v["scene_config"]["map"]["terrain"][0] = json!("0000............");
    v["scene_config"]["map"]["objects"]
        .as_array_mut()
        .unwrap()
        .extend([
            json!({"index":2,"type":"trigger","id":7}),
            json!({"index":3,"type":"door","id":7}),
        ]);
    load(&v).unwrap();

    v["scene_config"]["map"]["objects"][2]["id"] = json!(8);
    let error = load(&v).unwrap_err();
    assert_eq!(
        (error.reason, error.path.as_str()),
        ("InvalidDoorReference", "$.scene_config.map.objects[2].id")
    );

    v = example("robot");
    v["scene_config"]["map"]["terrain"][0] = json!("0000............");
    v["scene_config"]["map"]["objects"]
        .as_array_mut()
        .unwrap()
        .push(json!({"index":2,"type":"door","id":7}));
    let error = load(&v).unwrap_err();
    assert_eq!(
        (error.reason, error.path.as_str()),
        ("UnpairedDoor", "$.scene_config.map.objects[2].id")
    );

    v = example("robot");
    v["scene_config"]["map"]["terrain"][0] = json!("0000............");
    v["scene_config"]["map"]["objects"]
        .as_array_mut()
        .unwrap()
        .extend([
            json!({"index":2,"type":"trigger","id":7}),
            json!({"index":3,"type":"trigger","id":7}),
        ]);
    let error = load(&v).unwrap_err();
    assert_eq!(
        (error.reason, error.path.as_str()),
        ("DuplicateSceneId", "$.scene_config.map.objects[3].id")
    );
}

#[test]
fn robot_sparse_map_enforces_ground_colors_object_exclusivity_and_starts() {
    let mut v = example("robot");
    v["scene_config"]["map"]["terrain"][0] = json!("0000............");
    v["scene_config"]["map"]["colors"] = json!([
        {"index": 0, "color": 1},
        {"index": 1, "color": 2}
    ]);
    v["scene_config"]["map"]["objects"]
        .as_array_mut()
        .unwrap()
        .extend([
            json!({"index":2,"type":"trigger","id":9}),
            json!({"index":3,"type":"door","id":9}),
        ]);
    let level = load(&v).unwrap();
    let SceneConfig::Robot { map, .. } = level.config() else {
        panic!("robot configuration")
    };
    assert_eq!(map.cells[0].color, 1);
    assert_eq!(map.cells[1].color, 2);

    v["scene_config"]["map"]["objects"]
        .as_array_mut()
        .unwrap()
        .push(json!({"index":2,"type":"patrol","id":1}));
    let error = load(&v).unwrap_err();
    assert_eq!(
        (error.reason, error.path.as_str()),
        ("DuplicateMapIndex", "$.scene_config.map.objects[4].index")
    );

    v = example("robot");
    v["scene_config"]["map"]["objects"][1]["index"] = json!(2);
    let error = load(&v).unwrap_err();
    assert_eq!(
        (error.reason, error.path.as_str()),
        (
            "InvalidObjectPosition",
            "$.scene_config.map.objects[1].index"
        )
    );

    v = example("robot");
    v["scene_config"]["map"]["objects"]
        .as_array_mut()
        .unwrap()
        .push(json!({"index":0,"type":"patrol","id":1}));
    let error = load(&v).unwrap_err();
    assert_eq!(
        (error.reason, error.path.as_str()),
        ("DuplicateMapIndex", "$.scene_config.map.objects[2].index")
    );

    v = example("robot");
    v["scene_config"]["robot_count"] = json!(2);
    v["scene_config"]["map"]["terrain"][0] = json!("0000............");
    v["scene_config"]["map"]["objects"]
        .as_array_mut()
        .unwrap()
        .push(json!({"index":2,"type":"start","robot":"B","direction":"left"}));
    let level = load(&v).unwrap();
    let SceneConfig::Robot { map, .. } = level.config() else {
        panic!("robot configuration")
    };
    assert_eq!(map.starts.len(), 2);
    assert_eq!(map.starts[1].position, 2);
    assert_eq!(map.starts[1].direction, 3);

    v["scene_config"]["map"]["objects"][2]["robot"] = json!("A");
    let error = load(&v).unwrap_err();
    assert_eq!(
        (error.reason, error.path.as_str()),
        ("DuplicateRobotStart", "$.scene_config.map.objects[2].robot")
    );

    v = example("robot");
    v["evaluation"]["tests"][0]["map"] = json!([]);
    let error = load(&v).unwrap_err();
    assert_eq!(
        (error.reason, error.path.as_str()),
        ("UnknownField", "$.evaluation.tests[0].map")
    );
}

#[test]
fn robot_map_terrain_and_sparse_color_validation_is_strict() {
    let mut v = example("robot");
    v["scene_config"]["map"]["terrain"][0] = json!("0x..............");
    let error = load(&v).unwrap_err();
    assert_eq!(
        (error.reason, error.path.as_str()),
        ("InvalidTerrainCell", "$.scene_config.map.terrain[0]")
    );

    v = example("robot");
    v["scene_config"]["map"]["terrain"][0] = json!("0..............");
    let error = load(&v).unwrap_err();
    assert_eq!(
        (error.reason, error.path.as_str()),
        ("InvalidTerrainRow", "$.scene_config.map.terrain[0]")
    );

    v = example("robot");
    v["scene_config"]["map"]["colors"] = json!([
        {"index": 0, "color": 1},
        {"index": 0, "color": 2}
    ]);
    let error = load(&v).unwrap_err();
    assert_eq!(
        (error.reason, error.path.as_str()),
        ("DuplicateMapIndex", "$.scene_config.map.colors[1].index")
    );

    v = example("robot");
    v["scene_config"]["map"]["colors"] = json!([{"index": 2, "color": 1}]);
    let error = load(&v).unwrap_err();
    assert_eq!(
        (error.reason, error.path.as_str()),
        (
            "InvalidObjectPosition",
            "$.scene_config.map.colors[0].index"
        )
    );

    v = example("robot");
    v["scene_config"]["map"]["colors"] = json!([{"index": 0, "color": 3}]);
    let error = load(&v).unwrap_err();
    assert_eq!(
        (error.reason, error.path.as_str()),
        ("IntegerOutOfRange", "$.scene_config.map.colors[0].color")
    );
}

#[test]
fn mechanical_robot_state_domain_and_table_slots_are_finite() {
    let mut v = example("mechanical-arm");
    let mut accepted = 0;
    for byte in 0..=255 {
        v["evaluation"]["tests"][0]["expected_output"] = json!([byte]);
        let legal = byte & 192 == 0 && byte & 3 != 3 && (byte & 48 == 0 || (byte >> 2) & 3 == 0);
        assert_eq!(load(&v).is_ok(), legal, "state {byte}");
        accepted += usize::from(legal);
    }
    assert_eq!(accepted, 21);
    v["evaluation"]["tests"][0]["expected_output"] = json!([33]);
    v["scene_config"]["worktables"] = json!(["Inspection", "Inspection", null, "Removal"]);
    load(&v).unwrap();
    v["scene_config"]["worktables"] = json!([]);
    assert_eq!(load(&v).unwrap_err().reason, "InvalidWorktableSlots");
}

#[test]
fn collection_and_trusted_size_boundaries_are_enforced() {
    let source = example("robot").to_string();
    let mut limit = limits();
    limit.max_total_test_bytes = 10;
    assert_eq!(
        load_scene_level_json(source.as_bytes(), limit)
            .unwrap_err()
            .reason,
        "TestDataSizeExceeded"
    );
    let mut limit = limits();
    limit.max_tests = 0;
    assert_eq!(
        load_scene_level_json(source.as_bytes(), limit)
            .unwrap_err()
            .reason,
        "TestCountExceeded"
    );
}

#[test]
fn removed_scene_identifiers_and_metrics_are_rejected() {
    for scene in ["Baudot", "QualityControl", "Elevator"] {
        let mut v = example("exactio");
        v["scene_type"] = json!(scene);
        let error = load(&v).unwrap_err();
        assert_eq!(error.category, "UnsupportedSceneType");
        assert_eq!(error.path, "$.scene_type");
    }
    for name in ["exactio", "robot", "mechanical-arm"] {
        let mut v = example(name);
        v["constraints"]["max_stop_count"] = json!(1);
        assert_eq!(load(&v).unwrap_err().reason, "UnsupportedMetric");
        v["constraints"] = json!({});
        v["scoring"]["metrics"]["travel_distance"] = json!({"target":1});
        assert_eq!(load(&v).unwrap_err().reason, "UnsupportedMetric");
    }
}

#[test]
fn gas_and_size_are_current_author_contract_without_legacy_aliases() {
    let mut value = example("exactio");
    value["constraints"] = json!({"max_gas":u64::MAX,"max_size":0});
    value["scoring"]["metrics"] = json!({"gas_used":{"target":u64::MAX},"size":{"target":0}});
    assert!(load(&value).is_ok());
    for old in ["max_cost", "max_non_empty_cells"] {
        let mut invalid = value.clone();
        invalid["constraints"][old] = json!(1);
        assert!(load(&invalid).is_err());
    }
    for invalid_name in [
        "cost",
        "non_empty_cells",
        "execution_gas",
        "memory_gas",
        "stack_gas",
    ] {
        let mut invalid = value.clone();
        invalid["scoring"]["metrics"][invalid_name] = json!({"target":1});
        assert!(load(&invalid).is_err());
    }
}
