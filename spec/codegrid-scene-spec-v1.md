# CodeGrid Scene Specification v1

Status: accepted product protocol, implemented by the shared Rust scene core and API/ABI v2 adapters; direct-vector and production integration coverage remains open.
Recorded: 2026-10-05.
Source: the user-selected conversation [梳理确定场景](chatgpt-conversation://6ac2518f-10cc-83ec-98ae-a8e59bd6b639), especially its consolidated Scene Spec v1 and accepted follow-up rules.

## 1. Scope and authority

This specification defines six scenes: ExactIO, Baudot, Elevator, Robot,
QualityControl, and MechanicalArm. It translates the selected conversation into
an implementation-oriented English contract. The existing compiler and VM do
not interpret elevators, actors, passengers, robot parts, maps, or worktables.
Scenes interpret ordinary byte input and committed byte output in the Rust level
layer. Presentation and animation remain in the game host.

All six scenes execute through the shared Rust scene core and API/ABI v2.
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
Baudot codes are 0–31, floors are 0–9, and QualityControl decisions are 0–1.
Invalid author data is rejected before execution. Invalid player output fails
the current case.

Each case creates a fresh standard VM and fresh scene state from the same
verified initial program. Reset registers, memory, stacks, execution positions,
mutable code, input/output, round counters, actor state, pending transitions,
and per-case scene metrics. No state leaks between cases.

Within a dynamic scene case, keep the same VM across actions and rounds.
Append new observation bytes to the tail of its remaining input queue; never
replace unread bytes. Registers, memory, stacks, execution position, and mutable
code persist according to ordinary VM semantics. Only a new case resets them.
The shared scene session implements input append and continuous execution.
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
frame is also unfinished. ExactIO and Baudot with empty expected output pass
only on successful HALT without any output. Complete nonempty goals pass
immediately without requiring HALT or consumption of remaining input.

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
no further scene actions and append no observations.

### 2.3 Rounds and actor frames

Robot and MechanicalArm use these action bytes:

| Value | Shared meaning | Robot | MechanicalArm |
| --- | --- | --- | --- |
| 0 | WAIT | Preserve position/direction | Preserve hand/orientation |
| 1 | Scene-specific | FORWARD | GRAB |
| 2 | Scene-specific | JUMP | DROP |
| 3 | TURN_LEFT | Turn left | Rotate left by 60 degrees |
| 4 | TURN_RIGHT | Turn right | Rotate right by 60 degrees |

Other bytes fail as `InvalidOutput`. Elevator uses floor targets instead;
its byte 0 means floor 0, not WAIT.

For a single actor, one output is a complete frame. For two actors, collect
two outputs: first A, then B. Do not execute A until both outputs are collected.
Decode and validate each collected byte only when its actor is dispatched,
A then B, against the updated scene state. Invalid A fails without actor
effects. Invalid B fails only if A did not already terminate the case.
Retain completed A effects if B fails; skip round-end promotions and
observations. Scene rounds do not roll back previously completed actors.
If A produces a
terminal outcome, stop immediately without executing B or round-end work.
WAIT is an action. Robot and MechanicalArm advance a completed nonterminal
round even when both actors wait or movement/grab is a no-op. Elevator's special
round advancement rule is defined in Section 5.

### 2.4 Tick boundary and terminal priority

The orchestrator must process scene output after each committed outer tick,
append resulting observations before the next tick, and only then resume the
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
Robot starts and patrol points occupy distinct cells, so valid author maps cannot
complete their patrol goal during initialization. Per-case dynamic counters start
at zero.

Reserve/validate input append capacity before publishing a successful scene
transition and observation. Insufficient capacity produces ResourceLimitExceeded,
not dropped input or altered bytes. Previously committed VM effects remain
committed; resource-terminal scene projections must be defined by the session
contract. All queue and scene-metric counters use checked arithmetic. Waiting
and blocked actions remain subject to VM work/tick ceilings even if Elevator
rounds do not advance.

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

## 4. Baudot

Case data: byte vectors `input` and `expected_output`, with every value 0–31.
A message unit is one five-bit code; a case contains an entire sequence.
Append the complete input once at initialization, not one code per round.
Author values above 31 are invalid. Player values above 31 fail immediately.

Start interpretation/display mode at LTRS. LTRS/FIGS mode persists through
the case and resets for the next case. Correctness compares raw five-bit codes,
not rendered text. Pass/fail and empty expected-output behavior exactly follow
ExactIO. A particular character table, shift-code numbers, and presentation
conversion are not selected by this protocol; they must not alter byte
correctness or be invented as author-data acceptance rules.

## 5. Elevator

### 5.1 Definition and validation

Floors are 0–9. A case has one or two elevators, each with an independently
configured initial floor. Two elevators may start on or occupy the same floor.
For a single elevator, no B initial floor is required.

Passengers are ordered records `{from: u8, to: u8}`. Both floors must be 0–9
and different. There must be at least one initial passenger and 1–10 passengers
total across `initial_passengers` and `sequential_passengers`. Preserve author
order; do not sort or deduplicate passengers.

Initially all initial passengers are waiting at their origins. Merely starting
an elevator at a floor does not service passengers. Initial input is each
initial passenger's `[from, to]` pair concatenated in author order, with no
count prefix.

### 5.2 Targets, servicing, and rounds

One elevator receives one target output per frame; two receive A's target then
B's. Targets 10–255 fail. A target equal to the elevator's current floor is a
no-op: no boarding, alighting, travel, or stop is counted.

For a different target, move immediately, without simulated travel time:

1. Alight every onboard passenger whose destination is this floor.
2. Board every waiting passenger at this floor.

There is no capacity limit. Execute A before B. If both arrive at the same
floor, A boards first; B cannot board passengers already taken by A.

A completed frame advances the round only if at least one elevator actually
moved. A stationary elevator does not service passengers. After each advanced
nonterminal round, introduce at most one sequential passenger in author order
and append that passenger's `[from, to]` to the unread queue tail. Introduction
does not retroactively service an elevator already at that origin. A passenger
is introduced and sent only once. A frame where every target equals the
corresponding current floor introduces no passenger.

Pass immediately when all configured passengers have appeared and all have
reached their destinations. Pending sequential passengers prevent passing.

### 5.3 Scene metrics

`travel_distance` adds `abs(target - previous_floor)` for every actual move.
`stop_count` adds one per elevator per actual move/arrival. Sum across both
elevators. Current-floor outputs contribute zero to both metrics.

## 6. Robot

### 6.1 Map and actors

The map is exactly 16 by 16. Coordinates are 0–15; encode a position as
`y * 16 + x`, yielding 0–255. Actor directions are 0 UP, 1 RIGHT, 2 DOWN,
3 LEFT. x increases right and y increases down: UP=(0,-1),
RIGHT=(1,0), DOWN=(0,1), LEFT=(-1,0). TURN_LEFT uses `(direction + 3) % 4`;
TURN_RIGHT uses `(direction + 1) % 4`. Colors are 0 NONE, 1 BLUE, 2 RED;
height is 0 or 1. Terrain is VOID, low ground, or high ground. The initial
16-by-16 author canvas is entirely VOID; only explicitly drawn ground is usable.

Colors are ground attributes, defaulting to NONE when omitted. Starts, patrol
points, triggers, and doors are mutually exclusive objects; at most one object
occupies each position. Every object requires low or high ground. Configure one
or two fixed robot starts and directions in the static map; dual starts must
differ. Starts cannot occupy closed doors or patrol/trigger cells. Configure at
least one required patrol point. VOID accepts neither color records nor objects.

### 6.2 Actions and collision

FORWARD attempts the adjacent cell in the facing direction. Move only if it is
passable, unoccupied, and at the same height. JUMP also attempts exactly one
adjacent cell; move only if it is passable, unoccupied, and at a different height.
Bounds, VOID, closed doors, occupancy, or an incompatible height make movement
a no-op, not failure. TURN changes direction only; WAIT changes neither.
Every completed nonterminal frame advances a round, including blocked moves.

Robots cannot share a position. A acts first. If both target the same empty
cell, A takes it and B stays. A cannot enter B's occupied position, so a direct
swap leaves both stationary. If A moves to a third cell, B may then enter A's
vacated position.

### 6.3 Patrols and doors

Entering a required patrol point marks it visited automatically. Both actors
share the visited set. Starts and patrol objects are mutually exclusive, so
the revised author format cannot complete patrol at initialization.
Pass immediately when every required point is visited. No return home, HALT, stopped actor, or mechanism reset is
required.

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
still Running, append another snapshot:

| Actors | Snapshot |
| --- | --- |
| One | `[position, color]` |
| Two | `[A_position, A_color, B_position, B_color]` |

Color is the current cell's color. Direction is not included. Map and initial
direction are scene definition/presentation data. Do not replace unread snapshots.

## 7. QualityControl

Decisions are 0 Reject and 1 Accept; outputs 2–255 fail. Each level selects
one input representation and one case mode; do not mix representations.

Color representation permits one byte per item: 0 NONE, 1 BLUE, 2 RED.
PackedRobot representation uses one byte per item:

| Bit(s) | Meaning |
| --- | --- |
| 7 | Left leg fault |
| 6 | Right leg fault |
| 5 | Left arm fault |
| 4 | Right arm fault |
| 3 | Visual sensor fault |
| 2 | Core fault |
| 1–0 | Color: 0 NONE, 1 BLUE, 2 RED; 3 invalid |

Each fault flag value 0 means normal and value 1 means faulty. Reject author
data with color 3.
This layout is distinct from MechanicalArm's state layout.

Item mode requires exactly one input item and exactly one expected decision.
Batch mode requires at least one input item and exactly one expected decision.
Append the entire batch in author order at initialization. There is no count
prefix and no per-item replenishment; READ exhaustion can detect the batch end.
The first decision output immediately passes or fails against the expected
0/1. No later outputs are processed after terminal completion.

There is no general rule engine. Descriptions such as "accept at least three
blue items" are author guidance; the author supplies inputs and the expected
decision, and the evaluator compares the actual decision.

## 8. MechanicalArm

### 8.1 Actors, directions, and layout

Configure one or two arms. Every arm starts empty and facing LEFT; the author
cannot override this. Orientations are:

| Index | Direction | Single arm | Upper arm A | Lower arm B |
| --- | --- | --- | --- | --- |
| 0 | LEFT | Input | Input | Unavailable |
| 1 | UPPER_LEFT | Worktable | Upper worktable 1 | Shared buffer left |
| 2 | UPPER_RIGHT | Worktable | Upper worktable 2 | Shared buffer right |
| 3 | RIGHT | Output | Output | Output |
| 4 | LOWER_RIGHT | Worktable | Shared buffer right | Lower worktable 2 |
| 5 | LOWER_LEFT | Worktable | Shared buffer left | Lower worktable 1 |

TURN_RIGHT sets `(orientation + 1) % 6`; TURN_LEFT sets
`(orientation + 5) % 6`. Each turn is exactly 60 degrees. There are four fixed
optional worktable slots total, populated with 0–4 tables. Unpopulated slots
are noninteractive. Table types may repeat: Inspection, Repair, Processing,
Packing, Removal. Dual arms cannot access each other's private tables.

### 8.2 Robot data and encoding

An input robot's author data contains only color (0–2) and hidden true
inspection (0 NORMAL, 1 DEFECT, 2 IRREPARABLE). Authors cannot provide
UNINSPECTED, processed, or packed initial states. Preserve input FIFO order.
On its first grab from input, the robot's visible inspection is UNINSPECTED,
processed and packed are zero, and its color is the author color. The hidden
inspection result stays within the scene until Inspection completes.

Visible state is one byte:

| Bit(s) | Meaning |
| --- | --- |
| 7–6 | Reserved, must be zero |
| 5 | Packed |
| 4 | Processed |
| 3–2 | Inspection: 0 NORMAL, 1 DEFECT, 2 IRREPARABLE, 3 UNINSPECTED |
| 1–0 | Color: 0 NONE, 1 BLUE, 2 RED; 3 invalid |

State is `color | (inspection << 2) | (processed << 4) | (packed << 5)`.
UNINSPECTED, DEFECT, and IRREPARABLE require processed=0 and packed=0.
Processed or packed requires NORMAL. A packed normal robot may be processed
or unprocessed: Inspection followed directly by Packing is valid.
Byte 255 is EMPTY for dual-arm observations and never a valid robot state.

### 8.3 Conveyors and worktables

Only single/upper arms grab the input FIFO head. An empty input is a no-op.
Lower-arm input access is unavailable and GRAB there is a no-op. DROP with a
held robot onto input fails. Output cannot be grabbed; GRAB there is a no-op.
DROP onto output immediately removes the held robot into the output FIFO,
empties the hand, and performs expected-output comparison. Output has no
occupancy limit. Dual outputs are ordered A before B.

Ordinary tables have Empty, Processing(robot), and Ready(robot) states.
Legal DROP to Empty makes it Processing. Complete its transformation and make
it Ready at the end of the round; it is grabbable starting next round.

| Table | Required input | Completion effect |
| --- | --- | --- |
| Inspection | UNINSPECTED, processed=0, packed=0 | Reveal hidden true inspection |
| Repair | DEFECT, processed=0, packed=0 | Set visible inspection to NORMAL |
| Processing | NORMAL, processed=0, packed=0 | Set processed=1 |
| Packing | NORMAL, packed=0 | Set packed=1; processed is not required |
| Removal | Any valid state | Immediately delete the robot and empty the hand |

Removal never becomes Processing/Ready, never occupies the table, and cannot
be grabbed later. Packed robots may go only to buffers, output, or Removal;
they cannot enter any transforming table. Reinspection, repairing NORMAL or
IRREPARABLE, reprocessing, and repacking are illegal and fail immediately.

Shared buffers have Empty, Pending(robot), and Ready(robot) states. DROP to
Empty makes Pending; make it Ready at round end. B cannot grab an item A
dropped into a buffer earlier in the same round. A successful grab empties
the source and transfers the same robot, including its hidden identity/state.

### 8.4 GRAB and DROP rules

| GRAB situation | Result |
| --- | --- |
| Empty hand, accessible ready robot | Transfer robot to hand |
| Empty hand, empty/unavailable target | No-op |
| Empty hand, Processing table or Pending buffer | No-op |
| Occupied hand, empty/unavailable target | No-op |
| Occupied hand, target contains a robot | Fail; no second robot may be grabbed |

Output and Removal do not retain grabbable robots. Input contains a robot
only while its FIFO is nonempty. Processing/Pending occupancy still counts as
containing a robot for the occupied-hand error rule.

| DROP situation | Result |
| --- | --- |
| Empty hand, any target | No-op |
| Held robot, legal empty ordinary table | Processing, subject to table precondition |
| Held robot, empty buffer | Pending |
| Held robot, output | Immediate output comparison |
| Held robot, Removal | Immediate deletion |
| Held robot, occupied table/buffer | Fail |
| Held robot, input/noninteractive/unavailable slot | Fail |

### 8.5 Rounds and observations

One arm's action completes one round, including WAIT, TURN, GRAB, DROP, and
no-ops. Two arms require A/B actions, execute A then B, and only then complete
Processing/Pending transitions at round end. If terminal during A or B, skip
remaining actions and round-end work.

Single-arm input is event-based. There is no initial snapshot or EMPTY byte.
Append the acquired visible state only after successful GRAB from input,
Inspection, Repair, Processing, or Packing. WAIT, TURN, DROP, and unsuccessful
GRAB append nothing. Removal has no retrievable state.

Dual-arm input uses snapshots: initial `[255, 255]`, then
`[A_state, B_state]` after every completed round still Running, even if neither
hand changed. A held hand contributes its visible byte, an empty one 255.
Append at queue tail.

### 8.6 Expected robots and goal

Require at least one input robot and one expected state, with
`expected_output.len() <= input.len()`. Every expected byte must satisfy the
visible-state rules: reserved bits zero, color not 3, flags compatible with
inspection, and not 255. This is state validation, not a requirement to prove
that the particular conveyor sequence/worktable layout can solve the level.

Compare the full visible state of each robot entering output, in FIFO order.
A mismatch or excess output while active fails. A final match passes
immediately even if robots remain in input, hands, tables, or buffers.
Packing need not follow Processing unless the expected state requires both.

## 9. Validation and implementation records

`validate_level()` must validate before constructing an executable scene.
Use bounded domain types after validating untrusted integers; do not truncate
an out-of-range author value into a byte. Required logical records include:

| Scene | Required data |
| --- | --- |
| ExactIO | Input bytes, expected bytes |
| Baudot | Input/expected five-bit codes |
| Elevator | Actor count, initial floors, ordered initial/sequential passengers |
| Robot | 256 map cells, 1–2 actor starts/directions, patrol set, trigger/door associations |
| QualityControl | Level-wide representation and Item/Batch mode, input items, one expected decision |
| MechanicalArm | Arm count, four optional table slots, ordered input robot definitions, expected state bytes |

Validation covers all bounds and structural restrictions in Sections 3–8,
including duplicate/missing IDs, dangling trigger/door references, unique dual
starts, legal table types, and per-case reset requirements. Only host-trusted
ceilings bound otherwise unspecified author collection sizes; do not invent
gameplay limits for QC batches or MechanicalArm conveyors.

The Rust scene layer owns validation, observation encoding, action decoding,
ordered transitions, goals, and scene metrics. Compiler/model/VM stay independent
of game rules. Hosts delegate to the shared evaluator; they must not implement
a second scene interpreter. Keep hidden case data and hidden true inspection
out of public results, snapshots, traces, and presentation beyond permitted
observations under the existing privacy contract.

Only Elevator adds scene-specific metrics in v1: travel_distance and stop_count.
Other scenes use existing VM/level metrics. Operation cost, work units, Global
Ticks, and scene rounds are distinct; a blocked action can advance a round
without changing position. Adding a metric requires a later recorded contract.

## 10. Supporting contracts and implementation gaps

The six review recommendations were accepted by the user on 2026-10-05:
per-tick output/input interleaving, actor-order validation, retaining completed
A effects on B failure, explicit terminal priority, fixed Robot geometry, and
a continuous VM with all ordinary state retained within a case. Sections 2 and
6 now define these rules; they are not optional implementation choices.

The [scene session design](../docs/scene-session-design.md) defines ownership,
queue append, transactional boundaries, slicing, resource accounting, and
integration requirements. The [scene conformance plan](../docs/scene-conformance-plan.md)
defines concrete acceptance vectors and delivery gates. The selected
[Scene Level JSON format v1 contract](codegrid-scene-level-json-v2.md) fixes
author records, strict validation, metrics, and replay seeds. Its filename
retains the earlier v2 name. The Rust loader, examples, and fixtures use that
contract. The continuous session, trusted limits, public events, and
privacy-safe projections are implemented in the current Level API path; the
remaining direct protocol and actual-host evidence is listed in the
[conformance plan](../docs/scene-conformance-plan.md). A Baudot display table is
only needed if a host adds character rendering; raw-code correctness is defined
here and does not depend on presentation choices. These evidence gaps do not
reopen the accepted gameplay protocol.

## 11. Required acceptance cases

| Area | Direct cases |
| --- | --- |
| Shared | Fresh-case isolation including self-modified code; same-case memory persistence; append behind unread data; empty READ; HALT with incomplete frame/goal; failed VM tick consumes no scene output; terminal goal with pending B |
| ExactIO/Baudot | First/middle mismatch; final match; unread input; empty expected and HALT; empty expected and output; Baudot 31 versus 32; per-case LTRS reset |
| Elevator | Floors 0/9 versus 10; from=to rejected; passenger bounds; author order; current-floor no-op; alight-before-board; same-floor A/B service; late passenger append; pending arrivals prevent pass; distance/stops |
| Robot | All directions; boundary/VOID/door blocking; forward same-height and jump different-height; swap/shared-target/vacated-cell collision; shared patrols; trigger activation and one-round door delay; snapshots |
| QualityControl | Both representations; color 3 rejection; all fault bits; Item lengths; empty Batch rejection; one expected decision; output 2 failure; full batch queued before execution |
| MechanicalArm | All six orientations and both layouts; duplicate/absent tables; FIFO input; hidden inspection; byte packing; illegal flags/255; every table precondition; direct normal Packing; repeat transformation failure; Removal; occupied GRAB/DROP; pending buffer/table delay; event/snapshot queues; ordered outputs and immediate pass |
| Hosts | Same definitions, inputs, actions, statuses, errors, metrics, and privacy projections in actual native/browser/server execution before claiming parity |

This document establishes requirements, not implementation or test-pass evidence.
