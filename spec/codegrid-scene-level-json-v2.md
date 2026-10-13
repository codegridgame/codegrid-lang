# CodeGrid Scene Level JSON Format v1

**Implemented amendment (2026-10-08):**
[Status Flag and directionless READ](../docs/status-flag-and-read.md) defines the current F state,
`?!`, directionless `,`, empty POPADD flag and Level permissions. These rules
are implemented in the current contracts and Rust/WASM hosts.
The [2026-10-11 implementation record](../docs/gas-size-implementation.md)
records current Gas, Size, Custom restrictions and actual-host evidence.

Status: current author-file contract, implemented by the Rust loader and fixtures. The filename retains its historical v2 name; the logical format is 1.
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
field defaults except the explicitly specified Robot color default and optional empty fixed_cells/instruction_limits. Null is legal only where explicitly listed. Array order is
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
| Robot | Environment | robot_count |
| MechanicalArm | Environment | arm_count, mode, stations |

A mismatched family/scene combination is invalid, even if the tests happen
to resemble another scene. Configuration is level-wide and shared by all cases.
Each case owns its initial world and queues. Tests must be nonempty with at
least one visible test. Every test explicitly declares `visible: boolean`.
Duplicate tests are independent. Existing Debug/Official selection, shuffling,
hidden-data exclusion, and static validation rules apply.

Neither scene_config nor tests may override program_rules, constraints,
scoring, host seeds, VM registers/memory/stacks, Custom limits,
work budgets, or trusted profiles. No presentation fields, localized text,
image paths, callbacks, or script expressions are accepted. Keep those in a
separate host-owned file keyed by level_id and level_version.

## 3. Program rules, constraints, and scoring

Reuse all required program_rules fields and their domains from the
[ExactIO contract](codegrid-level-exactio-contract-v1.md#1-level-loading-and-numeric-domains):
allowed_instructions, allowed_attachments, main_board, function_board,
max_functions, max_custom, max_threads, memory_enabled. The capability vocabulary
includes CMP, independent CONDITION_0/1/2, and the optional OUTPUT_IMMEDIATE
permission for `.0` through `.9` (absent by default). Ordinary OUTPUT remains
always allowed. Scene actor count is not VM
max_threads: a single VM thread can issue both actors' actions.

Reuse the existing 11 VM/static metric identifiers, constraint mappings,
SUM/MAX/STATIC aggregation, minimize scoring, target/null shape, wide integer
domains, and widened rating arithmetic. Aggregate visible cases only, using
one continuous VM per dynamic case. Do not sum multiple cumulative snapshots
of the same VM. VM peaks and used-address cardinalities are per-case values
aggregated with their existing MAX rule; static metrics are computed once.

No current scene adds scoring metrics beyond the shared VM/static metrics.

All limits and non-null targets are integer 0–18446744073709551615. A metric
unsupported by the selected scene is invalid. Scene rounds are internal state,
not an extra v2 scoring metric. Overflow is a typed evaluator/resource fault,
never saturation or wrapped data. Hosts pass original author JSON or exact
integer transport; JavaScript Number must not round u64 limits/targets.

## 4. Static scene test records

ExactIO tests have exactly these fields:

```json
{ "visible": true, "input": [1], "expected_output": [1] }
```

| Scene | input | expected_output |
| --- | --- | --- |
| ExactIO | Bytes 0–255; empty permitted | Bytes 0–255; empty permitted |

## 5. Removed scene identifiers

Baudot, QualityControl, and Elevator are unsupported Rust scene identifiers. Paper-tape and quality
authoring emit ExactIO byte cases; presentation data stays outside this payload.
Existing elevator drafts require recovery in their application and cannot run.

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

Config has exactly arm_count (1 or 2), mode (OUTPUT_SEQUENCE or FINAL_LAYOUT),
and stations. Each station record has exactly station_id and station_type.
Provide every ID exactly once: 0..5 for one arm or 0..9 for two. Record order
is immaterial. Types are STACK, FLIP, INPUT, OUTPUT. Capacities derive from
type: STACK=4, FLIP=1, INPUT/OUTPUT=0. At most one each special type is allowed.
OUTPUT_SEQUENCE requires OUTPUT; FINAL_LAYOUT forbids conveyors.

OUTPUT_SEQUENCE cases contain exactly visible, initial_station_contents,
input_queue, expected_output. FINAL_LAYOUT cases contain exactly visible,
initial_station_contents, target_station_contents. Contents arrays have one
bottom-to-top array per numeric global station ID. Enforce capacity, valid
module bytes, empty conveyors, and no heavy module above light. Bytes have
color 0/1/2 in bits 1..0, face in bit 2, weight in bit 3, number in bits 7..4.
255 and all color-3 bytes are invalid modules. A nonempty input_queue requires
INPUT. Expected output must be nonempty; it may consume a subset of modules.
Final initial/target inventory must match after clearing bit 2. Rust derives
target count-prefixed bytes; independent expected_output is forbidden.
Hands/directions start fixed and are not author target fields. Loading FLIP
contents preserves the authored face. Validation does not prove reachability.

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
All hidden case records, stay behind the
existing hidden-test privacy boundary. Trusted load/test/world/feedback ceilings
apply to every case; authors cannot disable them through this JSON.

Replay uses the existing explicit root seed, test-selection/shuffle rules, and
VM seed derivation `mix64(root_seed XOR 0x43474C564D303031)` for every case's
fresh VM. Dynamic scenes add no host-random world events. Keep that seed and
the VM PRNG state for the entire case, without per-action reseeding. Include
format_version, scene identity/config, original case index, validated logical
level identity/hash, source, Custom limit, and trusted profile
in replay identity. New host wire fields require a recorded version decision.

## 9. Examples and acceptance status

The [v2 author examples](../examples/scene-level-v2/README.md) contain complete
JSON documents for every scene and six deliberately invalid counterparts.
Their manifest records expected validation and exact offending paths.
The Rust loader checks their accepted/rejected outcomes against the manifest.
Actual-host execution evidence is maintained separately in the
[implementation record](../docs/gas-size-implementation.md); validation examples
alone do not establish execution parity.

## Gas and Size metrics

The current metric vocabulary uses `gas_used` and `size`, with `max_gas` and
`max_size` constraints. Former Cost and nonempty-cell metric names are rejected
without aliases. Gas schedule version 1 is identified by `gas_schedule_version`
in evaluation results.

Size counts each nonempty source cell, including explicit Entry and each Folded
Block reference. Folded Block body nonempty cells count once per definition,
including unused definitions. Function bodies count once, including unused
functions. Implicit Entry adds no cell. Runtime code changes do not alter Size.
Fixed initial instructions count normally toward Size; they have no metric exemption.

Each test starts with independent Gas, cold-memory tracking, and stack capacity
peaks. `gas_used`, `execution_gas`, `memory_gas`, and `stack_gas` are summed over
visible cases; their breakdown satisfies `gas_used = execution_gas + memory_gas
+ stack_gas`. Hidden cases contribute no public metrics or soft constraints but
remain subject to the trusted per-test Gas ceiling. Auxiliary dynamic peaks use
maximum aggregation; static metrics count once per program.

Only `gas_used` is selectable for scoring or a soft Gas constraint; breakdown
metrics are diagnostic result fields, not author-defined score dimensions.
Failed evaluations have no final metrics or rating. Runtime Gas-limit failure
remains a structured runtime error and cannot produce a successful score.

## Fixed main-board initial instructions (2026-10-11)

`program_rules.fixed_cells` is optional; omission means no fixed cells. Each
entry is exactly `{ "x": 0, "y": 0, "instruction": { "main": ",", "prefix": null, "suffix": null } }`.
Coordinates are zero-based main-board coordinates within main_board bounds,
with no duplicate positions. Main is a current primary instruction token;
prefix is null or ?0/?1/?2/?!; suffix is null or a legal current suffix.
No Function-board positions or explicit Entry tokens are accepted.
The submitted initial program must contain each exact complete instruction at
that position, otherwise it is ProgramRejected/FixedCellMismatch. This policy
only restricts player editing of the initial program: runtime self modification
can rewrite fixed cells. Existing capability permissions, counts and metrics
still apply without exemptions. Host editors may seed these cells in drafts.

## Initial instruction inventories (2026-10-11)

Optional `program_rules.instruction_limits` is an object from instruction type
to unsigned integer maximum (0..4294967295); absent keys are unlimited.
Example: `{ "DIRECTION": 1, "REGISTER_POINTER": 2 }`. Zero forbids placement.
This counts the whole submitted initial program: Main, Functions, Custom
sources, fixed cells and every Folded Block definition once, including unused
bodies. Explicit Entry is not a direction Primary. Runtime self modification
is allowed to change quantities; permissions still apply to generated code.
Excess inventory is ProgramRejected/InstructionCountExceeded with a path to
the limiting inventory key. Permissions and quantities are independent.

Individual primary and attachment identifiers from the capability vocabulary
are supported; OUTPUT_IMMEDIATE counts all ten immediate forms separately
from OUTPUT. Group members are:

| Group | Members |
| --- | --- |
| DIRECTION | MOVE_UP, MOVE_DOWN, MOVE_LEFT, MOVE_RIGHT |
| REGISTER_POINTER | POINTER_LEFT, POINTER_RIGHT |
| STACK | PUSH, POP_ADD |
| CODEC | DECODE, ENCODE |
| MEMORY | MEMORY_LOAD, MEMORY_STORE, PAGE_INCREMENT, PAGE_DECREMENT |
| PAGE | PAGE_INCREMENT, PAGE_DECREMENT |
| SHIFT | SHIFT_LEFT, SHIFT_RIGHT |
| CALL | CALL, RETURN |
| CUSTOM | CUSTOM, CUSTOM_RETURN |
| UNARY | CLEAR, ADD, SUB, NEG, SHIFT_LEFT, SHIFT_RIGHT |
| BINARY | NAND, CMP |
| SELF_MODIFY | ENCODE, DECODE, READ_CODE, WRITE_CODE |
| PREFIX_CONDITION | CONDITION_0, CONDITION_1, CONDITION_2, CONDITION_FLAG |
| CUSTOMIZATION | CALL, RETURN, CUSTOM, CUSTOM_RETURN, FOLDED_BLOCK |
| SPECIAL | RANDOM_DIRECTION, REPEAT, OUTPUT_IMMEDIATE |

Each matching Primary, prefix or suffix consumes one item. Repeat x2-x9 is one
placed suffix; runtime repetitions do not consume more inventory. All configured
limits apply simultaneously, including overlapping individual and group limits.
The inventory object has at most 64 keys; unknown types are rejected.
