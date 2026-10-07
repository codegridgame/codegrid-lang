mod scene_v2;
use crate::validate::{attachment_identifiers, instruction_identifiers};
pub use scene_v2::*;
use serde::de::{self, MapAccess, Visitor};
use serde::Deserializer;
use serde_json::{value::RawValue, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LevelError {
    pub category: &'static str,
    pub reason: &'static str,
    pub path: String,
}
impl LevelError {
    pub(crate) fn invalid(reason: &'static str, path: &str) -> Self {
        Self {
            category: "LevelInvalid",
            reason,
            path: path.into(),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoardBounds {
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramRules {
    pub allowed_instructions: BTreeSet<String>,
    pub allowed_attachments: BTreeSet<String>,
    pub main_board: BoardBounds,
    pub function_board: BoardBounds,
    pub max_functions: u32,
    pub max_custom: u32,
    pub max_threads: u32,
    pub memory_enabled: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactIoTest {
    pub visible: bool,
    pub input: Vec<u8>,
    pub expected_output: Vec<u8>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedLevel {
    level_id: String,
    level_version: u32,
    rules: ProgramRules,
    tests: Vec<ExactIoTest>,
    constraints: BTreeMap<String, u64>,
    scoring: BTreeMap<String, Option<u64>>,
}
impl ValidatedLevel {
    pub fn level_id(&self) -> &str {
        &self.level_id
    }
    pub fn level_version(&self) -> u32 {
        self.level_version
    }
    pub fn rules(&self) -> &ProgramRules {
        &self.rules
    }
    pub fn tests(&self) -> &[ExactIoTest] {
        &self.tests
    }
    pub fn constraints(&self) -> &BTreeMap<String, u64> {
        &self.constraints
    }
    pub fn scoring(&self) -> &BTreeMap<String, Option<u64>> {
        &self.scoring
    }
}
pub const METRICS: [&str; 11] = [
    "ticks",
    "cost",
    "operation_count",
    "memory_addresses_used",
    "max_data_stack_depth",
    "max_instruction_stack_depth",
    "max_call_stack_depth",
    "non_empty_cells",
    "instruction_kinds",
    "functions_used",
    "boards_used",
];
pub const CONSTRAINTS: [&str; 11] = [
    "max_ticks",
    "max_cost",
    "max_operation_count",
    "max_memory_addresses",
    "max_data_stack_depth",
    "max_instruction_stack_depth",
    "max_call_stack_depth",
    "max_non_empty_cells",
    "max_instruction_kinds",
    "max_functions_used",
    "max_boards_used",
];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoadLimits {
    pub max_level_bytes: usize,
    pub max_tests: usize,
    pub max_total_test_bytes: usize,
}
pub fn load_level(input: &[u8], limits: LoadLimits) -> Result<ValidatedLevel, LevelError> {
    let level = load_level_json(input, limits.max_level_bytes)?;
    if level.tests.len() > limits.max_tests {
        return Err(LevelError::invalid(
            "TestCountExceeded",
            "$.evaluation.tests",
        ));
    }
    let size = level.tests.iter().try_fold(0usize, |n, t| {
        n.checked_add(t.input.len())?
            .checked_add(t.expected_output.len())
    });
    if size.is_none_or(|n| n > limits.max_total_test_bytes) {
        return Err(LevelError::invalid(
            "TestDataSizeExceeded",
            "$.evaluation.tests",
        ));
    }
    Ok(level)
}

struct Pairs(Vec<(String, Box<RawValue>)>);
fn parse_pairs(text: &str, path: &str) -> Result<Pairs, LevelError> {
    let duplicate = std::cell::Cell::new(false);
    struct V<'a>(&'a std::cell::Cell<bool>);
    impl<'de> Visitor<'de> for V<'_> {
        type Value = Pairs;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("an object")
        }
        fn visit_map<M: MapAccess<'de>>(self, mut m: M) -> Result<Pairs, M::Error> {
            let mut v = Vec::new();
            let mut keys = BTreeSet::new();
            while let Some((k, x)) = m.next_entry::<String, Box<RawValue>>()? {
                if !keys.insert(k.clone()) {
                    self.0.set(true);
                    return Err(de::Error::custom(format!("duplicate key: {k}")));
                }
                v.push((k, x));
            }
            Ok(Pairs(v))
        }
    }
    let mut d = serde_json::Deserializer::from_str(text);
    let pairs = d.deserialize_map(V(&duplicate)).map_err(|_| {
        LevelError::invalid(
            if duplicate.get() {
                "DuplicateField"
            } else {
                "MalformedJson"
            },
            path,
        )
    })?;
    d.end()
        .map_err(|_| LevelError::invalid("MalformedJson", path))?;
    Ok(pairs)
}
fn decode(text: &str, path: &str, depth: usize) -> Result<Value, LevelError> {
    if depth > 128 {
        return Err(LevelError::invalid("NestingLimitExceeded", path));
    }
    let s = text.trim_start();
    if s.starts_with('{') {
        let pairs = parse_pairs(text, path)?;
        let mut map = serde_json::Map::new();
        for (k, v) in pairs.0 {
            let p = format!("{path}.{k}");
            map.insert(k, decode(v.get(), &p, depth + 1)?);
        }
        Ok(Value::Object(map))
    } else if s.starts_with('[') {
        let items: Vec<Box<RawValue>> =
            serde_json::from_str(text).map_err(|_| LevelError::invalid("MalformedJson", path))?;
        items
            .iter()
            .enumerate()
            .map(|(i, v)| decode(v.get(), &format!("{path}[{i}]"), depth + 1))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array)
    } else {
        serde_json::from_str(text).map_err(|_| LevelError::invalid("MalformedJson", path))
    }
}
fn object<'a>(
    v: &'a Value,
    path: &str,
    fields: &[&str],
) -> Result<&'a serde_json::Map<String, Value>, LevelError> {
    let m = v
        .as_object()
        .ok_or_else(|| LevelError::invalid("IncorrectType", path))?;
    for k in m.keys() {
        if !fields.contains(&k.as_str()) {
            return Err(LevelError::invalid("UnknownField", &format!("{path}.{k}")));
        }
    }
    for k in fields {
        if !m.contains_key(*k) {
            return Err(LevelError::invalid("MissingField", &format!("{path}.{k}")));
        }
    }
    Ok(m)
}
fn integer(v: &Value, path: &str, max: u64, positive: bool) -> Result<u64, LevelError> {
    let n = v
        .as_number()
        .and_then(|n| n.as_u64())
        .ok_or_else(|| LevelError::invalid("InvalidInteger", path))?;
    if n > max || positive && n == 0 {
        return Err(LevelError::invalid("IntegerOutOfRange", path));
    }
    Ok(n)
}
fn boolean(v: &Value, p: &str) -> Result<bool, LevelError> {
    v.as_bool()
        .ok_or_else(|| LevelError::invalid("IncorrectType", p))
}
fn string<'a>(v: &'a Value, p: &str) -> Result<&'a str, LevelError> {
    v.as_str()
        .ok_or_else(|| LevelError::invalid("IncorrectType", p))
}
fn bounds(v: &Value, p: &str) -> Result<BoardBounds, LevelError> {
    let m = object(v, p, &["width", "height"])?;
    let width = integer(&m["width"], &format!("{p}.width"), u32::MAX.into(), true)? as u32;
    let height = integer(&m["height"], &format!("{p}.height"), u32::MAX.into(), true)? as u32;
    if u64::from(width) * u64::from(height) > codegrid_model::MAX_BOARD_CELLS {
        return Err(LevelError::invalid("GeometryLimitExceeded", p));
    }
    Ok(BoardBounds { width, height })
}
fn capabilities(v: &Value, p: &str, names: &[&str]) -> Result<BTreeSet<String>, LevelError> {
    let arr = v
        .as_array()
        .ok_or_else(|| LevelError::invalid("IncorrectType", p))?;
    let mut set = BTreeSet::new();
    for (i, x) in arr.iter().enumerate() {
        let q = format!("{p}[{i}]");
        let name = string(x, &q)?;
        if !names.contains(&name) {
            return Err(LevelError::invalid("UnsupportedCapability", &q));
        }
        if !set.insert(name.to_owned()) {
            return Err(LevelError::invalid("DuplicateCapability", &q));
        }
    }
    Ok(set)
}
fn bytes(v: &Value, p: &str) -> Result<Vec<u8>, LevelError> {
    v.as_array()
        .ok_or_else(|| LevelError::invalid("IncorrectType", p))?
        .iter()
        .enumerate()
        .map(|(i, x)| integer(x, &format!("{p}[{i}]"), 255, false).map(|n| n as u8))
        .collect()
}
/// Decode logical UTF-8 JSON within the caller's trusted byte ceiling.
pub fn load_level_json(input: &[u8], max_bytes: usize) -> Result<ValidatedLevel, LevelError> {
    if input.len() > max_bytes {
        return Err(LevelError::invalid("InputSizeExceeded", "$"));
    }
    let text = std::str::from_utf8(input).map_err(|_| LevelError::invalid("InvalidUtf8", "$"))?;
    // Decode the envelope before interpreting any version-specific nested fields.
    let envelope = parse_pairs(text, "$")?;
    let version = envelope
        .0
        .iter()
        .find(|(k, _)| k == "format_version")
        .ok_or_else(|| LevelError::invalid("MissingField", "$.format_version"))?;
    let value: Value = serde_json::from_str(version.1.get())
        .map_err(|_| LevelError::invalid("InvalidInteger", "$.format_version"))?;
    let version = value
        .as_number()
        .map(ToString::to_string)
        .ok_or_else(|| LevelError::invalid("InvalidInteger", "$.format_version"))?;
    let digits = version.strip_prefix('-').unwrap_or(&version);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(LevelError::invalid("InvalidInteger", "$.format_version"));
    }
    if version != "1" {
        return Err(LevelError {
            category: "UnsupportedFormatVersion",
            reason: "UnsupportedFormatVersion",
            path: "$.format_version".into(),
        });
    }
    let v = decode(text, "$", 0)?;
    let m = object(
        &v,
        "$",
        &[
            "format_version",
            "level_id",
            "level_version",
            "evaluation_type",
            "program_rules",
            "constraints",
            "scoring",
            "evaluation",
        ],
    )?;
    let id = string(&m["level_id"], "$.level_id")?;
    if id.is_empty() || id.trim() != id {
        return Err(LevelError::invalid("InvalidLevelId", "$.level_id"));
    }
    let level_version = integer(
        &m["level_version"],
        "$.level_version",
        u32::MAX.into(),
        true,
    )? as u32;
    match string(&m["evaluation_type"], "$.evaluation_type")? {
        "ExactIO" => {}
        "Environment" => {
            let scene = m["evaluation"]
                .get("scene_type")
                .ok_or_else(|| LevelError::invalid("MissingField", "$.evaluation.scene_type"))?;
            string(scene, "$.evaluation.scene_type")?;
            return Err(LevelError {
                category: "UnsupportedSceneType",
                reason: "UnsupportedSceneType",
                path: "$.evaluation.scene_type".into(),
            });
        }
        _ => {
            return Err(LevelError::invalid(
                "UnsupportedEvaluationType",
                "$.evaluation_type",
            ))
        }
    }
    let rules = parse_rules(&m["program_rules"])?;
    let (constraints, scoring) = parse_metric_policy(&m["constraints"], &m["scoring"])?;
    let e = object(&m["evaluation"], "$.evaluation", &["tests"])?;
    let arr = e["tests"]
        .as_array()
        .ok_or_else(|| LevelError::invalid("IncorrectType", "$.evaluation.tests"))?;
    let mut tests = Vec::new();
    for (i, x) in arr.iter().enumerate() {
        let p = format!("$.evaluation.tests[{i}]");
        let t = object(x, &p, &["visible", "input", "expected_output"])?;
        tests.push(ExactIoTest {
            visible: boolean(&t["visible"], &format!("{p}.visible"))?,
            input: bytes(&t["input"], &format!("{p}.input"))?,
            expected_output: bytes(&t["expected_output"], &format!("{p}.expected_output"))?,
        });
    }
    if tests.is_empty() || !tests.iter().any(|t| t.visible) {
        return Err(LevelError::invalid(
            "VisibleTestRequired",
            "$.evaluation.tests",
        ));
    }
    Ok(ValidatedLevel {
        level_id: id.into(),
        level_version,
        rules,
        tests,
        constraints,
        scoring,
    })
}
fn parse_rules(value: &Value) -> Result<ProgramRules, LevelError> {
    let p = "$.program_rules";
    let r = object(
        value,
        p,
        &[
            "allowed_instructions",
            "allowed_attachments",
            "main_board",
            "function_board",
            "max_functions",
            "max_custom",
            "max_threads",
            "memory_enabled",
        ],
    )?;
    Ok(ProgramRules {
        allowed_instructions: capabilities(
            &r["allowed_instructions"],
            "$.program_rules.allowed_instructions",
            instruction_identifiers(),
        )?,
        allowed_attachments: capabilities(
            &r["allowed_attachments"],
            "$.program_rules.allowed_attachments",
            attachment_identifiers(),
        )?,
        main_board: bounds(&r["main_board"], "$.program_rules.main_board")?,
        function_board: bounds(&r["function_board"], "$.program_rules.function_board")?,
        max_functions: integer(
            &r["max_functions"],
            "$.program_rules.max_functions",
            10,
            false,
        )? as u32,
        max_custom: integer(&r["max_custom"], "$.program_rules.max_custom", 10, false)? as u32,
        max_threads: integer(
            &r["max_threads"],
            "$.program_rules.max_threads",
            u32::MAX.into(),
            true,
        )? as u32,
        memory_enabled: boolean(&r["memory_enabled"], "$.program_rules.memory_enabled")?,
    })
}
fn parse_metric_policy(
    constraints_value: &Value,
    scoring_value: &Value,
) -> Result<(BTreeMap<String, u64>, BTreeMap<String, Option<u64>>), LevelError> {
    let c = constraints_value
        .as_object()
        .ok_or_else(|| LevelError::invalid("IncorrectType", "$.constraints"))?;
    let mut constraints = BTreeMap::new();
    for (k, v) in c {
        let p = format!("$.constraints.{k}");
        if !CONSTRAINTS.contains(&k.as_str()) {
            return Err(LevelError::invalid("UnsupportedMetric", &p));
        }
        constraints.insert(k.clone(), integer(v, &p, u64::MAX, false)?);
    }
    let s = object(scoring_value, "$.scoring", &["metrics"])?;
    let s = s["metrics"]
        .as_object()
        .ok_or_else(|| LevelError::invalid("IncorrectType", "$.scoring.metrics"))?;
    let mut scoring = BTreeMap::new();
    for (k, v) in s {
        let p = format!("$.scoring.metrics.{k}");
        if !METRICS.contains(&k.as_str()) {
            return Err(LevelError::invalid("UnsupportedMetric", &p));
        }
        let x = object(v, &p, &["target"])?;
        scoring.insert(
            k.clone(),
            if x["target"].is_null() {
                None
            } else {
                Some(integer(
                    &x["target"],
                    &format!("{p}.target"),
                    u64::MAX,
                    false,
                )?)
            },
        );
    }
    Ok((constraints, scoring))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn level() -> Value {
        serde_json::json!({"format_version":1,"level_id":"测试.level","level_version":1,"evaluation_type":"ExactIO","program_rules":{"allowed_instructions":["HALT"],"allowed_attachments":[],"main_board":{"width":10,"height":10},"function_board":{"width":10,"height":10},"max_functions":0,"max_custom":0,"max_threads":1,"memory_enabled":false},"constraints":{},"scoring":{"metrics":{}},"evaluation":{"tests":[{"visible":true,"input":[],"expected_output":[]}]}})
    }
    fn load(v: &Value) -> Result<ValidatedLevel, LevelError> {
        load_level_json(v.to_string().as_bytes(), 100_000)
    }
    #[test]
    fn valid_empty_bytes_unicode_and_duplicate_tests() {
        let mut v = level();
        let t = v["evaluation"]["tests"][0].clone();
        v["evaluation"]["tests"].as_array_mut().unwrap().push(t);
        assert_eq!(load(&v).unwrap().tests().len(), 2);
    }
    #[test]
    fn exact_wide_integer() {
        let mut v = level();
        v["constraints"]["max_ticks"] = Value::from(u64::MAX);
        assert_eq!(load(&v).unwrap().constraints()["max_ticks"], u64::MAX);
        let s = v
            .to_string()
            .replace(&u64::MAX.to_string(), "18446744073709551616");
        assert_eq!(
            load_level_json(s.as_bytes(), 100_000).unwrap_err().reason,
            "InvalidInteger"
        );
    }
    #[test]
    fn format_dispatch_precedes_payload() {
        let s = r#"{"format_version":2,"future":{"x":1,"x":2}}"#;
        assert_eq!(
            load_level_json(s.as_bytes(), 1000).unwrap_err().category,
            "UnsupportedFormatVersion"
        );
    }
    #[test]
    fn nested_duplicate_and_unknown_paths() {
        let s = level()
            .to_string()
            .replace("\"visible\":true", "\"visible\":true,\"visible\":false");
        let e = load_level_json(s.as_bytes(), 100_000).unwrap_err();
        assert_eq!(e.reason, "DuplicateField");
        assert_eq!(e.path, "$.evaluation.tests[0]");
        let mut v = level();
        v["program_rules"]["extra"] = Value::Bool(false);
        assert_eq!(load(&v).unwrap_err().path, "$.program_rules.extra");
    }
    #[test]
    fn types_domains_and_required_fields() {
        for value in [
            Value::Bool(true),
            Value::from(-1),
            serde_json::json!(1.0),
            Value::from(0),
        ] {
            let mut v = level();
            v["level_version"] = value;
            assert!(load(&v).is_err());
        }
        let mut v = level();
        v["evaluation"]["tests"][0]["input"] = serde_json::json!([256]);
        assert_eq!(load(&v).unwrap_err().path, "$.evaluation.tests[0].input[0]");
        let mut v = level();
        v["program_rules"]
            .as_object_mut()
            .unwrap()
            .remove("allowed_attachments");
        assert_eq!(load(&v).unwrap_err().reason, "MissingField");
    }
    #[test]
    fn reject_metric_capability_visibility_and_scene() {
        let mut v = level();
        v["scoring"]["metrics"]["stop_count"] = serde_json::json!({"target":null});
        assert_eq!(load(&v).unwrap_err().reason, "UnsupportedMetric");
        let mut v = level();
        v["program_rules"]["allowed_instructions"] = serde_json::json!(["HALT", "HALT"]);
        assert_eq!(load(&v).unwrap_err().reason, "DuplicateCapability");
        let mut v = level();
        v["evaluation"]["tests"][0]["visible"] = Value::Bool(false);
        assert_eq!(load(&v).unwrap_err().reason, "VisibleTestRequired");
        let mut v = level();
        v["evaluation_type"] = Value::from("Environment");
        v["evaluation"] = serde_json::json!({"scene_type":"Elevator"});
        assert_eq!(load(&v).unwrap_err().category, "UnsupportedSceneType");
    }
    #[test]
    fn trusted_decode_limits_and_utf8() {
        assert_eq!(
            load_level_json(&[255], 10).unwrap_err().reason,
            "InvalidUtf8"
        );
        assert_eq!(
            load_level_json(b"{}", 1).unwrap_err().reason,
            "InputSizeExceeded"
        );
        let text = level().to_string();
        assert_eq!(
            load_level(
                text.as_bytes(),
                LoadLimits {
                    max_level_bytes: 100_000,
                    max_tests: 0,
                    max_total_test_bytes: 0
                }
            )
            .unwrap_err()
            .reason,
            "TestCountExceeded"
        );
    }
    #[test]
    fn weak_targets_and_aliases_permitted() {
        let mut v = level();
        v["scoring"]["metrics"] =
            serde_json::json!({"cost":{"target":0},"operation_count":{"target":null}});
        assert_eq!(load(&v).unwrap().scoring().len(), 2);
    }
    #[test]
    fn geometry_count_and_identity_boundaries() {
        for (pointer, boundary_value, reason) in [
            (
                "/program_rules/main_board/width",
                Value::from(0),
                "IntegerOutOfRange",
            ),
            (
                "/program_rules/function_board/height",
                Value::from(0),
                "IntegerOutOfRange",
            ),
            (
                "/program_rules/main_board/width",
                Value::from(u64::from(u32::MAX) + 1),
                "IntegerOutOfRange",
            ),
            (
                "/program_rules/max_functions",
                Value::from(11),
                "IntegerOutOfRange",
            ),
            (
                "/program_rules/max_custom",
                Value::from(11),
                "IntegerOutOfRange",
            ),
            (
                "/program_rules/max_threads",
                Value::from(0),
                "IntegerOutOfRange",
            ),
            (
                "/level_version",
                Value::from(u64::from(u32::MAX) + 1),
                "IntegerOutOfRange",
            ),
            ("/level_id", Value::from(""), "InvalidLevelId"),
            ("/level_id", Value::from(" level"), "InvalidLevelId"),
            ("/level_id", Value::from("level\u{2003}"), "InvalidLevelId"),
        ] {
            let mut value = level();
            *value.pointer_mut(pointer).unwrap() = boundary_value;
            assert_eq!(load(&value).unwrap_err().reason, reason, "{pointer}");
        }
        let mut value = level();
        value["program_rules"]["main_board"] = serde_json::json!({"width":65536,"height":65536});
        assert_eq!(load(&value).unwrap_err().reason, "GeometryLimitExceeded");
        value["program_rules"]["main_board"] = serde_json::json!({"width":u32::MAX,"height":1});
        value["program_rules"]["max_functions"] = Value::from(10);
        value["program_rules"]["max_custom"] = Value::from(10);
        value["program_rules"]["max_threads"] = Value::from(u32::MAX);
        value["level_version"] = Value::from(u32::MAX);
        assert!(load(&value).is_ok());
    }
    #[test]
    fn required_fields_empty_arrays_and_every_object_reject_unknown_fields() {
        for (parent, field) in [
            ("", "level_id"),
            ("", "level_version"),
            ("", "evaluation_type"),
            ("", "program_rules"),
            ("", "constraints"),
            ("", "scoring"),
            ("", "evaluation"),
            ("/program_rules", "allowed_instructions"),
            ("/program_rules", "allowed_attachments"),
            ("/program_rules", "main_board"),
            ("/program_rules", "function_board"),
            ("/program_rules", "max_functions"),
            ("/program_rules", "max_custom"),
            ("/program_rules", "max_threads"),
            ("/program_rules", "memory_enabled"),
            ("/program_rules/main_board", "width"),
            ("/program_rules/main_board", "height"),
            ("/program_rules/function_board", "width"),
            ("/program_rules/function_board", "height"),
            ("/scoring", "metrics"),
            ("/evaluation", "tests"),
            ("/evaluation/tests/0", "visible"),
            ("/evaluation/tests/0", "input"),
            ("/evaluation/tests/0", "expected_output"),
        ] {
            let mut value = level();
            value
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert_eq!(
                load(&value).unwrap_err().reason,
                "MissingField",
                "{parent}/{field}"
            );
        }
        for parent in [
            "",
            "/program_rules",
            "/program_rules/main_board",
            "/program_rules/function_board",
            "/scoring",
            "/evaluation",
            "/evaluation/tests/0",
        ] {
            let mut value = level();
            value
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unknown".into(), Value::Bool(true));
            assert_eq!(load(&value).unwrap_err().reason, "UnknownField", "{parent}");
        }
        let mut value = level();
        value["evaluation"]["tests"] = serde_json::json!([]);
        assert_eq!(load(&value).unwrap_err().reason, "VisibleTestRequired");
        value = level();
        value["program_rules"]["allowed_instructions"] = serde_json::json!([]);
        assert!(load(&value).is_ok());
        for field in ["allowed_instructions", "allowed_attachments"] {
            value = level();
            value["program_rules"][field] = serde_json::json!(["FUTURE"]);
            assert_eq!(load(&value).unwrap_err().reason, "UnsupportedCapability");
        }
    }
    #[test]
    fn trusted_total_data_and_nesting_boundaries() {
        let mut value = level();
        value["evaluation"]["tests"][0]["input"] = serde_json::json!([0, 255]);
        value["evaluation"]["tests"][0]["expected_output"] = serde_json::json!([1]);
        let text = value.to_string();
        let mut limits = LoadLimits {
            max_level_bytes: text.len(),
            max_tests: 1,
            max_total_test_bytes: 3,
        };
        assert!(load_level(text.as_bytes(), limits).is_ok());
        limits.max_total_test_bytes = 2;
        assert_eq!(
            load_level(text.as_bytes(), limits).unwrap_err().reason,
            "TestDataSizeExceeded"
        );
        let nested = format!(
            "{{\"format_version\":1,\"nested\":{}0{}}}",
            "[".repeat(130),
            "]".repeat(130)
        );
        assert_eq!(
            load_level_json(nested.as_bytes(), 100_000)
                .unwrap_err()
                .reason,
            "NestingLimitExceeded"
        );
        let text = r#"{"format_version":18446744073709551616,"future":true}"#;
        assert_eq!(
            load_level_json(text.as_bytes(), 1000).unwrap_err().category,
            "UnsupportedFormatVersion"
        );
    }
}
