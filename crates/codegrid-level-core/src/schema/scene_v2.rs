use super::*;
use crate::scenes::SceneKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StationType {
    Stack,
    Flip,
    Input,
    Output,
    Increment,
    Decrement,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MechanicalArmMode {
    OutputSequence,
    FinalLayout,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneConfig {
    ExactIO,
    Robot {
        actors: u8,
        map: RobotMap,
    },
    MechanicalArm {
        actors: u8,
        mode: MechanicalArmMode,
        stations: Vec<StationType>,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RobotStart {
    pub position: u8,
    pub direction: u8,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Trigger {
    pub id: u8,
    pub door_id: u8,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MapCell {
    pub color: u8,
    pub walkable: bool,
    pub patrol: bool,
    pub door: Option<u8>,
    pub trigger: Option<Trigger>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RobotMap {
    pub cells: Vec<MapCell>,
    pub starts: Vec<RobotStart>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneCaseData {
    Static {
        input: Vec<u8>,
        expected: Vec<u8>,
    },
    Robot,
    MechanicalArm {
        initial: Vec<Vec<u8>>,
        input: Vec<u8>,
        expected: Vec<u8>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SceneCase {
    pub visible: bool,
    pub data: SceneCaseData,
}

impl SceneCase {
    pub(crate) fn definition_units(&self) -> Option<u64> {
        match &self.data {
            SceneCaseData::Static { input, expected } => {
                (input.len() as u64).checked_add(expected.len() as u64)
            }
            SceneCaseData::Robot => Some(0),
            SceneCaseData::MechanicalArm {
                initial,
                input,
                expected,
            } => initial
                .iter()
                .try_fold(initial.len() as u64, |n, stack| {
                    n.checked_add(stack.len() as u64)
                })?
                .checked_add(input.len() as u64)?
                .checked_add(expected.len() as u64),
        }
    }
}

/// Only strict validation can construct a scene definition for execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedSceneLevel {
    level_id: String,
    level_version: u32,
    kind: SceneKind,
    config: SceneConfig,
    rules: ProgramRules,
    constraints: BTreeMap<String, u64>,
    scoring: BTreeMap<String, Option<u64>>,
    tests: Vec<SceneCase>,
    case_units: Vec<u64>,
    definition_units: u64,
}
impl ValidatedSceneLevel {
    pub(crate) fn definition_units(&self) -> u64 {
        self.definition_units
    }
    pub(crate) fn case_definition_units(&self, index: usize) -> Option<u64> {
        self.case_units.get(index).copied()
    }
    pub fn level_id(&self) -> &str {
        &self.level_id
    }
    pub fn level_version(&self) -> u32 {
        self.level_version
    }
    pub fn kind(&self) -> SceneKind {
        self.kind
    }
    pub fn config(&self) -> &SceneConfig {
        &self.config
    }
    pub fn rules(&self) -> &ProgramRules {
        &self.rules
    }
    pub fn constraints(&self) -> &BTreeMap<String, u64> {
        &self.constraints
    }
    pub fn scoring(&self) -> &BTreeMap<String, Option<u64>> {
        &self.scoring
    }
    pub fn tests(&self) -> &[SceneCase] {
        &self.tests
    }
}
fn array<'a>(v: &'a Value, p: &str) -> Result<&'a Vec<Value>, LevelError> {
    v.as_array()
        .ok_or_else(|| LevelError::invalid("IncorrectType", p))
}
fn byte(v: &Value, p: &str, max: u64) -> Result<u8, LevelError> {
    integer(v, p, max, false).map(|x| x as u8)
}
fn actor_count(v: &Value, p: &str) -> Result<u8, LevelError> {
    let n = integer(v, p, u64::MAX, false)?;
    if !(1..=2).contains(&n) {
        return Err(LevelError::invalid("InvalidActorCount", p));
    }
    Ok(n as u8)
}
fn config(kind: SceneKind, v: &Value) -> Result<SceneConfig, LevelError> {
    let p = "$.scene_config";
    Ok(match kind {
        SceneKind::ExactIO => {
            object(v, p, &[])?;
            SceneConfig::ExactIO
        }
        SceneKind::Robot => {
            let m = object(v, p, &["robot_count", "map"])?;
            let actors = actor_count(&m["robot_count"], &format!("{p}.robot_count"))?;
            let map = robot_map(&m["map"], &format!("{p}.map"), actors)?;
            SceneConfig::Robot { actors, map }
        }
        SceneKind::MechanicalArm => {
            let m = object(v, p, &["arm_count", "mode", "stations"])?;
            let actors = actor_count(&m["arm_count"], "$.scene_config.arm_count")?;
            let mode = match string(&m["mode"], "$.scene_config.mode")? {
                "OUTPUT_SEQUENCE" => MechanicalArmMode::OutputSequence,
                "FINAL_LAYOUT" => MechanicalArmMode::FinalLayout,
                _ => {
                    return Err(LevelError::invalid(
                        "UnsupportedMechanicalArmMode",
                        "$.scene_config.mode",
                    ))
                }
            };
            let count = if actors == 1 { 6 } else { 10 };
            let records = array(&m["stations"], "$.scene_config.stations")?;
            if records.len() != count {
                return Err(LevelError::invalid(
                    "InvalidStationCount",
                    "$.scene_config.stations",
                ));
            }
            let mut stations = vec![None; count];
            for (i, record) in records.iter().enumerate() {
                let q = format!("$.scene_config.stations[{i}]");
                let r = object(record, &q, &["station_id", "station_type"])?;
                let id = usize::from(byte(
                    &r["station_id"],
                    &format!("{q}.station_id"),
                    (count - 1) as u64,
                )?);
                if stations[id].is_some() {
                    return Err(LevelError::invalid(
                        "DuplicateStationId",
                        &format!("{q}.station_id"),
                    ));
                }
                stations[id] = Some(
                    match string(&r["station_type"], &format!("{q}.station_type"))? {
                        "STACK" => StationType::Stack,
                        "FLIP" => StationType::Flip,
                        "INPUT" => StationType::Input,
                        "OUTPUT" => StationType::Output,
                        "INCREMENT" => StationType::Increment,
                        "DECREMENT" => StationType::Decrement,
                        _ => {
                            return Err(LevelError::invalid(
                                "UnsupportedStationType",
                                &format!("{q}.station_type"),
                            ))
                        }
                    },
                );
            }
            let stations: Vec<_> = stations.into_iter().map(Option::unwrap).collect();
            for kind in [StationType::Flip, StationType::Input, StationType::Output] {
                if stations.iter().filter(|s| **s == kind).count() > 1 {
                    return Err(LevelError::invalid(
                        "DuplicateStationType",
                        "$.scene_config.stations",
                    ));
                }
            }
            match mode {
                MechanicalArmMode::OutputSequence if !stations.contains(&StationType::Output) => {
                    return Err(LevelError::invalid(
                        "MissingOutputStation",
                        "$.scene_config.stations",
                    ))
                }
                MechanicalArmMode::FinalLayout
                    if stations.contains(&StationType::Input)
                        || stations.contains(&StationType::Output) =>
                {
                    return Err(LevelError::invalid(
                        "InvalidFinalLayoutStation",
                        "$.scene_config.stations",
                    ))
                }
                _ => {}
            }
            SceneConfig::MechanicalArm {
                actors,
                mode,
                stations,
            }
        }
    })
}
fn robot_map(v: &Value, p: &str, actors: u8) -> Result<RobotMap, LevelError> {
    let m = object(v, p, &["width", "height", "terrain", "colors", "objects"])?;
    let width = integer(&m["width"], &format!("{p}.width"), u64::MAX, false)?;
    let height = integer(&m["height"], &format!("{p}.height"), u64::MAX, false)?;
    if width != 16 || height != 16 {
        return Err(LevelError::invalid("InvalidMapSize", p));
    }
    let terrain = array(&m["terrain"], &format!("{p}.terrain"))?;
    if terrain.len() != 16 {
        return Err(LevelError::invalid(
            "InvalidMapSize",
            &format!("{p}.terrain"),
        ));
    }
    let mut cells = Vec::with_capacity(256);
    for (y, row) in terrain.iter().enumerate() {
        let row = string(row, &format!("{p}.terrain[{y}]"))?;
        if !row.is_ascii() || row.len() != 16 {
            return Err(LevelError::invalid(
                "InvalidTerrainRow",
                &format!("{p}.terrain[{y}]"),
            ));
        }
        for symbol in row.bytes() {
            let walkable = match symbol {
                b'.' => false,
                b'0' => true,
                _ => {
                    return Err(LevelError::invalid(
                        "InvalidTerrainCell",
                        &format!("{p}.terrain[{y}]"),
                    ))
                }
            };
            cells.push(MapCell {
                color: 0,
                walkable,
                patrol: false,
                door: None,
                trigger: None,
            });
        }
    }

    let colors_path = format!("{p}.colors");
    let mut color_indices = BTreeSet::new();
    for (i, entry) in array(&m["colors"], &colors_path)?.iter().enumerate() {
        let q = format!("{colors_path}[{i}]");
        let record = object(entry, &q, &["index", "color"])?;
        let index = byte(&record["index"], &format!("{q}.index"), 255)?;
        if !color_indices.insert(index) {
            return Err(LevelError::invalid(
                "DuplicateMapIndex",
                &format!("{q}.index"),
            ));
        }
        let cell = &mut cells[usize::from(index)];
        if !cell.walkable {
            return Err(LevelError::invalid(
                "InvalidObjectPosition",
                &format!("{q}.index"),
            ));
        }
        cell.color = byte(&record["color"], &format!("{q}.color"), 2)?;
    }

    let objects_path = format!("{p}.objects");
    let mut object_indices = BTreeSet::new();
    let mut starts = BTreeMap::<String, RobotStart>::new();
    let mut patrol_ids = BTreeSet::new();
    let mut doors = BTreeSet::new();
    let mut triggers = BTreeSet::new();
    let mut door_paths = Vec::new();
    let mut trigger_paths = Vec::new();
    for (i, entry) in array(&m["objects"], &objects_path)?.iter().enumerate() {
        let q = format!("{objects_path}[{i}]");
        let object_fields = entry
            .as_object()
            .ok_or_else(|| LevelError::invalid("IncorrectType", &q))?;
        let kind = string(
            object_fields
                .get("type")
                .ok_or_else(|| LevelError::invalid("MissingField", &format!("{q}.type")))?,
            &format!("{q}.type"),
        )?;
        let fields: &[&str] = match kind {
            "start" => &["index", "type", "robot", "direction"],
            "patrol" | "trigger" | "door" => &["index", "type", "id"],
            _ => {
                return Err(LevelError::invalid(
                    "UnsupportedMapObject",
                    &format!("{q}.type"),
                ))
            }
        };
        let record = object(entry, &q, fields)?;
        let index = byte(&record["index"], &format!("{q}.index"), 255)?;
        if !object_indices.insert(index) {
            return Err(LevelError::invalid(
                "DuplicateMapIndex",
                &format!("{q}.index"),
            ));
        }
        let cell = &mut cells[usize::from(index)];
        if !cell.walkable {
            return Err(LevelError::invalid(
                "InvalidObjectPosition",
                &format!("{q}.index"),
            ));
        }
        match kind {
            "start" => {
                let robot = string(&record["robot"], &format!("{q}.robot"))?;
                if !["A", "B"].contains(&robot) {
                    return Err(LevelError::invalid("InvalidRobotId", &format!("{q}.robot")));
                }
                let direction = match string(&record["direction"], &format!("{q}.direction"))? {
                    "up" => 0,
                    "right" => 1,
                    "down" => 2,
                    "left" => 3,
                    _ => {
                        return Err(LevelError::invalid(
                            "InvalidDirection",
                            &format!("{q}.direction"),
                        ))
                    }
                };
                if starts
                    .insert(
                        robot.into(),
                        RobotStart {
                            position: index,
                            direction,
                        },
                    )
                    .is_some()
                {
                    return Err(LevelError::invalid(
                        "DuplicateRobotStart",
                        &format!("{q}.robot"),
                    ));
                }
            }
            "patrol" => {
                let id = byte(&record["id"], &format!("{q}.id"), 255)?;
                if !patrol_ids.insert(id) {
                    return Err(LevelError::invalid("DuplicateSceneId", &format!("{q}.id")));
                }
                cell.patrol = true;
            }
            "trigger" => {
                let id = byte(&record["id"], &format!("{q}.id"), 255)?;
                if !triggers.insert(id) {
                    return Err(LevelError::invalid("DuplicateSceneId", &format!("{q}.id")));
                }
                trigger_paths.push((id, format!("{q}.id")));
                cell.trigger = Some(Trigger { id, door_id: id });
            }
            "door" => {
                let id = byte(&record["id"], &format!("{q}.id"), 255)?;
                if !doors.insert(id) {
                    return Err(LevelError::invalid("DuplicateSceneId", &format!("{q}.id")));
                }
                door_paths.push((id, format!("{q}.id")));
                cell.door = Some(id);
            }
            _ => unreachable!("validated object type"),
        }
    }
    let expected_starts: Vec<&str> = if actors == 1 {
        vec!["A"]
    } else {
        vec!["A", "B"]
    };
    if starts.len() != usize::from(actors)
        || expected_starts.iter().any(|id| !starts.contains_key(*id))
    {
        return Err(LevelError::invalid(
            "ActorCountMismatch",
            &format!("{objects_path}"),
        ));
    }
    if !cells.iter().any(|c| c.patrol) {
        return Err(LevelError::invalid("MissingPatrolPoint", &objects_path));
    }
    for (id, path) in trigger_paths {
        if !doors.contains(&id) {
            return Err(LevelError::invalid("InvalidDoorReference", &path));
        }
    }
    for (id, path) in door_paths {
        if !triggers.contains(&id) {
            return Err(LevelError::invalid("UnpairedDoor", &path));
        }
    }
    let starts = expected_starts.iter().map(|id| starts[*id]).collect();
    Ok(RobotMap { cells, starts })
}
fn module_bytes(v: &Value, p: &str) -> Result<Vec<u8>, LevelError> {
    let values = bytes(v, p)?;
    for (i, value) in values.iter().enumerate() {
        if value & 3 == 3 {
            return Err(LevelError::invalid(
                "InvalidModuleByte",
                &format!("{p}[{i}]"),
            ));
        }
    }
    Ok(values)
}
fn station_contents(
    v: &Value,
    p: &str,
    stations: &[StationType],
    initial: bool,
) -> Result<Vec<Vec<u8>>, LevelError> {
    let records = array(v, p)?;
    if records.len() != stations.len() {
        return Err(LevelError::invalid("InvalidStationContents", p));
    }
    let mut result = Vec::with_capacity(stations.len());
    for (i, (record, kind)) in records.iter().zip(stations).enumerate() {
        let q = format!("{p}[{i}]");
        let contents = module_bytes(record, &q)?;
        let capacity = match kind {
            StationType::Stack => 4,
            StationType::Flip | StationType::Increment | StationType::Decrement if initial => 0,
            StationType::Flip | StationType::Increment | StationType::Decrement => 1,
            _ => 0,
        };
        if contents.len() > capacity {
            return Err(LevelError::invalid("StationCapacityExceeded", &q));
        }
        if contents
            .windows(2)
            .any(|pair| pair[0] & 8 == 0 && pair[1] & 8 != 0)
        {
            return Err(LevelError::invalid("InvalidModuleStack", &q));
        }
        result.push(contents);
    }
    Ok(result)
}
fn case(config: &SceneConfig, v: &Value, p: &str) -> Result<SceneCase, LevelError> {
    let fields: &[&str] = match config {
        SceneConfig::Robot { .. } => &["visible"],
        SceneConfig::MechanicalArm {
            mode: MechanicalArmMode::OutputSequence,
            ..
        } => &[
            "visible",
            "initial_station_contents",
            "input_queue",
            "expected_output",
        ],
        SceneConfig::MechanicalArm {
            mode: MechanicalArmMode::FinalLayout,
            ..
        } => &[
            "visible",
            "initial_station_contents",
            "target_station_contents",
        ],
        _ => &["visible", "input", "expected_output"],
    };
    let m = object(v, p, fields)?;
    let visible = boolean(&m["visible"], &format!("{p}.visible"))?;
    let data = match config {
        SceneConfig::Robot { .. } => SceneCaseData::Robot,
        SceneConfig::MechanicalArm { mode, stations, .. } => {
            let initial = station_contents(
                &m["initial_station_contents"],
                &format!("{p}.initial_station_contents"),
                stations,
                true,
            )?;
            let (input, expected) = match mode {
                MechanicalArmMode::OutputSequence => {
                    let input = module_bytes(&m["input_queue"], &format!("{p}.input_queue"))?;
                    let expected =
                        module_bytes(&m["expected_output"], &format!("{p}.expected_output"))?;
                    if expected.is_empty() {
                        return Err(LevelError::invalid(
                            "InvalidModuleSequence",
                            &format!("{p}.expected_output"),
                        ));
                    }
                    if !input.is_empty() && !stations.contains(&StationType::Input) {
                        return Err(LevelError::invalid(
                            "MissingInputStation",
                            &format!("{p}.input_queue"),
                        ));
                    }
                    (input, expected)
                }
                MechanicalArmMode::FinalLayout => {
                    let target = station_contents(
                        &m["target_station_contents"],
                        &format!("{p}.target_station_contents"),
                        stations,
                        false,
                    )?;
                    let inventory = |contents: &[Vec<u8>]| {
                        let mut counts = [0usize; 256];
                        let transforms_numbers = stations.iter().any(|kind| {
                            matches!(kind, StationType::Increment | StationType::Decrement)
                        });
                        for value in contents.iter().flatten() {
                            let key = if transforms_numbers {
                                *value & 0x0b
                            } else {
                                *value & !4
                            };
                            counts[usize::from(key)] += 1;
                        }
                        counts
                    };
                    if inventory(&initial) != inventory(&target) {
                        return Err(LevelError::invalid(
                            "ModuleConservationMismatch",
                            &format!("{p}.target_station_contents"),
                        ));
                    }
                    let expected = target
                        .iter()
                        .flat_map(|stack| {
                            std::iter::once(stack.len() as u8).chain(stack.iter().copied())
                        })
                        .collect();
                    (Vec::new(), expected)
                }
            };
            SceneCaseData::MechanicalArm {
                initial,
                input,
                expected,
            }
        }
        _ => {
            let input = bytes(&m["input"], &format!("{p}.input"))?;
            let expected = bytes(&m["expected_output"], &format!("{p}.expected_output"))?;
            SceneCaseData::Static { input, expected }
        }
    };
    Ok(SceneCase { visible, data })
}

/// Strict scene author-format-1 loading.
pub fn load_scene_level_json(
    input: &[u8],
    limits: LoadLimits,
) -> Result<ValidatedSceneLevel, LevelError> {
    if input.len() > limits.max_level_bytes {
        return Err(LevelError::invalid("InputSizeExceeded", "$"));
    }
    let text = std::str::from_utf8(input).map_err(|_| LevelError::invalid("InvalidUtf8", "$"))?;
    let envelope = parse_pairs(text, "$")?;
    let version = envelope
        .0
        .iter()
        .find(|(k, _)| k == "format_version")
        .ok_or_else(|| LevelError::invalid("MissingField", "$.format_version"))?;
    // Interpret the envelope version before decoding nested fields, mirroring
    // the v1 loader: out-of-range or negative integers keep the dedicated
    // UnsupportedFormatVersion identity instead of collapsing to level.invalid.
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
    let value = decode(text, "$", 0)?;
    let m = object(
        &value,
        "$",
        &[
            "format_version",
            "level_id",
            "level_version",
            "evaluation_type",
            "scene_type",
            "scene_config",
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
    let kind = match string(&m["scene_type"], "$.scene_type")? {
        "ExactIO" => SceneKind::ExactIO,
        "Robot" => SceneKind::Robot,
        "MechanicalArm" => SceneKind::MechanicalArm,
        _ => {
            return Err(LevelError {
                category: "UnsupportedSceneType",
                reason: "UnsupportedSceneType",
                path: "$.scene_type".into(),
            })
        }
    };
    let family = string(&m["evaluation_type"], "$.evaluation_type")?;
    if !["ExactIO", "Environment"].contains(&family) {
        return Err(LevelError::invalid(
            "UnsupportedEvaluationType",
            "$.evaluation_type",
        ));
    }
    let is_static = matches!(kind, SceneKind::ExactIO);
    if (family == "ExactIO") != is_static {
        return Err(LevelError::invalid(
            "SceneFamilyMismatch",
            "$.evaluation_type",
        ));
    }
    let config = config(kind, &m["scene_config"])?;
    let rules = parse_rules(&m["program_rules"])?;
    let (constraints, scoring) = parse_metric_policy(&m["constraints"], &m["scoring"])?;
    let e = object(&m["evaluation"], "$.evaluation", &["tests"])?;
    let arr = array(&e["tests"], "$.evaluation.tests")?;
    if arr.len() > limits.max_tests {
        return Err(LevelError::invalid(
            "TestCountExceeded",
            "$.evaluation.tests",
        ));
    }
    let mut size = 0usize;
    let mut tests = Vec::new();
    for (i, v) in arr.iter().enumerate() {
        size = size
            .checked_add(v.to_string().len())
            .ok_or_else(|| LevelError::invalid("TestDataSizeExceeded", "$.evaluation.tests"))?;
        if size > limits.max_total_test_bytes {
            return Err(LevelError::invalid(
                "TestDataSizeExceeded",
                "$.evaluation.tests",
            ));
        }
        tests.push(case(&config, v, &format!("$.evaluation.tests[{i}]"))?);
    }
    if tests.is_empty() || !tests.iter().any(|t| t.visible) {
        return Err(LevelError::invalid(
            "VisibleTestRequired",
            "$.evaluation.tests",
        ));
    }
    let case_units = tests
        .iter()
        .map(SceneCase::definition_units)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| LevelError::invalid("TestDataSizeExceeded", "$.evaluation.tests"))?;
    let config_units = match &config {
        SceneConfig::MechanicalArm { stations, .. } => stations.len() as u64,
        SceneConfig::Robot { map, .. } => {
            map.cells.len() as u64
                + map.starts.len() as u64
                + map
                    .cells
                    .iter()
                    .filter(|c| c.door.is_some() || c.trigger.is_some())
                    .count() as u64
        }
        _ => 0,
    };
    let definition_units = case_units
        .iter()
        .try_fold(
            config_units + rules.fixed_cells.len() as u64 + rules.instruction_limits.len() as u64,
            |n, units| n.checked_add(*units),
        )
        .ok_or_else(|| LevelError::invalid("TestDataSizeExceeded", "$.evaluation.tests"))?;
    Ok(ValidatedSceneLevel {
        level_id: id.into(),
        level_version,
        kind,
        config,
        rules,
        constraints,
        scoring,
        tests,
        case_units,
        definition_units,
    })
}
