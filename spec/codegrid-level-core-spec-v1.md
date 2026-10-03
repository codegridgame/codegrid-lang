# CodeGrid Level Core Specification v1

**Status:** Final  
**Format:** Markdown  
**Scope:** Rust `level-core` evaluation layer  
**Version:** 1

The [ExactIO implementation contract](codegrid-level-exactio-contract-v1.md)
records the decided v1 schema, capabilities, metric projections, deterministic
configuration, and execution boundaries. Its explicit refinements supersede
the earlier conceptual examples where their details differ.

---

## 1. Purpose

The CodeGrid Level Core defines the deterministic, implementation-level rules used to evaluate a player program against a level.

It is responsible for:

- level identity and versioning;
- allowed language capabilities;
- structural program restrictions;
- Exact I/O test cases;
- Environment-based evaluation;
- correctness;
- runtime failure classification;
- metrics and metric aggregation;
- level constraints;
- scoring metrics;
- star ratings;
- deterministic replay;
- standardized evaluation results.

The Level Core does **not** define presentation content such as:

- display titles;
- descriptions;
- story text;
- localization;
- images;
- animation;
- audio;
- UI layout;
- Steam integration;
- leaderboard upload;
- save-game presentation.

The Rust `level-core` library must depend only on logical level data.

---

## 2. Level File Format

CodeGrid Level Core v1 uses JSON.

A logical level has the following top-level shape:

```json
{
  "format_version": 1,
  "level_id": "official.chapter01.level03",
  "level_version": 1,
  "evaluation_type": "ExactIO",

  "program_rules": {},
  "constraints": {},
  "scoring": {},
  "evaluation": {}
}
```

Logical evaluation data and presentation data must remain separate.

A game client may associate presentation resources with a logical level through `level_id`, but the Rust evaluator must not require presentation resources.

---

## 3. Versioning and Identity

### 3.1 `format_version`

Every level must explicitly declare:

```json
"format_version": 1
```

`format_version` identifies the Level Core schema version.

If the evaluator does not support the specified version, loading must fail with:

```text
UnsupportedFormatVersion
```

The evaluator must not attempt best-effort parsing of an unknown future format.

### 3.2 `level_id`

Every level must have a stable unique identifier.

Example:

```json
"level_id": "official.chapter01.level03"
```

`level_id` is independent of:

- filename;
- display title;
- localization;
- description;
- asset path.

It may be used by:

- save data;
- achievements;
- leaderboards;
- personal-best records;
- server records;
- UGC references.

Changing presentation-only content must not require changing `level_id`.

### 3.3 `level_version`

Every level must have a logical version.

Example:

```json
"level_version": 4
```

`level_version` must be incremented whenever a change can affect evaluation results.

Examples include:

- visible test changes;
- hidden test changes;
- constraints;
- scoring targets;
- scene data;
- Environment maps;
- passenger definitions;
- goals;
- program restrictions.

Presentation-only changes do not require a `level_version` increment.

---

## 4. Evaluation Types

Level Core v1 supports exactly two evaluation types:

```text
ExactIO
Environment
```

A level must explicitly declare one:

```json
"evaluation_type": "ExactIO"
```

A single level has **exactly one** evaluation model.

A level may not mix ExactIO and Environment evaluation within the same level.

The evaluator must never infer the type from other fields.

A future extension point is reserved for:

```text
CustomJudge
```

`CustomJudge` is intentionally **not implemented in v1**.

---

## 5. VM Initial State

A level cannot override the initial state of the CodeGrid VM.

The standard initial state is defined exclusively by the CodeGrid Language / VM specification.

A level must not seed or override:

- current value or registers;
- memory;
- Page;
- offset;
- data stack;
- call stack;
- custom stack;
- execution position;
- direction;
- thread state;
- instruction pointer.

A level supplies runtime data only through:

- ExactIO input;
- Environment observations.

This rule prevents Level files from redefining language semantics.

---

## 6. Program Rules

`program_rules` defines which language features are structurally legal in the level.

Example:

```json
{
  "program_rules": {
    "allowed_instructions": [
      "MOVE_UP",
      "MOVE_DOWN",
      "MOVE_LEFT",
      "MOVE_RIGHT",
      "READ",
      "OUTPUT"
    ],

    "allowed_attachments": [],

    "main_board": {
      "width": 5,
      "height": 5
    },

    "function_board": {
      "width": 5,
      "height": 5
    },

    "max_functions": 0,
    "max_custom": 0,
    "max_threads": 1,
    "memory_enabled": false
  }
}
```

### 6.1 Instruction whitelist

`allowed_instructions` is a whitelist with default permission for the four
direction Primaries, OUTPUT, and HALT. Group names and individual identifiers
are defined by the ExactIO implementation contract. CALL includes RETURN;
CUSTOM includes CUSTOM_RETURN. All other capabilities require explicit permission.

Anything outside the default permissions and explicitly selected capabilities
is forbidden. NAND and CMP remain independent of STACK. Conditional prefixes require per-value `CONDITION_0`, `CONDITION_1`, and `CONDITION_2` permissions in `allowed_attachments`; a false condition does not hide a forbidden Primary. These identifiers and their static/runtime metric rules are defined in the ExactIO implementation contract.

This guarantees that adding a new language instruction in a future game version does not silently make that instruction available in old levels.

### 6.2 Structural restrictions

The Level Core may enforce structural restrictions such as:

- main-board dimensions;
- function-board dimensions;
- maximum function count;
- maximum custom-instruction count;
- maximum thread count;
- whether memory is enabled;
- other fixed capability limits.

### 6.3 Program validation

The player's program must be validated against `program_rules` before test execution.

A structural violation returns:

```text
ProgramRejected
```

Typed reasons may include:

```text
InstructionNotAllowed
TooManyFunctions
TooManyCustomInstructions
ThreadNotAllowed
MemoryNotAllowed
BoardSizeExceeded
```

`ProgramRejected` means the submitted program is structurally illegal for this level.

It is distinct from:

```text
TestFailed
RuntimeError
ConstraintExceeded
```

---

## 7. Level Constraints vs Program Rejection

Program legality and level constraints are separate concepts.

Structural legality belongs to `program_rules` and may reject the program before execution.

Metric-based limits belong to `constraints`.

Examples of metric-based limits include:

```text
max_ticks
max_cost
max_non_empty_cells
max_instruction_kinds
max_functions_used
max_boards_used
```

Even if a metric can be computed statically, a configured metric constraint remains a **Level Constraint**, not a `ProgramRejected` reason.

All configured Level Constraints participate in normal level validation/evaluation semantics.

---

## 8. ExactIO Evaluation

ExactIO is the standard CodeGrid evaluation model.

Conceptually:

```text
Input
  ↓
Player Program
  ↓
Output
  ↓
Expected Output
```

The author defines one or more test cases.

All required tests must pass.

### 8.1 Test-case structure

A test case has the form:

```json
{
  "visible": true,
  "input": [10, 20],
  "expected_output": [30]
}
```

`input` and `expected_output` contain only:

```text
u8
0..255
```

The Level Core attaches no higher-level meaning to those values.

For example, `65` may be displayed elsewhere as `A`, but the evaluator sees only byte value `65`.

### 8.2 Empty arrays

Both arrays may be empty.

Valid examples:

```json
{
  "visible": true,
  "input": [],
  "expected_output": [1]
}
```

```json
{
  "visible": true,
  "input": [1, 2, 3],
  "expected_output": []
}
```

The semantic usefulness of such tests is the author's responsibility.

### 8.3 No per-test overrides

A test case contains test data only.

A test case may not override:

- constraints;
- scoring;
- metric aggregation;
- program rules;
- star targets.

Those are Level-level definitions.

### 8.4 Duplicate tests

The evaluator does not deduplicate tests.

Two tests with identical input and expected output remain two independent tests.

A hidden test may be identical to a visible test.

Test-suite quality is the responsibility of the level author.

### 8.5 No TestGroup / repeat weighting in v1

Level Core v1 does not provide:

- `TestGroup`;
- per-group weighting;
- repeat counts;
- benchmark-balancing machinery;
- special weighting intended to equalize cases such as `99+1` and `1+99`.

The author is responsible for choosing a representative test suite.

---

## 9. Visible and Hidden Tests

Every ExactIO test explicitly declares:

```json
"visible": true
```

or:

```json
"visible": false
```

Visible tests may expose full debugging information.

Hidden tests are used only to verify correctness without exposing test data.

---

## 10. Evaluation Modes

Evaluation mode is a runtime parameter.

It is not stored in the Level file.

Supported modes:

```text
Debug
Official
```

Conceptually:

```rust
evaluate(level, program, EvaluationMode::Debug, options)
```

or:

```rust
evaluate(level, program, EvaluationMode::Official, options)
```

### 10.1 Debug mode

Debug mode executes only visible tests.

All visible tests are executed even if earlier visible tests fail.

This allows one run to return multiple visible failures.

### 10.2 Official mode

Official mode executes:

```text
Visible Tests + Hidden Tests
```

Official mode is fail-fast.

The first failed test ends the official evaluation.

---

## 11. Test Order and Determinism

ExactIO test order is shuffled for every evaluation.

Debug mode:

```text
shuffle(visible_tests)
```

Official mode:

```text
shuffle(visible_tests + hidden_tests)
```

The shuffle is deterministic given a seed.

### 11.1 `shuffle_seed`

The evaluator may generate the seed automatically.

The caller may also supply an explicit seed for:

- replay;
- debugging;
- deterministic CI;
- server reproduction.

The actual seed used must be returned in `EvaluationResult`.

### 11.2 Other randomness

Any future or scene-specific random behavior used by the evaluator must also be deterministic and replayable from recorded seed data.

Randomness must never make a completed evaluation impossible to reproduce.

---

## 12. ExactIO Test Isolation

Every ExactIO test starts with a completely fresh standard VM.

No VM state is shared between tests.

This includes:

- current value;
- registers;
- memory;
- Page;
- offset;
- stacks;
- threads;
- output buffer;
- execution position;
- instruction pointer.

Because each test starts from the same standard state, test order must not affect correctness.

---

## 13. READ Exhaustion

READ exhaustion behavior is fixed by the CodeGrid Language / VM specification.

A Level cannot redefine it.

The Level Core only provides the test's byte input sequence.

The currently defined language semantics remain authoritative, including the rule that exhausted READ does not overwrite the current value.

---

## 14. Exact Output Semantics

### 14.1 Non-empty expected output

For:

```text
expected = [1, 2, 3]
```

actual output must match exactly in:

- value;
- order.

A mismatch fails immediately.

Once the complete expected sequence has been matched, the test passes immediately.

After the final required output:

- remaining input is irrelevant;
- future program instructions are irrelevant;
- HALT is not required.

Example:

```text
expected = [1, 2, 3]
actual   = [1, 2, 3]
```

The third matching output immediately completes the test.

### 14.2 Wrong output

Example:

```text
expected = [1, 2, 3]
actual   = [1, 5]
```

The second output proves the test incorrect.

Result:

```text
TestFailed: WrongOutput
```

### 14.3 Empty expected output

An empty expected output does **not** pass immediately at test start.

For:

```json
{
  "input": [1, 2, 3],
  "expected_output": []
}
```

the program must terminate normally without producing any output.

Any output causes:

```text
TestFailed: WrongOutput
```

Normal termination with no output passes.

### 14.4 HALT

HALT means only:

> stop executing the player program.

HALT is not itself a success condition.

Example:

```text
expected = [1, 2, 3]
actual   = [1, 2]
HALT
```

Result:

```text
TestFailed: IncompleteOutput
```

For:

```text
expected = []
actual   = []
HALT
```

the test passes.

---

## 15. Runtime Errors

Any VM runtime error immediately fails the current test.

Examples include:

```text
OutOfBounds
CustomExecutionLimitExceeded
ConcurrentOutputConflict
```

Use the VM specification's exact runtime error identities. Tick-slice
`TickLimitReached` and work-budget interruption are nonterminal yields, not
runtime errors. A cumulative evaluation resource ceiling has a separate
`ResourceLimitExceeded` outcome and does not synthesize a VM error.

In Official mode, the entire Level evaluation ends immediately.

In Debug mode:

1. the current visible test is recorded as failed;
2. evaluation continues with remaining visible tests.

Runtime errors are distinct from incorrect output and structural rejection.

---

## 16. Failure Information

### 16.1 Visible test failure

A visible test failure may expose complete debugging data:

- input;
- expected output;
- actual output;
- typed failure reason;
- runtime error reason.

Example:

```json
{
  "input": [5, 7],
  "expected_output": [12],
  "actual_output": [11],
  "reason": "WrongOutput"
}
```

### 16.2 Hidden test failure

A hidden test must not expose:

- input;
- expected output;
- actual output;
- test index;
- shuffled position;
- execution order.

A hidden failure may expose only a generic hidden-test failure plus an optional typed category.

Examples:

```text
HiddenTestFailed
WrongOutput
```

```text
HiddenTestFailed
OutOfBounds
```

---

## 17. Hidden Tests, Metrics, Constraints, and Scoring

Hidden tests participate only in correctness.

They do **not** participate in:

- `final_metrics`;
- metric-based Level Constraints;
- scoring;
- star ratings;
- leaderboard results;
- personal-best results.

For ExactIO, official metric aggregation is based only on visible tests.

Therefore hidden-test execution does not contribute to values used by constraints such as:

```text
max_ticks
max_cost
max_memory_addresses
```

Hidden tests remain subject to VM execution limits and the trusted evaluation
safety profile defined in section 34.

This prevents hidden programs from running indefinitely or exceeding core safety limits.

---

## 18. Environment Evaluation

Environment evaluation is used for persistent simulated worlds such as:

- elevators;
- maintenance robots;
- LightBot-style maps;
- future simulated devices.

Example:

```json
{
  "evaluation_type": "Environment",
  "evaluation": {
    "scene_type": "Elevator",
    "scene_data": {},
    "goals": {}
  }
}
```

---

## 19. Scene Types

Every Environment level must explicitly declare a `scene_type`.

Example:

```json
"scene_type": "Elevator"
```

Each scene type defines its own fixed:

- `scene_data` schema;
- Observation protocol;
- Action protocol;
- goal fields;
- scene metrics;
- valid scene-specific constraints.

An unsupported scene type must fail with:

```text
UnsupportedSceneType
```

The evaluator must never fall back to ExactIO.

---

## 20. Environment Data Model

Environment data is conceptually separated into:

```text
scene_data
goals
constraints
```

### 20.1 `scene_data`

Describes the world.

Examples:

- floors;
- passengers;
- map layout;
- repair nodes;
- start position.

### 20.2 `goals`

Describes what must become true for success.

Examples:

```text
all_passengers_delivered
repair_all
reach_exit
```

### 20.3 `constraints`

Describes limits that must remain satisfied.

Examples:

```text
max_stops
max_travel_distance
max_ticks
max_cost
```

Constraints remain Level-level.

---

## 21. Environment Goals

All configured Environment goals use AND semantics.

Example:

```text
repair_all
AND
reach_exit
```

Both must become true.

Arbitrary Boolean goal expressions are not part of v1.

---

## 22. Environment Execution Model

An Environment level is a sequence of dynamic decision steps.

The Environment persists across steps.

The VM does not.

Conceptually:

```text
Initialize Environment
        ↓
Create Observation #1
        ↓
Reset VM
        ↓
Run Player Program
        ↓
Produce Complete Action
        ↓
Discard VM
        ↓
Apply Action to Environment
        ↓
Update Persistent Environment State
        ↓
Create Observation #2
        ↓
Reset VM
        ↓
Run Same Player Program
        ↓
...
```

This repeats until:

- all goals are satisfied;
- a constraint is irreversibly violated;
- an invalid/incomplete action occurs;
- a runtime failure occurs;
- another scene-defined failure occurs.

---

## 23. Environment VM Reset

Every dynamic Environment step starts with the standard VM initial state.

The same player program is executed for every step.

No VM state survives between steps.

Only Environment state persists.

---

## 24. Observation Protocol

Every Environment observation is encoded as:

```text
u8[]
```

The observation becomes the normal CodeGrid input queue.

The player reads it using the standard READ instruction.

There is no scene-specific read channel.

### 24.1 Stable protocol per scene type

Each `scene_type` defines one fixed Observation protocol.

Individual levels of that type cannot redefine it.

All Elevator levels therefore use the same Elevator Observation protocol.

This gives players one stable device interface to learn.

---

## 25. Action Protocol

The player produces Environment actions using normal OUTPUT instructions.

Action values are also:

```text
u8
```

Each scene type defines:

- how many output bytes form one complete action;
- how those bytes are decoded.

Examples:

```text
Elevator:
[floor]
```

Possible future dual-elevator scene:

```text
[elevator_id, floor]
```

Maintenance Robot:

```text
[action_code]
```

---

## 26. Dynamic Step Completion

As soon as a complete Action has been produced, the current dynamic step ends immediately.

The remainder of the player's program is not executed for that step.

The VM is discarded.

The Environment applies the Action and advances to the next persistent world state.

---

## 27. Incomplete and Invalid Actions

### 27.1 Incomplete action

If the program terminates before producing the required number of Action bytes:

```text
TestFailed: IncompleteAction
```

### 27.2 Invalid action

If the Action has the correct shape but contains an illegal value:

```text
TestFailed: InvalidAction
```

Example:

An elevator supports floors `1..9`, but the program outputs:

```text
12
```

This is an invalid scene action.

It is not:

```text
RuntimeError
```

and not:

```text
ProgramRejected
```

---

## 28. Environment Completion and Failure

If all configured goals become true:

```text
Passed
```

immediately.

HALT is not required.

If a constraint becomes irreversibly violated, the level fails immediately.

Example:

```text
max_stops = 5
stop_count = 6
```

No additional dynamic steps are executed.

---

## 29. Metrics

Metrics are divided into:

```text
VM Metrics
Scene Metrics
```

### 29.1 VM Metrics

VM Metrics are predefined by the runtime.

Examples may include:

```text
ticks
cost
instruction_kinds
max_data_stack_depth
max_call_stack_depth
max_custom_stack_depth
memory_addresses_used
non_empty_cells
boards_used
```

Their exact definitions belong to the VM Metrics specification.

### 29.2 Scene Metrics

Each Environment scene type defines its own fixed scene metric vocabulary.

Example Elevator metrics:

```text
stop_count
travel_distance
passengers_delivered
```

Example MaintenanceRobot metrics:

```text
action_count
move_count
turn_count
repair_count
```

A Level cannot invent arbitrary new Scene Metric names.

---

## 30. Metric Aggregation

Each predefined Metric has one system-defined aggregation semantic.

A Level cannot override it.

Typical categories include:

### SUM

Examples:

```text
ticks
execution_count
```

### MAX

Examples:

```text
max_data_stack_depth
max_call_stack_depth
max_custom_stack_depth
memory_addresses_used
```

### STATIC

Examples:

```text
non_empty_cells
instruction_kinds
boards_used
```

The exact metric-to-aggregation mapping must be defined centrally by the runtime.

---

## 31. Metric Scope

### 31.1 ExactIO

For ExactIO:

- every test uses a fresh VM;
- visible-test metrics are aggregated at Level scope;
- hidden-test metrics do not participate in official aggregates.

### 31.2 Environment

Every dynamic Environment step produces normal VM Metrics.

Those metrics are aggregated across the entire Environment run using the fixed aggregation semantics.

Scene Metrics are produced by the persistent Environment.

Final Environment metrics consist of:

```text
Aggregated VM Metrics
+
Scene Metrics
```

---

## 32. Constraints

A Level may define constraints over predefined VM Metrics and Scene Metrics.

Example:

```json
{
  "constraints": {
    "max_ticks": 500,
    "max_cost": 30,
    "max_stops": 5,
    "max_travel_distance": 20
  }
}
```

All configured constraints use AND semantics.

Every configured constraint must be satisfied.

There is no arbitrary AND/OR expression system in v1.

---

## 33. Constraint Scope

Constraints operate on Level-level metric values.

They are not defined separately per ExactIO test case or per Environment decision step.

For ExactIO, metric-based constraints use only visible-test metric aggregation.

For Environment, constraints use the final/current Level aggregate across dynamic steps and scene state.

If an Environment constraint becomes irreversibly violated during execution, evaluation may fail immediately.

---

## 34. VM Execution Limits and Evaluation Safety

The VM specification owns its execution limits and error/yield behavior.
Custom execution-limit exhaustion is a runtime error. Bounded tick/work
exhaustion is nonterminal and does not define universal memory, thread, or
stack quotas. Do not infer global language quotas from host policy.

The embedding evaluator uses an explicit immutable trusted safety profile,
including cumulative limits and resource ceilings. A Level may never relax
that profile. Safety ceilings apply to visible and hidden tests; host resource
termination is distinct from a VM runtime error and logical level constraint.

A Level may impose stricter metric constraints, with the visible-only ExactIO
scope defined in section 33. The ExactIO implementation contract defines the
separate Pending and ResourceLimitExceeded outcomes and completion boundaries.

---

## 35. Failed Evaluations and Metrics

A failed solution does not produce official `final_metrics`.

The evaluator may return:

```text
partial_metrics
```

for debugging.

Partial metrics must not be used for:

- personal bests;
- leaderboards;
- official scoring;
- star ratings.

`final_metrics` are produced only after:

1. all required correctness checks pass; and
2. all configured Level Constraints pass.

---

## 36. Scoring Metrics

A Level explicitly chooses which predefined metrics are official scoring metrics.

Example:

```json
{
  "scoring": {
    "metrics": {
      "ticks": {
        "target": 200
      },
      "cost": {
        "target": 20
      }
    }
  }
}
```

Environment example:

```json
{
  "scoring": {
    "metrics": {
      "stop_count": {
        "target": 4
      },
      "ticks": {
        "target": null
      }
    }
  }
}
```

There is no `primary_metric`.

All scoring metrics are peers.

There is no weighted composite score in v1.

---

## 37. Scoring Metrics Without Rating Targets

A scoring metric does not have to define a star-rating target.

Example:

```json
"ticks": {
  "target": null
}
```

Such a metric may still be:

- displayed;
- stored;
- used for personal best;
- used for leaderboard ranking.

It does not affect star rating.

---

## 38. Metric Optimization Direction

Every predefined Metric has a fixed optimization direction.

Typical examples:

```text
ticks                  minimize
cost                   minimize
memory_addresses_used  minimize
stop_count             minimize
travel_distance        minimize
```

A future metric may be:

```text
throughput             maximize
```

A Level cannot redefine optimization direction.

---

## 39. Star-Rating Targets

For each scoring metric that participates in rating, the author supplies exactly one target.

That value is the **3-star target**.

For a minimize metric:

```text
★★★ if value <= target
```

For a maximize metric:

```text
★★★ if value >= target
```

Targets apply to the final Level-level aggregated Metric, not to individual test cases.

---

## 40. Two-Star Threshold

The 2-star threshold is derived automatically using a fixed system-wide ratio of:

```text
1.5
```

For a minimize metric:

```text
two_star_threshold = ceil(target * 1.5)
```

For a maximize metric:

```text
two_star_threshold = floor(target / 1.5)
```

Examples:

```text
Minimize target = 100

★★★ <= 100
★★  <= 150
```

```text
Maximize target = 100

★★★ >= 100
★★  >= 66
```

The rounding direction is intentionally player-favorable.

---

## 41. One-Star Rule

A successfully completed **rated** level always earns at least:

```text
★
```

There is no independent 1-star threshold.

---

## 42. Overall Star Rating

If several scoring metrics have rating targets, each produces its own internal star result.

The overall Level star rating is:

```text
minimum rating across all rated metrics
```

Example:

```text
ticks   ★★★
cost    ★★
memory  ★★★
```

Overall:

```text
★★
```

No weighted numerical total is calculated.

---

## 43. Levels Without Rating Targets

If no scoring metric has a rating target, the Level has no star rating.

Passing such a Level does not automatically produce one star.

Raw metrics may still be:

- shown;
- stored;
- compared;
- ranked.

This is suitable for tutorial or non-rated Levels.

---

## 44. Scoring Targets vs Constraints

The Level Core does not reject a Level merely because its scoring targets and constraints create weak, trivial, or unreachable rating behavior.

Example:

```text
max_ticks = 80
3-star ticks target = 100
```

Any valid solution satisfying the constraint will necessarily receive 3 stars for ticks.

This is legal.

An editor may warn about such configurations, but the Core must not reject them.

Level-design quality remains the author's responsibility.

---

## 45. Static Level Validation

Every Level must be statically validated before evaluation.

Conceptually:

```rust
validate_level(level)
    -> Result<ValidatedLevel, LevelError>
```

Validation should include at least:

- supported `format_version`;
- valid `level_id`;
- valid `level_version`;
- supported `evaluation_type`;
- valid instruction identifiers;
- valid Program Rules;
- valid ExactIO test structure;
- byte-range validation;
- supported `scene_type`;
- scene-specific schema validation;
- valid goals;
- valid constraint metric names;
- valid scoring metric names;
- valid rating targets.

A malformed or semantically invalid Level returns:

```text
LevelInvalid
```

This is not a player failure.

---

## 46. Unsupported Types and Versions

Unsupported format and scene capabilities must remain distinguishable.

Examples:

```text
UnsupportedFormatVersion
UnsupportedSceneType
```

They must not be silently converted to:

```text
LevelInvalid
```

or:

```text
TestFailed
```

when the actual problem is missing runtime support.

---

## 47. Safe Defaults

Fields that materially affect evaluation semantics should be explicit.

The Level format should avoid broad implicit defaults.

For example, the evaluator should not silently assume:

```text
missing max_threads -> 1
```

unless that behavior is formally defined by the schema.

Only small, safe, unambiguous defaults may be introduced.

Semantic fields should generally remain explicit.

---

## 48. Public Rust API

The public API should expose one unified evaluation entry point.

Conceptually:

```rust
evaluate(
    level: &ValidatedLevel,
    program: &Program,
    mode: EvaluationMode,
    options: EvaluationOptions,
) -> EvaluationResult
```

Internal implementations may dispatch to modules such as:

```text
ExactIOEvaluator
EnvironmentEvaluator
```

but callers should not require separate top-level APIs.

---

## 49. Evaluation Options

Conceptually:

```rust
struct EvaluationOptions {
    shuffle_seed: Option<u64>,
}
```

If no seed is supplied, the evaluator generates one.

Future deterministic scene seeds may be added without changing the core requirement that all randomness remain replayable.

---

## 50. Evaluation Result

All evaluation types return one unified top-level result.

A conceptual shape is:

```rust
struct EvaluationResult {
    status: EvaluationStatus,

    failure: Option<EvaluationFailure>,

    partial_metrics: Option<Metrics>,
    final_metrics: Option<Metrics>,

    rating: Option<StarRating>,

    shuffle_seed: Option<u64>,

    exact_io: Option<ExactIOResult>,
    environment: Option<EnvironmentResult>,
}
```

The actual Rust implementation may use enums rather than optional fields.

The external conceptual model remains unified.

---

## 51. Evaluation Status and Failure Classes

The evaluator must preserve meaningful failure classes.

At minimum, the system must distinguish:

```text
Passed
ProgramRejected
LevelInvalid
TestFailed
RuntimeError
UnsupportedFormatVersion
UnsupportedSceneType
```

Useful typed sub-reasons include:

```text
WrongOutput
IncompleteOutput
IncompleteAction
InvalidAction
OutOfBounds
CustomExecutionLimitExceeded
ConcurrentOutputConflict
ResourceLimitExceeded
InstructionNotAllowed
TooManyFunctions
ThreadNotAllowed
MemoryNotAllowed
BoardSizeExceeded
```

Different failure origins must not be collapsed into one generic:

```text
EvaluationFailed
```

---

## 52. ExactIO Example

```json
{
  "format_version": 1,
  "level_id": "official.addition.01",
  "level_version": 1,

  "evaluation_type": "ExactIO",

  "program_rules": {
    "allowed_instructions": [
      "MOVE_UP",
      "MOVE_DOWN",
      "MOVE_LEFT",
      "MOVE_RIGHT",
      "READ",
      "OUTPUT",
      "ADD",
      "SUB"
    ],

    "allowed_attachments": [],

    "main_board": {
      "width": 5,
      "height": 5
    },

    "function_board": {
      "width": 5,
      "height": 5
    },

    "max_functions": 0,
    "max_custom": 0,
    "max_threads": 1,
    "memory_enabled": false
  },

  "constraints": {
    "max_ticks": 1000
  },

  "scoring": {
    "metrics": {
      "ticks": {
        "target": 200
      },
      "cost": {
        "target": 20
      }
    }
  },

  "evaluation": {
    "tests": [
      {
        "visible": true,
        "input": [1, 2],
        "expected_output": [3]
      },
      {
        "visible": true,
        "input": [99, 1],
        "expected_output": [100]
      },
      {
        "visible": false,
        "input": [1, 99],
        "expected_output": [100]
      }
    ]
  }
}
```

In Official evaluation:

- all three tests participate in correctness;
- only the two visible tests contribute to official metrics;
- only visible-test metrics are used by metric-based Level Constraints;
- only visible-test metrics are used for scoring and star rating.

---

## 53. Elevator Environment Example

```json
{
  "format_version": 1,
  "level_id": "official.elevator.01",
  "level_version": 1,

  "evaluation_type": "Environment",

  "program_rules": {
    "allowed_instructions": [
      "MOVE_UP",
      "MOVE_DOWN",
      "MOVE_LEFT",
      "MOVE_RIGHT",
      "READ",
      "OUTPUT"
    ],

    "allowed_attachments": [],

    "main_board": {
      "width": 5,
      "height": 5
    },

    "function_board": {
      "width": 5,
      "height": 5
    },

    "max_functions": 0,
    "max_custom": 0,
    "max_threads": 1,
    "memory_enabled": false
  },

  "constraints": {
    "max_stops": 5
  },

  "scoring": {
    "metrics": {
      "stop_count": {
        "target": 4
      },
      "ticks": {
        "target": null
      }
    }
  },

  "evaluation": {
    "scene_type": "Elevator",

    "scene_data": {
      "floor_count": 9,
      "initial_floor": 1,

      "passengers": [
        {
          "from": 2,
          "to": 6
        },
        {
          "from": 7,
          "to": 3
        },
        {
          "from": 4,
          "to": 8
        }
      ]
    },

    "goals": {
      "all_passengers_delivered": true
    }
  }
}
```

The author defines complete passenger state:

```text
2 -> 6
7 -> 3
4 -> 8
```

The player initially receives only currently observable requests, encoded through the fixed Elevator Observation protocol.

Conceptually this may represent:

```text
2 Up
4 Up
7 Down
```

The player outputs the next destination, for example:

```text
OUTPUT 2
```

The Environment then:

1. applies the action;
2. moves the elevator;
3. boards/alights passengers according to scene rules;
4. updates persistent state;
5. exposes newly relevant internal requests;
6. discards the VM;
7. creates the next observation;
8. resets the VM;
9. runs the same player program again.

This repeats until all passengers are delivered or the Level fails.

---

## 54. Maintenance Robot Example

```json
{
  "format_version": 1,
  "level_id": "official.maintenance.01",
  "level_version": 1,

  "evaluation_type": "Environment",

  "program_rules": {
    "allowed_instructions": [
      "MOVE_UP",
      "MOVE_DOWN",
      "MOVE_LEFT",
      "MOVE_RIGHT",
      "READ",
      "OUTPUT"
    ],

    "allowed_attachments": [],

    "main_board": {
      "width": 5,
      "height": 5
    },

    "function_board": {
      "width": 5,
      "height": 5
    },

    "max_functions": 0,
    "max_custom": 0,
    "max_threads": 1,
    "memory_enabled": false
  },

  "constraints": {
    "max_actions": 20
  },

  "scoring": {
    "metrics": {
      "action_count": {
        "target": 12
      }
    }
  },

  "evaluation": {
    "scene_type": "MaintenanceRobot",

    "scene_data": {
      "map": {},
      "start": {},
      "targets": []
    },

    "goals": {
      "repair_all": true,
      "reach_exit": true
    }
  }
}
```

The exact map schema and byte protocols belong in a separate:

```text
MaintenanceRobot Scene Specification
```

---

## 55. Presentation Data

Presentation data is outside Level Core.

Examples include:

```text
title
description
tutorial text
story text
localization
background image
scene artwork
audio
UI configuration
```

The game may associate presentation data with the logical Level through `level_id`.

The Rust evaluator must not require these resources.

---

## 56. Features Explicitly Outside v1

The following are intentionally excluded from Level Core v1:

```text
CustomJudge execution
custom Scene Metrics
custom Metric aggregation rules
custom Metric optimization direction
arbitrary Goal Boolean expressions
arbitrary Constraint Boolean expressions
per-Test constraints
per-Test scoring overrides
TestGroup / repeat weighting
benchmark-balancing machinery
Level-defined VM initial state
Level-defined VM semantic overrides
```

These may be introduced in future `format_version`s.

---

## 57. Core Evaluation Pipeline

The intended top-level pipeline is:

```text
Load Level JSON
        ↓
Check format_version
        ↓
validate_level()
        ↓
ValidatedLevel
        ↓
Validate Player Program
        ↓
ProgramRejected?
        ↓ no
Dispatch by evaluation_type
        ↓
ExactIO or Environment Evaluation
        ↓
Collect Metrics
        ↓
Check Correctness
        ↓
Check Level Constraints
        ↓
Passed?
        ↓
Produce final_metrics
        ↓
Calculate Optional Star Rating
        ↓
Return EvaluationResult
```

For ExactIO Official evaluation:

```text
Visible + Hidden Tests determine correctness

Visible Tests determine:
- official metrics
- metric-based Level Constraints
- scoring
- star rating
- personal-best values
- leaderboard values

Hidden Tests determine:
- correctness only

All Tests remain subject to VM execution limits and the trusted evaluation safety profile
```

This separation is normative for Level Core v1.

---

# Appendix A — Normative Summary

The following requirements are normative:

1. A Level uses exactly one evaluation model.
2. ExactIO and Environment cannot be mixed in one Level.
3. Every test/Environment step starts from the standard VM initial state.
4. Level files cannot override VM initial state or language semantics.
5. ExactIO input/output values are bytes (`u8`).
6. ExactIO tests are isolated and independently reset.
7. Test order is shuffled deterministically from a recorded seed.
8. Debug runs all visible tests.
9. Official runs visible + hidden tests and is fail-fast.
10. Hidden tests participate only in correctness.
11. Hidden tests do not affect official metrics, metric-based constraints, scoring, rating, PBs, or leaderboards.
12. Hidden tests remain subject to VM execution limits and the trusted evaluation safety profile.
13. ExactIO passes immediately after a complete non-empty expected output has been matched.
14. Empty expected output requires normal termination with no output.
15. HALT is not itself a success condition.
16. Environment state persists across dynamic decision steps.
17. VM state does not persist across Environment steps.
18. Environment observations use the normal `u8[]` input queue.
19. Environment actions use normal OUTPUT bytes.
20. Observation and Action protocols are fixed by `scene_type`.
21. Invalid scene actions are `TestFailed`, not RuntimeError or ProgramRejected.
22. All configured goals use AND semantics.
23. All configured constraints use AND semantics.
24. Constraints are Level-level and use Level-level aggregated Metrics.
25. Metric aggregation semantics are system-defined.
26. Scene Metric vocabularies are scene-defined and cannot be invented per Level.
27. Failed solutions do not produce official final metrics.
28. Scoring Metrics are explicitly selected by the Level.
29. A scoring Metric may exist without participating in star rating.
30. Metric optimization direction is fixed by the system.
31. There is no primary metric.
32. There is no weighted combined score.
33. Authors provide one 3-star target per rated Metric.
34. 2-star threshold uses the fixed 1.5 ratio.
35. Minimize 2-star thresholds use `ceil(target * 1.5)`.
36. Maximize 2-star thresholds use `floor(target / 1.5)`.
37. Passing a rated Level guarantees at least 1 star.
38. Overall Level rating is the minimum rating among all rated Metrics.
39. A Level with no rating targets has no star rating.
40. Core validation does not reject weak or trivial scoring configurations solely because they are poor level design.
41. Duplicate tests are permitted and are not deduplicated.
42. v1 has no TestGroup, repeat weighting, or benchmark-balancing system.
43. Program structural legality is separate from Level Metric Constraints.
44. Unsupported format versions and scene types must be reported explicitly.
45. CustomJudge is reserved for a future version and is not implemented in v1.

---

# Appendix B — Recommended Module Boundaries

A practical Rust implementation may be organized roughly as:

```text
level-core/
├── schema/
│   ├── level.rs
│   ├── program_rules.rs
│   ├── exact_io.rs
│   ├── environment.rs
│   ├── constraints.rs
│   └── scoring.rs
│
├── validate/
│   ├── level.rs
│   ├── program.rs
│   └── scene.rs
│
├── evaluate/
│   ├── mod.rs
│   ├── exact_io.rs
│   └── environment.rs
│
├── metrics/
│   ├── definitions.rs
│   ├── aggregation.rs
│   └── scoring.rs
│
├── scenes/
│   ├── elevator/
│   └── maintenance_robot/
│
├── result/
│   ├── status.rs
│   ├── failure.rs
│   └── evaluation_result.rs
│
└── lib.rs
```

This module structure is informative rather than normative. The behavioral rules in the main specification are normative.
