# Level Core v1 ExactIO Implementation Contract

Status: decided for the first implementation phase on 2026-09-30.

This contract supplements [Level Core v1](codegrid-level-core-spec-v1.md).
The user selected dynamic `cost`, explicit Exit/Wrap configuration, WriteCode
support, and enforcement of the whitelist on generated code, then authorized
the recommended remaining choices. Source acceptance and VM transitions remain
defined by their existing specifications. Environment scene protocols and the
external level ABI are independently defined by the [Level transports
v1 contract](../docs/level-wasm-v1.md); this document does not define production
scene protocols.

## 1. Level loading and numeric domains

Reject duplicate JSON object keys, unknown fields, incorrect types, unsupported
evaluation types, and numeric values outside the declared domain. Check
`format_version` before interpreting the version-specific payload. Unsupported
format and scene types retain their dedicated categories. Other malformed data
is `LevelInvalid` with a typed reason and JSON field path.

`format_version` is the JSON integer 1. `level_id` is a nonempty Unicode string
with no leading/trailing whitespace; preserve its exact spelling without case
folding or normalization. `level_version` is an integer in `1..=u32::MAX`.
Booleans are not integers. Arrays contain bytes only where byte data is required.

Require `program_rules`, `constraints`, `scoring`, and `evaluation`, even when
constraints or scoring metrics are empty. Require every capability field:
`allowed_instructions`, `allowed_attachments`, `main_board`, `function_board`,
`max_functions`, `max_custom`, `max_threads`, and `memory_enabled`.
Empty capability arrays are legal; reject duplicate capability identifiers.
Direction Primaries, OUTPUT, and HALT are always allowed. Other permissions
are explicit, including the grouped capabilities below.

Board dimensions are positive `u32` values within the existing portable board
geometry bounds. They are upper bounds, not exact required dimensions.
`max_functions` is `0..=10` per code grid; `max_custom` is `0..=10` in the outer
program; `max_threads` is a positive `u32` maximum Entry-thread count per code
grid. This counts the outer Main Entries and each Custom Main's Entries
separately, not the aggregate of resident outer and internal threads.

Apply Main and Function dimension bounds in both outer and Custom code grids.
Folded Blocks use their owner's geometry according to the language spec;
their shell/body cells are still inspected for program rules. Count all defined
Functions, Customs, and code cells, including unreachable definitions.
`memory_enabled: false` forbids MEMORY_LOAD, MEMORY_STORE, PAGE_INCREMENT,
and PAGE_DECREMENT in every context. Registers and stacks remain separate
capabilities controlled by their instruction identifiers.

ExactIO requires a nonempty `tests` array and at least one visible test.
Every test has exactly `visible`, `input`, and `expected_output`; both byte
arrays may be empty. Duplicate tests remain independent. An omitted field is
invalid. `scoring` has exactly a `metrics` object, which may be empty.
Each scoring entry has exactly `target`, either null or a nonnegative integer.

Metric limits and targets are integers in `0..=u64::MAX` in logical JSON.
The Rust loader parses them without floating-point conversion. Host APIs
transport these values using canonical decimal strings or pass original level
JSON text unchanged; JavaScript must not parse/re-serialize wide values through
Number. Aggregate overflow is a distinct evaluator fault, never wrapping,
saturation, `TestFailed`, or `ConstraintExceeded`.

## 2. Fixed capability vocabulary

Following the explicit 2026-10-02 user decision, grouped names are accepted:

| Group | Allowed Primaries |
| --- | --- |
| IF_ZERO | IF_ZERO_UP, IF_ZERO_DOWN, IF_ZERO_LEFT, IF_ZERO_RIGHT |
| READ | READ_UP, READ_DOWN, READ_LEFT, READ_RIGHT |
| REGISTER_POINTER | POINTER_LEFT, POINTER_RIGHT |
| STACK | PUSH, POP_ADD |
| CODEC | DECODE, ENCODE |
| MEMORY | MEMORY_LOAD, MEMORY_STORE |
| PAGE | PAGE_INCREMENT, PAGE_DECREMENT |
| SHIFT | SHIFT_LEFT, SHIFT_RIGHT |
| CALL | Every valid Function Slot invocation and RETURN |
| CUSTOM | Every valid Custom Slot invocation and CUSTOM_RETURN |

The four MOVE directions, OUTPUT, and HALT are always permitted, even with an
empty whitelist. RANDOM_DIRECTION remains explicit. CLEAR, ADD, SUB, and
FOLDED_BLOCK remain independent capabilities. Existing individual names below
remain accepted for compatibility and precise permissions. Groups and individual
names form a union; an explicitly listed default permission is redundant but
valid. `memory_enabled: false` still prohibits MEMORY and PAGE operations.
NAND remains an independent capability, separate from STACK. Attachments remain
independent. Apply the same permissions to initial and committed generated code.
This changes permission grouping only; static `instruction_kinds` measurement
and VM dynamic Instruction Variety remain unchanged.

Use the following fixed v1 individual names. Match the canonical Rust model variants,
not a second token parser. Parameterized family names permit every valid Slot;
the compiler still verifies references. Future instructions are forbidden
until explicitly added to a later capability contract.

| Identifier | Canonical Primary |
| --- | --- |
| MOVE_UP, MOVE_DOWN, MOVE_LEFT, MOVE_RIGHT | Direction of the named orientation |
| RANDOM_DIRECTION | RandomDirection |
| IF_ZERO_UP, IF_ZERO_DOWN, IF_ZERO_LEFT, IF_ZERO_RIGHT | IfZero of the named orientation |
| READ | All four Read orientations |
| READ_UP, READ_DOWN, READ_LEFT, READ_RIGHT | Only the named Read orientation |
| CLEAR, ADD, SUB | Clear, Add, Sub |
| POINTER_LEFT, POINTER_RIGHT | MoveRegisterPointer of the named orientation |
| OUTPUT, PUSH, POP_ADD, DECODE, ENCODE | Corresponding model variants |

| CALL, RETURN | Call of any valid Slot, Return |
| NAND, MEMORY_LOAD, MEMORY_STORE | Nand, MemoryLoad, MemoryStore |
| PAGE_INCREMENT, PAGE_DECREMENT | MovePage of the named orientation |
| SHIFT_LEFT, SHIFT_RIGHT | Shift of the named orientation |
| FOLDED_BLOCK, CUSTOM, CUSTOM_RETURN, HALT | Corresponding model variants; Slot families allow any valid Slot |

OUTPUT includes both register output `.` and immediate outputs `.0` through
`.9`. These forms share the OUTPUT permission and one static instruction kind.

`READ` and directional READ names may coexist; membership is the union of
their permitted variants. Empty and Entry are structural cells and need no
instruction permission. Folded Block and Custom invocation shells do require
their named permissions. Function/Custom definitions remain subject to counts
and their bodies are checked regardless of invocation permissions.

`allowed_attachments` independently permits `READ_CODE`, `WRITE_CODE`, and
`REPEAT`. REPEAT permits only counts accepted by the source/IR specification.
Primary permission never implies Attachment permission. Attachments still
obey the existing Primary compatibility matrix. Validation is purely structural;
even inert RETURN attachments require permission.

WriteCode cannot bypass program rules. After each successful VM tick, the Rust
level evaluator validates mutable Primary values in every returned execution
context, including committed Custom internal changes. If a forbidden Primary
or memory capability is detected, terminate the whole evaluation as
`ProgramRejected` with reason `GeneratedInstructionNotAllowed` or
`GeneratedMemoryNotAllowed`, before accepting output or computing a rating.
Do not roll back the VM's committed tick or invent a VM error. Empty writes
are permitted. Fixed Attachments are not modified by WriteCode.

The detection must include internal Custom writes even if the invocation is
discarded in that outer tick. A final outer snapshot alone is insufficient.
Use committed scoped CodeChanged events from the existing VM API; event payload
adequacy is an implementation check, not permission to change VM semantics.
Also inspect the current mutable outer program after commit. Rollback-only
writes produce no committed events and no generated-code rejection; the VM
error/fault remains the failure. Do not check output until this guard succeeds.
Events and diagnostic details from hidden tests are never exposed publicly.

## 3. Metric registry

All metrics below are nonnegative integers and minimize for scoring. The
static metrics refer to initial verified IR, including unreachable definitions;
self-modification does not change them. The evaluator owns these projections,
and does not redefine VM raw metrics.

| Metric | Measurement | Across eligible tests/steps | Constraint |
| --- | --- | --- | --- |
| ticks | Committed Global Tick | SUM | max_ticks |
| cost | VM Operation Count, including Custom work and metric-bearing Attachments | SUM | max_cost |
| operation_count | Exact alias of cost | SUM | max_operation_count |
| memory_addresses_used | Cardinality of VM Used Memory Addresses | MAX | max_memory_addresses |
| max_data_stack_depth | VM Peak Data Stack Usage | MAX | max_data_stack_depth |
| max_instruction_stack_depth | VM Peak Instruction Stack Usage | MAX | max_instruction_stack_depth |
| max_call_stack_depth | VM Peak Call Stack Usage | MAX | max_call_stack_depth |
| non_empty_cells | Cells with an initial Primary or Entry, including structural invocation shells and Folded Block bodies; an Attachment adds no extra cell | STATIC | max_non_empty_cells |
| instruction_kinds | Distinct permitted Primary capability kinds present in initial code, with all READ orientations one kind; count each distinct Attachment kind as well; exclude Empty and Entry | STATIC | max_instruction_kinds |
| functions_used | Number of defined Functions across outer and Custom code grids | STATIC | max_functions_used |
| boards_used | Number of Main, Function, and Folded Block boards across outer and Custom code grids | STATIC | max_boards_used |

For static instruction kinds, each direction-sensitive name is a distinct
kind except READ; Slot values do not create distinct CALL/CUSTOM/FOLDED_BLOCK
kinds. REPEAT counts as one static kind independent of its count. This is a
static program metric, not the VM's dynamic Instruction Variety.
`functions_used` and `boards_used` intentionally count definitions, not visited
runtime identities; these fixed meanings prevent test-dependent static metrics.

The earlier illustrative `max_custom_stack_depth` has no distinct VM raw stack
metric and is unsupported in this contract; reject it rather than inventing a
stack. Custom stack usage already contributes to the normative VM peaks.
Scene metrics such as `stop_count` are invalid for ExactIO. All unregistered
metric/constraint names are `LevelInvalid`.

ExactIO aggregates visible tests only, including repeated tests. Environment
will aggregate every decision step using this registry plus its scene registry.
Aliases are accepted as separate peer scoring entries; if both cost and
operation_count are selected, they refer to the same value and each configured
target/constraint applies. No alias introduces extra execution or weighting.

Compute two-star thresholds in a widened integer domain: minimize uses
`ceil(3 * target / 2)`, maximize uses `floor(2 * target / 3)` if a future metric
supports that direction. Do not overflow a u64 intermediate or clamp thresholds.
Failed evaluations have no final metrics or rating. Visible partial metrics
may include VM attempted-work counters under the VM's rollback metric rules.

## 4. Determinism and host configuration

The level API requires explicit `boundary_mode: Exit | Wrap`, positive Custom
execution limit, and an immutable trusted safety profile. These are evaluation
configuration, not level initial-state overrides. Include them in result and
replay identity; server ranking partitions must use the same profile and
boundary mode. A caller cannot relax trusted ceilings.

The API accepts optional `shuffle_seed: u64`. When omitted, resolve it through
an explicit host seed source before starting the kernel. Use the returned root
seed for deterministic shuffle and derive one VM seed shared by every fresh
test VM: `mix64(root_seed XOR 0x43474C564D303031)`, using VM section 13's mix64.
Sharing a test VM seed avoids making output depend on shuffle position.
Environment scene streams require their own later protocol decision.

Shuffle the selected tests in source order using descending Fisher-Yates.
Use the VM section 13 generator convention: state initially equals root_seed;
each draw sets `state = mix64(state)` and returns that new state. This specifies
the recurrence explicitly rather than relying on another library's SplitMix64
convention. For bound `b = i + 1`, draw x until
`x >= (2^64 mod b)`, then choose `j = x mod b` and swap i with j. All generator
state arithmetic wraps modulo 2^64; all indexing conversions are checked.
Use independent RNG state for shuffling and VM execution. Publish golden
vectors in focused implementation tests before claiming determinism acceptance.

The trusted safety profile specifies positive per-test committed-tick and
cumulative-evaluation work ceilings, a per-call work ceiling, and explicit
resource ceilings for input, level/program sizes, state, handles, and responses.
Do not infer universal limits from the VM or publish guessed production values.
Profiles are versioned input data and part of official result identity.
The [safety accounting contract](../docs/level-safety-accounting.md) records
the implementation refinement: retained representation bytes and deterministic
VM storage units are distinct from physical heap memory. The named local
profile and Level API/transport v1 contracts are published independently from
the existing language host versions.
Safety ceilings apply to hidden tests as well; level metric constraints do not.
Count actual dispatched work for host safety, including retries, separately
from normative VM metrics. If the VM facade does not expose that count, add a
neutral host accounting surface without changing language work definitions.

Tick/work call exhaustion is nonterminal Pending. Work interruption rejects
the entire attempted tick. Increase retry budgets within trusted ceilings when
necessary; never resume a partially evaluated tick. If safety ceilings prevent
progress, report `ResourceLimitExceeded`, distinct from VM RuntimeError,
logical ConstraintExceeded, and incorrect output. Cancellation and evaluator/VM
faults are also separate host outcomes with no official final metrics or rating.

## 5. Terminal tick and failure priority

For each VM step:

1. A VM fault ends evaluation as a fault. A VM runtime error fails the test
   using its original stable identity; failed tick output is absent. Debug
   continues to other visible tests, while Official stops.
2. A work interruption leaves evaluation Pending unless a trusted cumulative
   safety ceiling is exhausted. It contributes no normative interrupted-tick
   metrics or output.
3. After successful commit, update permitted metrics and apply the generated
   instruction guard. A violation ends the entire evaluation as ProgramRejected.
4. Check newly committed output in order. Wrong output is TestFailed. Complete
   nonempty expected output establishes correctness; normal termination with
   empty output establishes correctness for empty expectations. Normal HALT
   with missing output is IncompleteOutput. VM error priority still applies.
5. Check visible-only level aggregates against every configured constraint.
   Correct output cannot override ConstraintExceeded. For Debug, a level-wide
   constraint breach does not prevent executing the remaining visible tests.
   Official stops on the first established failure, including a constraint
   breach. When output failure and constraint breach coincide, output failure
   is the primary reason; neither can produce final metrics.
6. At the exact safety ceiling, allow a completed test/evaluation that needs
   no further work to finish. If work is still required, return the distinct
   resource-limit outcome. Check level constraints before awarding any success.

Check static constraints after structural acceptance. In Official, a failed
static constraint can stop evaluation before creating a VM. Debug still runs
all visible tests for feedback and reports the overall constraint failure.
Final metrics and rating require all selected correctness checks and constraints
to pass. Debug success is not a certification that hidden tests passed.

This contract does not specify Environment goal/constraint priority; define
that when the first complete scene protocol is introduced.

## 6. Retained feedback and response accounting refinement

The host reserves an explicit conservative encoded feedback/result allowance
before creating an evaluation handle. The resolved kernel configuration includes
this allowance, independently of VM output/state units and player constraints.
Check the next visible feedback record before retaining its input, expected or
actual output, and diagnostic payload. Exhaustion is a host ResourceLimitExceeded
outcome, with no final metrics or rating; hidden tests never contribute feedback.
The host checks projection bounds before creating JSON result/diagnostic
collections and uses bounded serialization rather than allocating an oversized
response and truncating it. The fixed v1 accounting formula and reservation
lifecycle are defined in [level safety accounting](../docs/level-safety-accounting.md).

The opaque session retains committed generated-code rejection scope, static
cell, Primary, and trusted test identity internally. Public results redact these
details for hidden tests. This refinement changes host resource accounting and
trusted diagnostics only; the atomic VM transition and terminal player-failure
priorities remain as defined above.
