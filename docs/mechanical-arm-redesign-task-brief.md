# MechanicalArm module logistics redesign: task brief

Status: implementation authorized by the user on 2026-10-13; implemented and verified locally.
Date: 2026-10-13.
Input: the user's MechanicalArm scene redesign specification v2.0.

## 1. Objective and scope

Replace the current inspection, repair, processing, packing, and removal scene
with deterministic module transport, stacking, flipping, and sorting. Support
one arm with six stations and two arms with ten unique stations, including two
shared stations. Implement STACK, FLIP, INPUT, and OUTPUT stations,
module bytes with three valid color codes, multiple independent cases, and OUTPUT_SEQUENCE / FINAL_LAYOUT.

This task includes the shared Rust evaluator, author JSON contract, feedback,
metrics, constraints, examples, actual-host verification, downstream editor,
publication validation, generated artifacts, and scene presentation.
CodeGrid source syntax, general VM instructions, registers, and stacks are out
of scope. Scene outcomes remain subject to constraints and trusted limits.

The decisions below are implemented in the current scene contract. Historical
proposal conflicts are retained here to explain the adopted replacement.

## 2. Review findings and required decisions

### D1. Action byte mapping conflicts with the current contract

The proposal maps 1/2 to TURN_LEFT/TURN_RIGHT and 3/4 to GRAB/DROP.
The current contract maps 1/2 to GRAB/DROP and 3/4 to TURN_LEFT/TURN_RIGHT.
Therefore, the claim that existing actions are retained is inaccurate at the
byte level. This also affects the shared Robot action framing vocabulary.

Confirmed by the user: retain 0 WAIT, 1 GRAB, 2 DROP, 3 TURN_LEFT,
and 4 TURN_RIGHT. Robot actions remain unchanged.
Specify TURN_RIGHT as `(direction + 1) % 6` and TURN_LEFT as
`(direction + 5) % 6`; the geometry alone does not define these formulas.

### D2. Full byte encoding conflicts with current empty-hand observations

Current arms publish `[held_byte]` or `[A_byte, B_byte]` after every completed
nonterminal round, replacing unread VM input. Empty hands use 255, including
the initial observation. The proposed module format makes 255 a valid module.
Using Option in Rust fixes storage but does not fix this byte protocol.

Confirmed by the user: retain 255 for empty hands and refresh input after
commands using the existing single-command / complete A-B-frame boundary and
unread-input replacement policy. VM READ semantics remain unchanged.

Confirmed by the user: only three color codes are valid. Use color codes
0, 1, and 2; color code 3 is invalid. This supersedes the source proposal's
four-color / all-256-values requirement. Valid module bytes satisfy
`(value & 0x03) != 3`: 192 encodings are valid. Both 251 and 255 have color
code 3 and therefore cannot be modules. Byte 255 exclusively denotes an empty
hand. Flip is XOR 0x04 and preserves the two color bits, so it maps every valid
module to another valid module. Reject invalid color bytes in initial contents,
input queues, targets, expected module outputs, and author imports.

### D3. Confirmed atomic MechanicalArm rounds

The user accepted the following complete-frame policy:

1. Collect A and B commands before executing any scene effect.
2. Validate action bytes and preflight shared-station conflicts. Two GRAB/DROP
   commands targeting the same shared station conflict, including no-op cases.
3. Resolve both actions against a staged scene state, in A-then-B order.
4. If either action is illegal, fail without committing the scene round.
5. If both actions are legal, commit the complete scene round, check the goal,
   and refresh unread input if the case remains nonterminal.

A completing the expected sequence cannot bypass B validation. Output-prefix
mismatch or overflow during staged execution fails the round. Scene hands,
directions, stations, queues, and successful action feedback commit together;
failed candidates do not publish partial successful actions. Attempt/failure
metrics must be distinguished from committed-round metrics in the contract.
Already committed VM ticks and VM work are not rolled back. A pending lone A
command at normal VM termination fails as an incomplete frame. Robot's existing
actor dispatch policy remains unchanged.

### D4. Final-layout encoding and targets must be normative

The proposed length-prefixed byte encoding is feasible: capacities fit in one
byte and length prefixes distinguish module byte zero from empty content.
However, a recommended format and unspecified station IDs are insufficient
for deterministic author/runtime comparison.

Adopted: fixed numeric IDs, with single-arm IDs 0..5 matching directions.
For dual arms, assign A.0..A.5 IDs 0..5, B.0=6, B.1=5, B.2=4, B.3=7,
B.4=8, B.5=9. Serialize station IDs in numeric order, excluding hands.
Each field is a count followed by bottom-to-top bytes.

Confirmed by the user: authors configure exact initial and final station
contents, fixed module bytes and locations, and bottom-to-top stack order.
Heavy modules must remain below light modules. FINAL_LAYOUT preserves the
module multiset, apart from face changes. Hands are not author-defined goals
and are excluded from target serialization. Conservation plus exact station
comparison makes a separate empty-hand goal unnecessary: a held module leaves
a required station module missing. Runtime hands remain necessary for actions,
observations, debugging, and cycle detection. Target orientations are ignored.
Targets may include a module on FLIP, treated as the stored orientation rather
than flipping it again during loading. Generate expected_output from target
contents in Rust; do not accept two independently authoritative target fields.
Alternatively, if both are stored, reject inconsistent values.
FINAL_LAYOUT compares after initialization and each complete valid action round.
A matching layout passes immediately without requiring VM termination. Both actors
must finish a valid atomic round; a transient match during A cannot bypass B.
Cancellation, resource limits and faults do not independently trigger target success.

### D5. No-op precedence needs an explicit rule

INPUT DROP is described as always failing, while empty-hand DROP is described
as a no-op. Confirmed by the user: empty-hand DROP is a no-op on every station,
including INPUT, matching current behavior. A held module dropped on INPUT
fails. OUTPUT GRAB is always a no-op. Occupied-hand GRAB fails only when its
target actually contains a retrievable module. Adopt or amend these choices
explicitly and cover each station type in tests.

### D6. Compatibility and version requirements conflict with repository policy

The proposal requires legacy readers and old-scene preservation. Both
repositories currently record no user-published release and prohibit adding
compatibility readers or mandatory version bumps solely for compatibility.

Confirmed by the user: directly replace the current MechanicalArm contract and
consumers; retain unrelated user data. Do not silently convert stored author
data or player programs. Report incompatible drafts to their authors without
adding a second executable scene. The proposal's v2.0 document title does not
require `scene_version: 2`. Distinguish author format_version, Level API version,
scene identity, and build provenance. Any policy exception needs an explicit
user decision before implementation.

### D7. Validation, resources, and observability need completion

Derive capacity from type rather than accepting an independently editable
capacity; otherwise require exact STACK=4 and FLIP=1. Define forbidden contents
for INPUT/OUTPUT, unknown or duplicate station IDs, absent versus empty queue
fields, target completeness, and mode-specific fields. Validate imported JSON
through Rust, not only editor interactions.

For FINAL_LAYOUT, require identical initial/target multisets
after masking flipped bit; flipping may change bit 2 but cannot create modules
or change other attributes. For OUTPUT_SEQUENCE, expected output may consume a
subset of available modules because the proposed immediate success does not
require an empty scene. State that explicitly. Conservation checks do not prove
reachability; the reference program remains the solvability evidence.

Define input/output queue ceilings and account for initialization, candidate
clones, round staging, feedback, serialization, and comparison work. Update
cycle-detection state to include mode, station contents, hands, input cursor,
pending frame, and goal progress; avoid interpreting every move as monotonic
goal progress in FINAL_LAYOUT. Review no-progress heuristics for valid long
rearrangements. Specify frames, rounds, produced modules, terminal snapshots,
and matched-output metrics per mode. Assign structured error identities at
detection and preserve hidden-case projection rules.

## 3. Implementation ownership and work packages

1. Contract decisions: record D1-D7 in docs/decisions.md, update the scene,
   author JSON, and host contracts coherently. Do not change source/VM specs
   merely to accommodate scene logic. Confirm the two mode schemas first.
2. Rust schema and state: implement byte helpers, fixed station mapping,
   mode-specific cases, fresh case initialization, validation, and serialization
   in codegrid-level-core. Read module-local rules before edits.
3. Rust execution: implement framed actions, stack rules, immediate XOR flip,
   input consumption, output comparison, termination comparison, and agreed
   conflict/commit rules. Update session input policy, limits, cycle state,
   feedback, metrics, and public projections through existing boundaries.
4. Authoring and presentation: update downstream draft model, import/export,
   station selection, module property controls, stack ordering, independent
   cases, targets, and playtest displays. Rust remains the authority for weight,
   flip, and success validation; host controls render validation results.
5. Publication: use the same submitted CodeGrid program for all required cases;
   server re-evaluation is authoritative. Preserve visibility, signing/build
   identity, and publication validation rather than treating local preview as
   publish approval.
6. Delivery: rebuild affected browser/server level WASM and bindings, record
   actual provenance/hashes, replace examples and fixtures, update documentation
   and consumer tests, and verify actual native and WASM hosts.

Presentation must make color, number, weight, and face readable at every stack
height. Flip animation consumes no additional round and must tolerate pause,
fast execution, replay, and reduced animation. Shared stations render once.
Inspect current Steam/Web integration before promising a complete game screen;
the downstream root currently records the Steam game screen as being rebuilt.

## 4. Acceptance matrix

| Area | Required evidence |
| --- | --- |
| Bytes | Round-trip all 192 valid module values; reject all 64 color-3 bytes; zero remains a module; 255 exclusively means empty hand; XOR changes only bit 2; two flips restore every byte |
| Layout | Six single stations; ten dual stations; exact shared aliases; complete six-direction turn cycle |
| Actions | Every station/action/hand combination; empty input; occupied FLIP; stack capacity 4; all weight pairs |
| Rounds | All four shared GRAB/DROP combinations; no-op conflicts; invalid A/B with no partial scene commit; output completion during A cannot skip B; incomplete frame on termination |
| Modes | Full-byte prefix mismatch/overflow; immediate sequence success; immediate final-layout equality after complete valid rounds; conservation implies no missing held modules; ignored orientation; shared station serialized once |
| Cases | Different initial states; one program across all cases; fresh VM/world per case; hidden-case details excluded |
| Validation | Strict field/type/range checks; duplicate/missing IDs; wrong counts/types; overweight targets; mode restrictions; target/output consistency |
| Resources | Tick/work ceilings; work yields; input/event capacity; checked counters; cancellation; faults; cycle detection; no false PASS |
| Hosts | Identical results and required Debug traces across actual native, browser WASM, and server WASM execution; host slicing does not alter results |
| Editor/publication | Property editing, reorder, invalid import/save prevention, preview, server rejection of unsolved cases, export/import round-trip |
| Visuals | Six/ten station display; one shared stack; all properties readable; correct flip/replay/fast-run states |

## 5. Completion gate and current draft impact

Review downstream consumers in apps/editor/src/scene-level.ts,
packages/client-application/src/editor/scene-level.ts, apps/editor/src/scene-wasm/,
apps/api/src/codegrid/level-eval/, publication adapters, author fixtures, API
contracts, and active Steam/Web presentations. Synchronize affected artifacts,
provenance, documentation, and tests in the same implementation task, following
[mandatory synchronization](downstream-synchronization.md).

Implemented in the current Rust evaluator and synchronized downstream editor
and publication evaluator. Existing unrelated working-tree changes were preserved.
No commit, push, release, production deployment, or player-data conversion was
performed. Current contracts directly replace the unpublished old scene.

Verification: all 607 Rust workspace tests and formatting; 77 complete results compared
across native CLI, actual browser Worker, Node WASM and Wasmtime, including
final-layout, flip, overweight-stack and shared-station cases. All three WASM
Debug traces agree. Rebuilt browser/server level adapters enforce the 64-MiB
linear-memory ceiling. Editor and API WASM/bindings, patch hashes and provenance
were synchronized together from source base 17641b5a2582d43822db3586419204588239ab13,
with source_dirty=true and evaluator build
codegrid-level-source-sha256:df21818653fe066f55f7224bfac23d2608e4cf54aad48f037138a3f9c862f60a.

Downstream checks include workspace architecture/type checks, client-application
228 tests, API-contract 37 tests, editor 88 tests, API 102 tests, editor/API
builds and packaged offline Electron execution. Six actual browser authoring/FLIP
playtest checks pass at desktop 1440px, mobile 375px and compact 320px; screenshots
were visually reviewed. Existing editor chunk-size warnings remain. Publication now uses Official
mode to include hidden cases; a real-WASM pipeline test rejects a solution
passing only the visible case. Publish envelope name/description are removed
before strict Rust scene loading; the signed original envelope is retained.

Current Steam's full game screen is being rebuilt independently. This task
provides module authoring and live playtest visualization in the shared editor,
including station snapshots and face transitions; it does not establish a
finished production game screen, device flip animation, Steamworks acceptance,
or deployed Workerd publication hosting.
