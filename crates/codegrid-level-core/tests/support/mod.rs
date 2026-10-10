#![allow(dead_code)]
use codegrid_ir::{Board, Cell, Program, ScopedProgram, VerifiedProgram, IR_FORMAT_VERSION};
use codegrid_level_core::{scene_session::*, *};
use codegrid_model::{Direction, PrimaryInstruction};
use serde_json::{json, Value};
use std::{collections::BTreeMap, num::NonZeroU64};
pub fn n(v: u64) -> NonZeroU64 {
    NonZeroU64::new(v).unwrap()
}
pub fn config() -> EvaluationConfig {
    EvaluationConfig {
        shuffle_seed: 42,
        custom_execution_limit: n(100),
        safety: ExecutionSafetyProfile {
            id: "test-v2".into(),
            version: 2,
            max_output_bytes: n(1000),
            max_state_units: n(100_000),
            max_feedback_bytes: n(1_000_000),
            per_test_ticks: n(1000),
            cumulative_work: n(10_000),
            per_call_work: n(1000),
        },
    }
}
pub fn limits() -> SceneLimits {
    SceneLimits {
        max_input_queue_bytes: n(1000),
        max_total_input_bytes_per_test: n(10_000),
        max_scene_state_units: n(100_000),
        max_scene_frames_per_test: n(1000),
        max_scene_work_per_call: n(100_000),
        max_total_scene_work: n(1_000_000),
        max_scene_events_per_evaluation: n(1000),
        max_scene_feedback_bytes: n(1_000_000),
    }
}
pub fn author(name: &str) -> Value {
    let mut v: Value = serde_json::from_str(match name {
        "robot" => include_str!("../../../../examples/scene-level-v2/robot.json"),
        "arm" => include_str!("../../../../examples/scene-level-v2/mechanical-arm.json"),
        "exact" => include_str!("../../../../examples/scene-level-v2/exactio.json"),
        _ => panic!("fixture name"),
    })
    .unwrap();
    v["program_rules"]["allowed_instructions"] = json!(instruction_identifiers());
    v["program_rules"]["allowed_attachments"] = json!(attachment_identifiers());
    v["program_rules"]["main_board"] = json!({"width":100,"height":2});
    v["program_rules"]["max_threads"] = json!(2);
    v
}
pub fn load(v: &Value) -> ValidatedSceneLevel {
    load_scene_level_json(
        v.to_string().as_bytes(),
        LoadLimits {
            max_level_bytes: 1_000_000,
            max_tests: 100,
            max_total_test_bytes: 1_000_000,
        },
    )
    .unwrap()
}
pub fn program(tokens: &[&str]) -> VerifiedProgram {
    let cells = std::iter::once(Cell::entry(Direction::Right))
        .chain(
            tokens
                .iter()
                .map(|t| Cell::instruction(PrimaryInstruction::from_token(t).unwrap(), None)),
        )
        .collect::<Vec<_>>();
    grid(cells.clone(), cells.len(), 1)
}
pub fn grid(cells: Vec<Cell>, width: usize, height: usize) -> VerifiedProgram {
    VerifiedProgram::new(Program {
        format_version: IR_FORMAT_VERSION,
        outer: ScopedProgram {
            main: Board {
                width,
                height,
                cells,
                folded_blocks: BTreeMap::new(),
            },
            functions: BTreeMap::new(),
        },
        customs: BTreeMap::new(),
    })
    .unwrap()
}
pub fn session(
    v: &Value,
    p: VerifiedProgram,
    c: EvaluationConfig,
    l: SceneLimits,
) -> SceneCaseSession {
    SceneCaseSession::new(load(v), p, 0, c, l).unwrap()
}
pub fn run(mut s: SceneCaseSession, slice: u64) -> SceneCaseResult {
    for _ in 0..2000 {
        if let SceneCaseProgress::Complete(result) = s.advance(n(slice), n(100_000)) {
            return result;
        }
    }
    panic!("bounded test session failed to terminate")
}
pub fn two_point_robot() -> Value {
    let mut v = author("robot");
    set_robot_path(&mut v, 3);
    set_robot_patrol(&mut v, 0, 2);
    v
}

pub fn set_robot_path(v: &mut Value, length: usize) {
    assert!(length <= 16);
    v["scene_config"]["map"]["terrain"][0] =
        json!(format!("{}{}", "0".repeat(length), ".".repeat(16 - length)));
}

pub fn set_robot_patrol(v: &mut Value, id: u64, index: usize) {
    let objects = v["scene_config"]["map"]["objects"]
        .as_array_mut()
        .expect("robot object list");
    if let Some(object) = objects
        .iter_mut()
        .find(|object| object["type"] == "patrol" && object["id"].as_u64() == Some(id))
    {
        object["index"] = json!(index);
    } else {
        objects.push(json!({"index":index,"type":"patrol","id":id}));
    }
}

pub fn set_robot_start(v: &mut Value, robot: &str, index: usize, direction: &str) {
    let objects = v["scene_config"]["map"]["objects"]
        .as_array_mut()
        .expect("robot object list");
    if let Some(object) = objects
        .iter_mut()
        .find(|object| object["type"] == "start" && object["robot"].as_str() == Some(robot))
    {
        object["index"] = json!(index);
        object["direction"] = json!(direction);
    } else {
        objects.push(json!({
            "index":index,
            "type":"start",
            "robot":robot,
            "direction":direction
        }));
    }
}
