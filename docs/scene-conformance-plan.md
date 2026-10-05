# Scene Conformance and Delivery Plan

Status: Rust core, API-2 lifecycle, and both WASM adapters implemented; initial actual-host parity verified; full protocol/resource coverage audit pending.
Recorded: 2026-10-05.
Robot revision gate: migrate fixtures and validation to author format v1,
terrain `. / 0 / 1`, sparse colors, unique object indices, mutually exclusive
start/patrol/trigger/door objects, and equal-ID mechanism pairing. Reject objects
or colors on VOID, duplicate indices/IDs, missing starts, and unpaired mechanisms.
Replace earlier initial-patrol/initial-trigger vectors: these are now invalid
author maps because start objects cannot overlap patrols or triggers. Existing
host evidence below predates this revision and must be rerun after implementation.
Authorities: [Scene Spec v1](../spec/codegrid-scene-spec-v1.md) and the
[continuous session design](scene-session-design.md).

## Protocol vectors

These are logical test inputs/expectations, not accepted Level JSON fixtures.
The [v2 author contract](../spec/codegrid-scene-level-json-v2.md) and
[complete examples](../examples/scene-level-v2/README.md) fix their document shape.
The Rust v2 loader now validates the complete author examples. Native protocol and continuous-execution tests are implemented; cross-host vectors remain pending.

| Case ID | Setup / action | Required result |
| --- | --- | --- |
| exact.empty | Empty expected output | Initially Running; silent successful HALT passes; any output fails |
| baudot.range | Author input or expected contains 32 | Reject before VM execution |
| baudot.player-range | Player outputs 32 while active | InvalidOutput |
| qc.item | Color input [1], expected [1], output 1 | Pass on first output |
| qc.batch | Input [1,1,2], expected [0], output 0 | All three inputs queued initially; one decision passes |
| qc.reserved | Packed item low bits are 3, e.g. 255 | Author rejection |
| elevator.no-service | Start floor 2; initial passenger 2→7; output 2 | No boarding, no round/stop/distance; still Running |
| elevator.trip | Same setup; targets 3,2,7 | First leave, then board at 2, then deliver at 7; distance 7, stops 3; Pass |
| elevator.order | Two elevators reach same floor with waiting passengers | A boards first; B cannot board the same passengers |
| elevator.arrival | Unread initial pairs remain; one advanced round ends | Append next sequential [from,to] behind them exactly once |
| robot.position | Position (2,3) | Encoded byte 50 |
| robot.geometry | At (2,3) facing UP, walkable same-height (2,2), FORWARD | Move to byte 34; left/right turns use modular direction rules |
| robot.jump | Adjacent cell same height versus different height | JUMP stays versus moves; never skips an intervening cell |
| robot.swap | A and B attempt to enter each other's current cell | Neither moves |
| robot.trigger | A enters trigger, B attempts its closed door that round | B is blocked; door opens only at completed round end |
| robot.initial-goal | All required patrol points occupied at start | Pass after applicable constraints, with zero action rounds/ticks |
| arm.initial-state | Grab BLUE robot, hidden DEFECT | Visible state 13 (UNINSPECTED BLUE), not 5 |
| arm.inspect | DROP that robot on Inspection, complete round, GRAB next round | Ready state 5 (DEFECT BLUE); earlier same-round GRAB cannot obtain it |
| arm.repair | Repair state 5 | Ready state 1 (NORMAL BLUE) |
| arm.direct-pack | Pack state 1 without Processing | Ready state 33 (packed NORMAL BLUE) |
| arm.process-pack | Process then pack state 1 | State 17 then 49 |
| arm.invalid-state | Reserved bits, color 3, DEFECT+processed, or expected 255 | Author rejection |
| arm.pending-buffer | A DROP to shared buffer, B GRAB same round | B obtains nothing; robot becomes Ready only at round end |
| arm.remove | DROP any legal robot state onto Removal | Robot immediately disappears; no Processing/Ready occupancy |
| arm.final-output | Final expected robot output by A; B would act illegally | Stop after A; B is not validated/dispatched; constraints still checked |

Every table precondition, GRAB/DROP matrix entry, input/output conveyor rule,
single-arm event append, dual-arm snapshot, and map validation rule also needs
direct positive/negative cases from Scene Spec Section 11.

## Session invariants

| Case ID | Required evidence |
| --- | --- |
| session.tick-append | Output triggers observation; next tick READ sees it; identical under small/large advance slices |
| session.queue-tail | Existing unread [9,8] plus new [2,7] becomes [9,8,2,7] |
| session.partial-frame | A output alone changes no world; survives Pending; B completes exactly one frame |
| session.invalid-B | Valid A changes world, illegal B fails; A effect remains, round-end work does not run |
| session.terminal-A | A completes goal; invalid collected B is ignored |
| session.vm-rejection | Output attempted in a failed VM tick never changes world/input/frame |
| session.guard | Generated forbidden Primary and potentially goal-completing output in committed tick; ProgramRejected before scene action |
| session.halt | Valid action and HALT in same committed tick; Process action first; completed goal may pass, incomplete goal/frame fails |
| session.constraint | Goal and constraint breach coincide; ConstraintExceeded, never Passed |
| session.state | Registers, memory, stacks, mutable code, input, and PRNG persist within case; No per-action VM reconstruction |
| session.reset | Next case begins after a self-modifying, stateful case; Standard state and original verified code; no queued bytes/frame/world leak |
| session.capacity | Observation cannot be fully reserved; ResourceLimitExceeded; no partial append or speculative round events |
| session.no-progress | Elevator outputs current floor indefinitely; Cumulative tick/work ceiling terminates independently of round count |
| session.privacy | Hidden case/inspection and visible permitted observation; No hidden definitions/traces/metrics escape projections |

## Delivery gates and evidence

1. Implement the decided v2 author schema, Elevator metric registry, and
   [Scene Host Contract v2](../spec/codegrid-scene-host-contract-v2.md) failure/profile/result/event rules. Distinguish logical examples from
   loader-accepted fixtures and planned scene IDs from executable capability IDs.
2. Add focused validation and transition tests in the Rust level layer; use
   shared VM semantics without a TypeScript scene evaluator or parser.
3. Add the continuous-case loop and input-append tests before registering the
   first dynamic scene. Compare step-based and bounded advance execution.
4. Register scenes individually only after their schema and direct protocol
   cases pass. Extend the existing conformance manifest without changing its
   older IDs or rewriting recorded pass counts as new evidence.
5. Verify native CLI first within current local delivery scope. Actual browser
   and server WASM comparisons are required before claiming parity or exposing
   scene support on those hosts; production Steam/backend integration remains
   a separate delivery gate.
6. Update [traceability](level-traceability.md) with commands, fixture counts,
   results, limitations, and runtime versions from actual runs. Compilation
   and this plan are not execution evidence.

The initial loader stage implemented strict v2 author loading, typed definitions for all six scenes, and Elevator SUM/constraint metric policy. Subsequent implementation and host-registration evidence is recorded below.

Validation evidence: `cargo test -p codegrid-level-core` passes 19 unit tests, 16 existing evaluator tests, and 7 v2 schema/metric tests. The v2 suite checks all 12 author examples, exact rejection paths, all 256 MechanicalArm state bytes (21 legal), Robot associations/starts, strict envelopes, trusted size limits, checked Elevator metric sums, and scene-specific metric restrictions.

## Host v2 acceptance additions

- Exercise every scene validation reason and illegal action/interaction/goal
  reason, preserving existing v1 error numbers and hidden redaction.
- Reject missing/unknown/zero/noncanonical scene_limits fields; test each
  independent queue, cumulative input, frame, state, work, feedback ceiling.
- Interrupt/resume a scene draft without rerunning the VM or publishing a
  partial transition; count actual retry work and counter overflow correctly.
- Test Debug visible feedback initialization/action/round/end ordering, cursor
  acknowledgement/replay, stale/future cursors, and bounded complete responses.
- Reject Official feedback; construct no hidden scene events or public sequence
  gaps that encode hidden processing. Compare results with identical acknowledgement
  policy and trusted profiles across actual hosts.
- Verify full v1 compatibility and v2 capability/version dispatch, not merely
  compilation of new result types.

## Scene protocol implementation evidence

The Rust `scene_protocol` layer constructs scenes from validated definitions,
frames dual actor bytes before dispatch, stops at terminal A, and preserves A
effects when B fails. Private `scene_world` transitions implement Elevator, Robot,
and MechanicalArm; static comparisons cover ExactIO, Baudot, and QualityControl.
The Rust-only VM `append_input` reserves whole appends and preserves unread data
and execution metrics.

`cargo test -p codegrid-vm -p codegrid-level-core` passes 57 level core tests
and 165 VM tests. New evidence includes nine world unit tests, six validated
protocol integration tests, and two input-append tests, including append after a
rolled-back work interruption. These are focused transition tests, not proof of
all acceptance vectors or host parity. Continuous evaluation, bounded scene work,
feedback projection, host v2 integration, and actual WASM comparisons remain
required before new scene capability registration.

## Continuous case implementation evidence

`SceneCaseSession` adds ten focused integration tests for the accepted continuous
VM/input publication rules and independent ceilings. These pass under
`cargo test -p codegrid-level-core --test scene_session`. Host v2, complete event
work/accounting, multi-case privacy/scoring, and browser/server conformance are
still outstanding delivery gates.

## Multi-case and feedback-storage evidence

The shared `SceneEvaluationSession` now evaluates the selected visible/hidden
cases with one fresh continuous case session per test, visible-only cumulative
metrics/constraints/scoring, per-evaluation work and retention ceilings, and
coarse hidden failures. Nine focused integration tests cover slice equivalence,
case reset, Debug continuation, hidden redaction, cumulative Elevator constraints,
cancellation, feedback/work limits, comparison privacy, local failure ticks,
and forwarded VM error identities.

Five additional `SceneFeedbackStore` tests verify exact encoded-byte accounting,
atomic staging/discard, cursor acknowledgement/replay/staleness, bounded pages,
Official rejection, and exact wide-integer serialization. Prepared reads change
no delivery watermark until the host can return a complete response. This
store was not yet connected to scene event production at that stage; the
subsequent Debug execution evidence below verifies that connection. Host v2 and actual
browser/server comparisons remain required before capability registration.

## Debug execution verification

Native tests verify atomic event publication, reserved case termination,
actor ordering, terminal A handling, preservation of A effects on B failure,
MechanicalArm inspection disclosure and delayed buffers, initial goals,
Official feedback exclusion, and capacity reclamation after acknowledgement.
Definition unit totals are cached during author loading; case advancement does
not repeatedly scan all immutable maps. Static incomplete output does not
include dynamic `pending_actions` details. These checks do not establish host
v2 registration or browser/server WASM parity.

## Trusted v2 profile loader evidence

The API layer exposes a separate `SafetyProfileV2` loader and checked conversion
to the core's `SceneLimits`. Four integration tests cover explicit version
separation, every required scene limit, canonical exact integers through u64::MAX,
missing/unknown/duplicate fields, and invalid trusted Rust configurations.
Existing API-1 lifecycle tests continue to pass. Profile loading does not register
API-2 operations or establish instance-wide scene accounting or WASM parity.

## API-2 result projection evidence

The API layer's `project_scene_result` serializes only the privacy-safe core
result. Shared common identity/configuration/constraint/scoring projection is
used by both API versions; scene failures reuse the core feedback serializer.
Four new integration tests compile Full `.cg`, execute all six loaded scene
examples, and check tagged comparison shapes, mechanical-arm author-state
exclusion, coarse hidden mismatch, exact wide integers, and complete-response
capacity rejection. These executions verify native projection, not fixture
solvability, API-2 request dispatch, or WASM parity. API-1 lifecycle coverage
remains a required regression check after common projection changes.

## Native API-2 lifecycle evidence

`LevelApiV2` uses explicit version-2 request dispatch, strict author format 2,
shared source compilation, isolated non-reused handles, full scene results,
transactional Debug feedback acknowledgement, release, and shutdown. API-1
paths remain format-1 ExactIO. Five lifecycle integration tests cover all six
advertised scenes, version/field rejection, case execution, feedback replay and
stale cursors, Official feedback rejection, cross-instance handles, and release
of reserved scene capacity. Result projection tests additionally execute the
six compiler-produced fixtures and verify privacy and exact integer output.

This section records the current implementation's earlier author-format draft.
The selected Scene Level contract now uses `format_version: 1` with no legacy
scene-format branch. Loader, fixtures, and these lifecycle inputs must migrate
before this evidence can establish conformance to the revised author contract.

The current native reservation policy assigns all remaining scene-state units
to one retained evaluation. A second evaluation returns a resource limit until
that handle is released. This conservative reservation prevents cross-handle
oversubscription; it is not a guarantee of concurrent evaluation throughput.
Profile replay hashing uses compact canonical profile wire JSON with all
ceilings represented as decimal strings. Level/source replay hashes use exact
original input bytes. Browser/server transport v2, actual WASM comparisons, and
complete cross-host resource/event-work audits remain delivery gates.

## Actual scene host execution evidence

The initial `fixtures/scene-v2/conformance-v2.json` manifest supplies six successful
Full `.cg` scene programs, each executed in Debug and Official modes, and two
small-work-slice variants. CLI integration tests invoke these files through
`evaluate --api-version 2`. Browser `SceneLevelSession` and portable ABI 2 use
the same Rust API; v1 constructors/transport requests remain supported.

`scripts/test-scene-hosts.mjs`, the actual Chromium module Worker, and Wasmtime
`level-parity --scene-v2` produce separate reports. `compare-scene-hosts.mjs`
compares every complete result to native CLI and every Debug event to the
portable report. All 14 runs match, including MechanicalArm inspection/packing,
Elevator boarding/delivery, Robot patrol, Official event exclusion, and slice
trace equivalence. Encoded 64 MiB memory limits, zero portable imports, and
Wasmtime fuel exhaustion are checked. These are actual executions, not parity
claims inferred from compilation. Full protocol-vector and resource/event-work
coverage audits remain pending; this initial manifest is not that complete audit.

## Feedback peak and cached encoding verification

Prepared pages now reserve owned event/observation/round-change copies against
the evaluation's remaining scene units before cloning. Failed page preparation
leaves acknowledgement and delivery unchanged. Published event encodings are
retained; the shared API reserves a complete response envelope and copies those
bytes without re-encoding scene payloads on every replay. Two focused store
tests check exact page capacity, no cursor change on rejection, cached-body
identity, exact original encoding work, and replay without new retention.
Existing API lifecycle tests verify complete feedback/replay and stable results.
Full protocol-vector and broader resource-accounting coverage remain separate.
