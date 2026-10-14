# CodeGrid Scene Specification v1

Status: accepted product protocol, implemented by the shared Rust scene core and API/ABI v2 adapters; direct-vector and production integration coverage remains open.
Recorded: 2026-10-05.
Source: the user-selected conversation [Scene definition review](chatgpt-conversation://6ac2518f-10cc-83ec-98ae-a8e59bd6b639), especially its consolidated Scene Spec v1 and accepted follow-up rules.

## 1. Scope and authority

This specification defines three scenes: ExactIO, Robot, and MechanicalArm. It translates the selected conversation into
an implementation-oriented English contract. The existing compiler and VM do
not interpret actors, modules, maps, or stations.
Scenes interpret ordinary byte input and committed byte output in the Rust level
layer. Presentation and animation remain in the game host.

All three scenes execute through the shared Rust scene core and API/ABI v2.
The author-file contract is defined in `codegrid-scene-level-json-v2.md`
(author format v1), and transport behavior in `codegrid-scene-host-contract-v2.md`.
The legacy API/ABI v1 retains its separate ExactIO contract.

For these new scene protocols, this specification supersedes the older
five-scene selection and its unresolved protocol examples. Robot's objective is
visiting required patrol points, not repairing devices. The continuous test-case
VM lifecycle below replaces the earlier proposed fresh-VM-per-decision model
for these protocols. Existing implemented ExactIO behavior, VM semantics,
transaction boundaries, resource outcomes, and hidden-test privacy remain
defined by their current contracts.

## 2. Shared lifecycle and execution boundary

### 2.1 Values and isolation

All VM-facing values are `u8`. Domain limits are additional scene validation:
Only Robot and MechanicalArm define additional scene domains.
Invalid author data is rejected before execution. Invalid player output fails
the current case.

Each case creates a fresh standard VM and fresh scene state from the same
verified initial program. Reset registers, memory, stacks, execution positions,
mutable code, input/output, round counters, actor state, pending transitions,
and per-case scene metrics. No state leaks between cases.

Within a dynamic scene case, keep the same VM across actions and rounds.
Replace all unread input with the latest complete observation after each valid
nonterminal round, including WAIT, turns, and no-ops. Discard partially consumed
old snapshots; already read register values persist. One actor updates per
command; two actors retain the complete A/B frame boundary.
Registers, memory, stacks, execution position, and mutable
code persist according to ordinary VM semantics. Only a new case resets them.
The shared scene session implements input replacement and continuous execution.
Host adapters delegate these transitions to Rust rather than reproducing them.

### 2.2 READ, OUTPUT, and termination

An exhausted READ retains the selected register and applies its existing
exhaustion direction. There is no EOF byte or additional sentinel, except the
explicit MechanicalArm empty-hand observation defined below.

Consume only committed outer VM outputs, in their normative order. Custom
internal outputs retain their existing caller-stack meaning. VM errors reject
their tick before the scene consumes any output from that tick. A scene must not
reinterpret empty-stack no-ops, host faults, work-limit yields, cancellation,
or safety ceilings as new VM errors. Preserve the existing distinct result
categories for those outcomes.

Successful HALT is not automatically a scene pass. If its goal is unfinished,
fail with the logical reason `IncompleteGoal`; a partially collected dual-actor
frame is also unfinished. ExactIO with empty expected output pass
only on successful HALT without any output. Complete nonempty ExactIO/OUTPUT_SEQUENCE goals pass
immediately without requiring HALT or consumption of remaining input. MechanicalArm
FINAL_LAYOUT compares after each complete valid action round and passes immediately when the target matches; an initially matching layout passes at initialization.

Scene outcome names are logical categories, not new published error codes:

```rust
enum SceneOutcome {
    Running,
    Pass,
    Fail(SceneFailure),
}

enum SceneFailure {
    InvalidOutput,
    OutputMismatch,
    IllegalOperation,
    IncompleteGoal,
    RuntimeError,
}
```

Existing ExactIO terminal priority applies to static comparison profiles.
Dynamic scenes use the explicit priority in Section 2.4; the earlier ExactIO
contract left Environment priority open. Passing a
scene does not bypass a violated constraint. Once a case is terminal, consume
no further scene actions and publish no observations.

### 2.3 Rounds and actor frames

Robot and MechanicalArm use these action bytes:

| Value | Shared meaning | Robot | MechanicalArm |
| --- | --- | --- | --- |
| 0 | WAIT | Preserve position/direction | Preserve hand/orientation |
| 1 | Scene-specific | FORWARD | GRAB |
| 2 | Scene-specific | REPAIR | DROP |
| 3 | TURN_LEFT | Turn left | Rotate left by 60 degrees |
| 4 | TURN_RIGHT | Turn right | Rotate right by 60 degrees |

Other bytes fail as `InvalidOutput`.
For a single actor, one output is a complete frame. For two actors, collect
two outputs: first A, then B. Do not execute A until both outputs are collected.
Robot decodes each byte when dispatched, A before B against updated state.
Robot retains A effects if B fails and stops immediately if A is terminal.
MechanicalArm instead validates and stages the entire atomic frame as Section 8
defines; it commits no partial A effects and always validates B.
WAIT is an action. Robot and MechanicalArm advance a completed nonterminal
round even when both actors wait or movement/grab is a no-op.

### 2.4 Tick boundary and terminal priority

The orchestrator must process scene output after each committed outer tick,
replace input with resulting observations before the next tick, and only then resume the
same VM. Do not batch unprocessed VM ticks across a host slice. A partial dual
frame yields no scene action or new observation. Host slice size must not change
the observation sequence, world state, or result.

For every attempted outer tick:

1. A VM fault or runtime error takes precedence; rejected ticks have no scene
   output or actor effects. Preserve its existing identity/category.
2. A work-budget interruption leaves the case Pending unless a trusted
   cumulative ceiling is exhausted. Interrupted ticks commit nothing.
3. After VM commit, update metrics and check generated-code permissions before
   processing any scene output. A forbidden mutation is ProgramRejected;
   do not undo the committed VM tick.
4. Collect and dispatch scene actions in protocol order. Scene failure takes
   precedence over a simultaneous constraint failure. On a scene goal, stop
   dispatch immediately, but defer awarding success until constraints are checked.
5. Check applicable constraints before awarding success. A correct goal cannot
   override ConstraintExceeded. If still Running and the VM halted, fail as
   IncompleteGoal, including an incomplete actor frame.
6. At an exact trusted safety ceiling, allow terminal completion needing no
   further work; otherwise return the distinct resource-limit outcome. Preserve
   cancellation semantics and do not invent a VM error.

Check structural acceptance and static constraints before scene initialization.
Robot starts and patrol points occupy distinct cells, so a valid author map
cannot repair a patrol point during initialization. Per-case dynamic counters
start at zero.

Reserve/validate input replacement capacity before publishing a successful scene
transition and observation. Insufficient capacity produces ResourceLimitExceeded,
not dropped input or altered bytes. Previously committed VM effects remain
committed; resource-terminal scene projections must be defined by the session
contract. All queue and scene-metric counters use checked arithmetic. Waiting
and blocked actions remain subject to VM work/tick ceilings even when no scene
goal progresses.

## 3. ExactIO

Case data: `input: Vec<u8>` and `expected_output: Vec<u8>`.
Queue the entire input at initialization, preserving author order.

For nonempty expected output, compare each committed output immediately with
its corresponding expected byte. A wrong byte fails immediately. The final
matching expected byte completes the case immediately. Extra output encountered
while the comparison is active fails; outputs after terminal completion are not
processed. Unread input does not prevent passing.

For empty expected output, initialization is Running, any output fails, and
successful HALT with no output passes. These rules follow the existing
[ExactIO implementation contract](codegrid-level-exactio-contract-v1.md),
including its transactional and constraint priorities.

## 4–5. Removed presentation and runtime scenes

Paper tape uses ExactIO with precompiled case bytes. Its five-bit authoring
restrictions and character rendering belong to the application. Elevator is
removed and its old wire identifier is unsupported. See the 2026-10-07 decision.

## 6. Robot

### 6.1 Map and actors

The map is exactly 16 by 16. Coordinates are 0–15; encode a position as
`y * 16 + x`, yielding 0–255. Actor directions are 0 UP, 1 RIGHT, 2 DOWN,
3 LEFT. x increases right and y increases down: UP=(0,-1),
RIGHT=(1,0), DOWN=(0,1), LEFT=(-1,0). TURN_LEFT uses `(direction + 3) % 4`;
TURN_RIGHT uses `(direction + 1) % 4`. Colors are 0 NONE, 1 BLUE, 2 RED.
Terrain is VOID or ground, and every ground cell has the same height. The initial
16-by-16 author canvas is entirely VOID; only explicitly drawn ground is usable.

Colors are ground attributes, defaulting to NONE when omitted. Starts, patrol
points, triggers, and doors are mutually exclusive objects; at most one object
occupies each position. Every object requires ground. Configure one
or two fixed robot starts and directions in the static map; dual starts must
differ. Starts cannot occupy closed doors or patrol/trigger cells. Configure at
least one required patrol point. VOID accepts neither color records nor objects.

### 6.2 Actions and collision

FORWARD attempts the adjacent cell in the facing direction. Move only if it is
passable and unoccupied. Bounds, VOID, closed doors, or occupancy make movement
a no-op, not failure. REPAIR does not move the robot. It marks the current cell's
required patrol point repaired; elsewhere it has no effect. TURN changes
direction only; WAIT changes neither. Every completed nonterminal frame advances
a round, including blocked moves and REPAIR away from a patrol point.

Robots cannot share a position. A acts first. If both target the same empty
cell, A takes it and B stays. A cannot enter B's occupied position, so a direct
swap leaves both stationary. If A moves to a third cell, B may then enter A's
vacated position.

### 6.3 Patrols and doors

Entering a required patrol point does not complete it. A robot must execute
REPAIR while occupying that point. Both actors share the repaired-point set;
repeated REPAIR at one point does not affect other points. Starts and patrol
objects are mutually exclusive, so no point can be repaired at initialization.
Pass immediately after REPAIR completes the final required point. No return
home, HALT, stopped actor, or mechanism reset is required.

Triggers and doors pair by equal integer IDs with a strict one-to-one association:
each ID has exactly one trigger and one door. Patrol IDs use a separate namespace.
All doors
start closed. First entry onto a trigger marks its associated door pending.
The door retains its old state for the whole current round; open pending doors
at round end, so they become passable next round. Doors stay open permanently.
Starts and triggers are mutually exclusive, so author maps cannot trigger a
door at initialization. If a goal terminates the case before
round end, perform no remaining door updates.

### 6.4 Observation protocol

Queue an initial snapshot before the first action. After each complete round
still Running, replace unread input with another snapshot:

| Actors | Snapshot |
| --- | --- |
| One | `[color, position]` |
| Two | `[A_color, A_position, B_color, B_position]` |

Color is the current cell's color. Direction is not included. Map and initial
direction are scene definition/presentation data. Discard unread old snapshots.

## 7. Quality presentation

Quality inspection is an application presentation of precompiled ExactIO cases.
Rust imposes no Color/PackedRobot domain or Item/Batch case-length policy.

## 8. MechanicalArm

One arm has six stations 0..5. Two arms have ten stations, mapping A directions
0..5 to [0,1,2,3,4,5] and B to [6,5,4,7,8,9]. Stations 4 and 5 are shared.
Arms start empty facing 0. Turns add 1 (right) or 5 (left), modulo 6.
Each station is STACK (capacity four), FLIP (capacity one), INPUT, or OUTPUT.
At most one each FLIP/INPUT/OUTPUT is allowed. Conveyors store no contents.

Module bits 1..0 are color 0/1/2 (3 invalid), bit 2 face, bit 3 weight
(1 heavy), bits 7..4 number. All 192 valid bytes, including zero, are modules;
255 exclusively means empty hand. FLIP DROP immediately XORs bit 2. Authored
initial/target FLIP contents retain their face. Heavy cannot lie above light.

Retain actions 0 WAIT, 1 GRAB, 2 DROP, 3 TURN_LEFT, 4 TURN_RIGHT. GRAB takes
the top stack module, FLIP module, or INPUT FIFO head. Empty targets and OUTPUT
GRAB do nothing. Occupied-hand GRAB fails only if a retrievable module exists.
Empty-hand DROP always does nothing, including INPUT. Held DROP to INPUT,
full STACK, occupied FLIP, or heavy above light fails. OUTPUT DROP removes and
compares the entire module byte.

Dual frames collect A/B before effects, preflight action validity and any pair
of shared-station GRAB/DROP commands (including no-ops), then stage A before B.
Both must succeed before committing any scene state or successful feedback.
A completing output cannot bypass B validation. VM ticks remain committed.
Every nonterminal completed round replaces unread VM input with current hands,
A before B. Initial input is [255] or [255,255]. Pending A at halt fails.
Robot retains its existing ordered dispatch and early-terminal behavior.

OUTPUT_SEQUENCE requires OUTPUT and a nonempty expected sequence. Prefix
mismatch/overflow fails; completion passes after the entire valid round.
Remaining modules are allowed. FINAL_LAYOUT permits only STACK/FLIP and
compares exact station contents after each complete valid action round. A matching
layout passes immediately; an initially matching layout passes at initialization.
Resource limits, cancellation and faults do not independently trigger success. Serialize stations in numeric order,
each count followed by bottom-to-top bytes. Hands/directions are excluded.
Initial/target inventories must match with face bit cleared; a held module
therefore leaves a target module missing. Each case starts fresh VM/world.

## 9. Validation and implementation records

`validate_level()` must validate before constructing an executable scene.
Use bounded domain types after validating untrusted integers; do not truncate
an out-of-range author value into a byte. Required logical records include:

| Scene | Required data |
| --- | --- |
| ExactIO | Input bytes, expected bytes |
| Robot | 256 map cells, 1–2 actor starts/directions, patrol set, trigger/door associations |
| MechanicalArm | Arm count, mode, typed stations, initial contents, module queues or targets |

Validation covers all bounds and structural restrictions in Sections 3–8,
including duplicate/missing IDs, dangling trigger/door references, unique dual
starts, legal station types, and per-case reset requirements. Only host-trusted
ceilings bound otherwise unspecified author collection sizes; do not invent
gameplay limits for quality inspection batches or MechanicalArm conveyors.

The Rust scene layer owns validation, observation encoding, action decoding,
ordered transitions, goals, and scene metrics. Compiler/model/VM stay independent
of game rules. Hosts delegate to the shared evaluator; they must not implement
a second scene interpreter. Keep hidden case data
out of public results, snapshots, traces, and presentation beyond permitted
observations under the existing privacy contract.

All current scenes use existing VM/level metrics. Gas, Operation Count, work units, Global
Ticks, and scene rounds are distinct; a blocked action can advance a round
without changing position. Adding a metric requires a later recorded contract.

## 10. Supporting contracts and implementation gaps

The six review recommendations were accepted by the user on 2026-10-05:
per-tick output/input interleaving, actor-order validation, retaining completed
A effects on B failure, explicit terminal priority, fixed Robot geometry, and
a continuous VM with all ordinary state retained within a case. Sections 2 and
6 now define these rules; they are not optional implementation choices.

The [scene session design](../docs/scene-session-design.md) defines ownership,
queue replacement, transactional boundaries, slicing, resource accounting, and
integration requirements. The [scene conformance plan](../docs/scene-conformance-plan.md)
defines concrete acceptance vectors and delivery gates. The selected
[Scene Level JSON format v1 contract](codegrid-scene-level-json-v2.md) fixes
author records, strict validation, metrics, and replay seeds. Its filename
retains the earlier v2 name. The Rust loader, examples, and fixtures use that
contract. The continuous session, trusted limits, public events, and
privacy-safe projections are implemented in the current Level API path; the
remaining direct protocol and actual-host evidence is listed in the
[conformance plan](../docs/scene-conformance-plan.md). Paper-tape character rendering belongs to the application. These evidence gaps do not
reopen the accepted gameplay protocol.

## 11. Required acceptance cases

| Area | Direct cases |
| --- | --- |
| Shared | Fresh-case isolation including self-modified code; same-case memory persistence; replace unread old snapshots; empty READ; HALT with incomplete frame/goal; failed VM tick consumes no scene output; terminal goal with pending B |
| ExactIO | First/middle mismatch; final match; unread input; empty expected and HALT; empty expected and output |
| Robot | All directions; boundary/VOID/door blocking; movement across all ground cells; REPAIR only at the current required patrol point; swap/shared-target/vacated-cell collision; shared repaired-point progress; trigger activation and one-round door delay; snapshots |
| MechanicalArm | All 192 module bytes; capacities and weighted stacks; FIFO input; immediate flips; atomic complete frames; shared no-op conflicts; refreshed hands; exact output sequence; immediate final-layout success after valid rounds |
| Hosts | Same definitions, inputs, actions, statuses, errors, metrics, and privacy projections in actual native/browser/server execution before claiming parity |

This document establishes requirements, not implementation or test-pass evidence.
