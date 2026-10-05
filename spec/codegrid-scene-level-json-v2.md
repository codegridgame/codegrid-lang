# CodeGrid Scene Level JSON Format v1

Status: revised author-file contract; the existing loader and fixtures require migration to this revision. The filename retains its earlier name until the documentation layout is consolidated.
Recorded: 2026-10-05.
Authorities: [Scene Spec v1](codegrid-scene-spec-v1.md) and the
[continuous session design](../docs/scene-session-design.md).

## 1. Version

The selected scene author contract uses `format_version: 1`. No legacy scene
format or compatibility branch is required. Scene documents explicitly contain
scene_type and scene_config; never infer a scene from filenames or assets.
The user's version reset supersedes the earlier author-format-2 proposal.
This documentation change does not itself migrate host implementations or the
independently specified language IR and runtime contracts.

## 2. Strict JSON and common envelope

Use UTF-8 JSON without a BOM or comments. Reject duplicate keys, unknown fields
at every object level, omitted required fields, incorrect types, fractional or
out-of-range numbers, and unsupported versions/enums. There are no implicit
field defaults except the explicitly specified Robot color default. Null is legal only where explicitly listed. Array order is
semantic; object-key order is not. Boolean is not an integer. Do not truncate
wide integers or bytes. Preserve exact Unicode IDs without normalization.

| Root field | Required value |
| --- | --- |
| format_version | JSON integer 1 |
| level_id | Nonempty Unicode string, no leading/trailing whitespace |
| level_version | Integer 1–4294967295 |
| evaluation_type | Exact case-sensitive `ExactIO` or `Environment` |
| scene_type | One scene identity from the table below |
| scene_config | Exactly the selected scene's configuration object |
| program_rules | Existing required v1 capability/geometry object |
| constraints | Metric-limit object, possibly empty |
| scoring | Exactly `{ "metrics": { ... } }`, possibly empty metrics |
| evaluation | Exactly `{ "tests": [ ... ] }` |

| scene_type | evaluation_type | scene_config fields |
| --- | --- | --- |
| ExactIO | ExactIO | None: exactly `{}` |
| Baudot | ExactIO | None: exactly `{}` |
| QualityControl | ExactIO | input_mode, case_mode |
| Elevator | Environment | elevator_count |
| Robot | Environment | robot_count |
| MechanicalArm | Environment | arm_count, worktables |

A mismatched family/scene combination is invalid, even if the tests happen
to resemble another scene. Configuration is level-wide and shared by all cases.
Each case owns its initial world and queues. Tests must be nonempty with at
least one visible test. Every test explicitly declares `visible: boolean`.
Duplicate tests are independent. Existing Debug/Official selection, shuffling,
hidden-data exclusion, and static validation rules apply.

Neither scene_config nor tests may override program_rules, constraints,
scoring, host seeds, boundary mode, VM registers/memory/stacks, Custom limits,
work budgets, or trusted profiles. No presentation fields, localized text,
image paths, callbacks, or script expressions are accepted. Keep those in a
separate host-owned file keyed by level_id and level_version.

## 3. Program rules, constraints, and scoring

Reuse all required program_rules fields and their domains from the
[ExactIO contract](codegrid-level-exactio-contract-v1.md#1-level-loading-and-numeric-domains):
allowed_instructions, allowed_attachments, main_board, function_board,
max_functions, max_custom, max_threads, memory_enabled. The capability vocabulary
includes CMP and independent CONDITION_0/1/2. Scene actor count is not VM
max_threads: a single VM thread can issue both actors' actions.

Reuse the existing 11 VM/static metric identifiers, constraint mappings,
SUM/MAX/STATIC aggregation, minimize scoring, target/null shape, wide integer
domains, and widened rating arithmetic. Aggregate visible cases only, using
one continuous VM per dynamic case. Do not sum multiple cumulative snapshots
of the same VM. VM peaks and used-address cardinalities are per-case values
aggregated with their existing MAX rule; static metrics are computed once.

Only Elevator additionally accepts:

| Metric | Aggregation | Constraint | Direction |
| --- | --- | --- | --- |
| travel_distance | SUM of visible case totals | max_travel_distance | Minimize |
| stop_count | SUM of visible case totals | max_stop_count | Minimize |

All limits and non-null targets are integer 0–18446744073709551615. A metric
unsupported by the selected scene is invalid. Scene rounds are internal state,
not an extra v2 scoring metric. Overflow is a typed evaluator/resource fault,
never saturation or wrapped data. Hosts pass original author JSON or exact
integer transport; JavaScript Number must not round u64 limits/targets.

## 4. Static scene test records

ExactIO, Baudot, and QualityControl tests have exactly these fields:

```json
{ "visible": true, "input": [1], "expected_output": [1] }
```

| Scene | input | expected_output |
| --- | --- | --- |
| ExactIO | Bytes 0–255; empty permitted | Bytes 0–255; empty permitted |
| Baudot | Codes 0–31; empty permitted | Codes 0–31; empty permitted |
| QualityControl Item | Exactly one valid item byte | Exactly one 0/1 decision |
| QualityControl Batch | At least one valid item byte | Exactly one 0/1 decision |

QualityControl config has exactly `input_mode: "Color" | "PackedRobot"` and
`case_mode: "Item" | "Batch"`. Color bytes are 0–2. PackedRobot bytes are
0–255 with `(byte & 3) != 3`; bit assignments follow Scene Spec Section 7.
No count prefix, per-item expected array, predicate, or rule-expression field.

Baudot wire correctness is raw five-bit codes. It has no mandatory character
table/display mode field; any future presentation table must stay outside
this author correctness payload.

## 5. Elevator records

Config has exactly `elevator_count: 1 | 2`.
Each test has exactly:

```json
{
  "visible": true,
  "initial_floors": [0],
  "initial_passengers": [{ "from": 2, "to": 7 }],
  "sequential_passengers": []
}
```

initial_floors length equals elevator_count; A then B, each floor 0–9. Initial
floors may coincide. initial_passengers is nonempty. Sequential may be empty.
Each passenger has exactly from/to, both 0–9 and different. Total passengers
across both arrays is 1–10; preserve duplicates and author order as separate
passengers. There are no passenger IDs, count prefixes, arrival-time fields,
capacity fields, expected_output, or initial onboard passengers.

The deterministic round-arrival rule, boarding order, no-op targets, and goal
come exclusively from Scene Spec Section 5, not author overrides.

## 6. Robot records

Config has exactly robot_count (1 or 2) and map. The map has exactly width,
height, terrain, colors, and objects. Width and height are both 16.

terrain is exactly 16 strings of exactly 16 ASCII characters each. Only `.`,
`0`, and `1` are accepted: VOID, low ground, and high ground respectively.
The initial editor canvas is all VOID; authors explicitly draw usable ground.
VOID cannot be occupied or entered by FORWARD or JUMP. There is no wall type.
Position index is `y * 16 + x`, in 0–255.

colors is a sparse array of exactly `{ "index": 18, "color": 1 }` records.
Indices are unique within colors and must address drawn ground. Color is 0
(NONE), 1 (BLUE), or 2 (RED); drawn ground without a color record defaults to 0.
Canonical editor export omits color-0 records and sorts records by index.

objects is a sparse array with globally unique indices within that array.
An index may also occur in colors because color is a terrain attribute.
Every object must address drawn ground. Each record has exactly the fields
listed for its type:

```json
[
  { "index": 18, "type": "start", "robot": "A", "direction": "right" },
  { "index": 22, "type": "patrol", "id": 0 },
  { "index": 83, "type": "trigger", "id": 0 },
  { "index": 55, "type": "door", "id": 0 }
]
```

start, patrol, trigger, and door are mutually exclusive on a cell. A one-robot
map has exactly one A start; a two-robot map has exactly one A and one B start.
Directions are up, right, down, or left, mapping to protocol values 0–3.
Require at least one patrol. Object IDs are integers 0–255. Patrol IDs are
unique in their own namespace. Mechanism IDs pair exactly one trigger and one
door with the same ID; no separate reference field is stored. Reject duplicate
triggers, duplicate doors, and unpaired mechanism IDs. Patrol and mechanism
IDs may coincide. Doors start closed and become traversable only after opening.
Neither starts nor any other object can share a door cell.

The static map, colors, and starts belong to scene_config. Each Robot test has
exactly visible; no per-test map, robot overrides, device states, or expected
output is accepted. This revision does not impose a new single-test limit.
There is no device object. Validation checks structure and associations, not
connectivity or program solvability. Erasing ground in an editor removes its
color and object; associated mechanisms must be repaired or removed before export.

## 7. MechanicalArm records

Config has exactly arm_count (1 or 2) and worktables (exactly four entries).
Each entry is null or one of Inspection, Repair, Processing, Packing, Removal.
Duplicates are legal. Slots are fixed:

| Mode | worktables array order |
| --- | --- |
| One arm | Orientations 1, 2, 4, 5 |
| Two arms | A orientation 1, A orientation 2, B orientation 4, B orientation 5 |

Null means a noninteractive empty slot. Buffers, conveyors, actor locations,
empty initial hands, and LEFT initial orientations are fixed by the protocol,
not additional configuration fields.

Each test has exactly visible, input, and expected_output. input is a nonempty
array of ordered records, each exactly:

```json
{ "color": 1, "true_inspection": 0 }
```

color and true_inspection are integers 0–2, with Scene Spec's numeric enums.
Reject input runtime-state bytes and initial processed/packed flags.
expected_output is a nonempty array of visible state bytes, length no greater
than input length. Require bits 7–6 zero, color not 3, and processed/packed zero
unless inspection is NORMAL. Byte 255 is invalid. NORMAL packed with processed
zero is legal. Do not require the loader to prove solvability or identify a
specific input robot for each expected state.

## 8. Validation surfaces and host integration

Parse format_version before selecting version-specific fields, after checking
JSON syntax and duplicate keys. Report unsupported format/evaluation/scene
categories explicitly; other malformed fields are LevelInvalid with a typed
reason and an exact path, e.g. `$.scene_config.map.objects[3].id`.
For cross-field errors use the offending field/reference path. Do not infer
identities from English wording. The [Scene Host Contract v2](codegrid-scene-host-contract-v2.md) fixes
existing numeric identities, new typed reason spellings, and transport versions
without repurposing v1 published identities. Planned scenes remain unsupported at runtime until
their executable registration gates pass.

Robot map validation paths are fixed as follows:

| Failure | Path |
| --- | --- |
| Invalid width or height | `$.scene_config.map` |
| `robot_count` not 1 or 2 | `$.scene_config.robot_count` |
| Terrain row count is not 16 | `$.scene_config.map.terrain` |
| Invalid terrain row length or non-ASCII row | `$.scene_config.map.terrain[y]` |
| Invalid terrain character | `$.scene_config.map.terrain[y]` |
| Duplicate sparse color/object index | Later record's `.index` |
| Color/object placed on VOID | That record's `.index` |
| Unknown object type | That record's `.type` |
| Invalid robot identity or duplicate start identity | That start's `.robot` |
| Invalid direction string | That start's `.direction` |
| Start records do not match `robot_count` | `$.scene_config.map.objects` |
| No patrol object | `$.scene_config.map.objects` |
| Duplicate same-type object ID | Later record's `.id` |
| Trigger without same-ID door / door without same-ID trigger | Unmatched record's `.id` |

For example, an unmatched trigger at object index 3 is reported at
`$.scene_config.map.objects[3].id`. Out-of-range numeric fields keep their
specific field path and the shared `IntegerOutOfRange` reason.

Case execution is not initial-state deserialization: decode untrusted records,
validate them into opaque definitions, then construct fresh VM/world state.
All hidden records, including MechanicalArm true_inspection, stay behind the
existing hidden-test privacy boundary. Trusted load/test/world/feedback ceilings
apply to every case; authors cannot disable them through this JSON.

Replay uses the existing explicit root seed, test-selection/shuffle rules, and
VM seed derivation `mix64(root_seed XOR 0x43474C564D303031)` for every case's
fresh VM. Dynamic scenes add no host-random world events. Keep that seed and
the VM PRNG state for the entire case, without per-action reseeding. Include
format_version, scene identity/config, original case index, validated logical
level identity/hash, source, boundary mode, Custom limit, and trusted profile
in replay identity. New host wire fields require a recorded version decision.

## 9. Examples and acceptance status

The [v2 author examples](../examples/scene-level-v2/README.md) contain complete
JSON documents for every scene and six deliberately invalid counterparts.
Their manifest records expected validation and exact offending paths.
They are specification examples, not current loader-accepted fixtures or
execution evidence. Promote them only after implementing Rust validation and
running the [scene conformance plan](../docs/scene-conformance-plan.md).

The existing example files and loader tests describe the earlier implementation.
They require migration to format_version 1 and the revised Robot map contract;
their earlier passing results do not establish acceptance of this revision.
