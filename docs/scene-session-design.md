# Continuous Scene Session Design

Status: VM append, scene transitions, continuous cases, multi-case evaluation, native API 2, and WASM adapters implemented; initial actual-host parity verified; full direct-vector and resource audit pending.
Recorded: 2026-10-05.
Authority: [Scene Spec v1](../spec/codegrid-scene-spec-v1.md).

## Ownership and dependency boundary

The Rust level layer owns one case session: immutable validated scene definition,
mutable world, actor-frame buffer, case metrics, one VM, and bounded feedback.
The compiler produces verified IR; the VM owns ordinary register, memory,
stack, mutable-code, execution, PRNG, and input/output semantics. Scene action
numbers must not become language instructions. Host adapters convert data only.

The conceptual sketch below describes ownership, not the exact implemented Rust layout:

```rust
struct SceneCaseSession {
    vm: VmSession,
    world: SceneState,
    pending_frame: ActionFrame,
    scene_metrics: SceneMetrics,
    limits: TrustedSceneLimits,
    outcome: SceneOutcome,
}
```

Create a new session for each case from verified initial code and standard VM
state. Within that session never recreate the VM merely because a round ends.
Start the next case only after disposing of mutable code, remaining input,
world state, partial frame, metrics, and PRNG state from the previous one.

## The append boundary

An internal VM operation must append explicit input bytes at a committed tick
boundary. It must reject mutation of a VM in an unfinished tick, preserve unread
bytes and their order, and neither execute instructions nor increment language
metrics. The VM must not invoke scene callbacks or load scene definitions.
Validate/reserve the whole appended vector; never append a partial vector.
Allocation/ceiling failure belongs to a typed resource surface, not a language
runtime error. Empty append has no semantic effect.

Keep this operation internal to the Rust evaluator initially. The [Scene Host Contract v2](../spec/codegrid-scene-host-contract-v2.md)
exports evaluator operations and Debug feedback, not arbitrary public input
mutation. Existing external APIs must not acquire a host append surface.

## Advancing a case

Advance is a loop over the shared deterministic VM step transition. The caller
supplies a work slice; the session retains state on Pending. In order:

1. Check cancellation and trusted cumulative resource ceilings.
2. Attempt one outer tick using the remaining work budget.
3. Handle VM fault/error or interrupted work before scene output.
4. On commit, record VM metrics and apply generated-code guards, including
   transient committed Custom mutations using the existing evaluator guard.
5. Process newly committed outer output exactly once. Buffer an incomplete
   frame. For a complete frame dispatch/decode A, then B only if still Running.
6. Retain completed actor effects on a scene error. Do not promote pending
   doors/tables/buffers or append a snapshot after terminal scene failure.
7. On a nonterminal completed round, apply round-end work and compute the next
   observation. Robot and MechanicalArm WAIT actions still advance rounds.
8. Check constraints and terminal rules before appending any observation. If
   still Running, reserve complete observation capacity and publish the
   candidate successful round and queue append together, then permit the next
   VM tick. A terminal goal or constraint failure publishes completed actor
   effects without appending more input. Never resume the VM while scene output
   is unprocessed.

VM and world are distinct transaction domains: a rejected VM tick produces no
scene effects; a scene error after VM commit does not undo the VM. B failing
does not undo successful A. Round-end promotion is deferred until both actors
have run without terminal outcome. This is not an all-or-nothing actor round.

If input-capacity reservation fails after computing a successful candidate
round, terminate as ResourceLimitExceeded without publishing that candidate
round or any appended bytes. Retain the last published scene/input boundary
for bounded feedback, alongside the already committed VM state. Do not expose
the temporary candidate as a completed world transition or emit its events.
This resource outcome is not a player scene failure and needs no fabricated
round rollback event.

Initial Robot completion is checked after structural/static acceptance. Award
no success if applicable constraints fail, and create no artificial tick/action.
Successful goals stop actor dispatch immediately; constraints can still prevent
the final Passed level result. Follow Scene Spec Section 2.4 for terminal order.

## Determinism and replay

Actor order, output consumption, and queue append depend on committed ticks,
not wall-clock time, animation, callbacks, or host slice size. Store a partial
frame across Pending returns; do not reconsume previously handled output.
Do not reseed the VM on every action. Preserve the ordinary per-thread PRNG
streams for the whole case.

Replay records must identify scene schema/spec version, validated definition,
program, initial case seed, Custom limit, and trusted profile.
Use the case-seed derivation in the selected [Scene Level JSON format v1 contract](../spec/codegrid-scene-level-json-v2.md). Do not reuse
the older decision-seed algorithm to reset or reseed a running case. Comparing
only final output is insufficient: compare world, queue framing, status, metrics,
failure category, and public-event order with several host slice sizes.

## Resource accounting and privacy

Extend the existing [safety accounting](level-safety-accounting.md) for actual
retained scene definitions, world records, frame bytes, conveyor/hand/table/buffer robots, patrol/trigger/door sets, and
pending observation bytes. Each retained input byte is counted once in its
owning queue; cumulative appended bytes require a separate checked ceiling.
Keep scene metrics, VM Operation Count, work units, and rounds distinct.

Use trusted positive ceilings for cumulative ticks/work, queue size, cumulative
input, feedback/events, and retained world state. The v2 host contract fixes scene_limits wire fields and accounting; numeric
deployment defaults remain trusted host policy. The old v1 profile does not
account scene memory. Checked counter
overflow is an evaluator/resource outcome, not wrapped gameplay data.

Hidden cases must not disclose definitions, observations, action sequences,
world locations, true inspection, or metric contribution. Visible inspection
results are permitted observations, not a leak of the whole hidden definition.
Source snapshots and public scene projections need explicit allowlists. Failed
or resource-terminal candidates emit no speculative public events. Completed
A effects retained after B's scene failure may appear in permitted visible
feedback; they are not speculative or hidden-case disclosures.

## Integration gates

Before the first executable dynamic scene:

- Migrate the loader and examples to the selected [Level JSON format v1 author contract](../spec/codegrid-scene-level-json-v2.md), which has no legacy scene-format branch; add logical validation before registration.
- Implement the append operation and continuous-case loop using existing
  compiler/VM APIs and the existing dependency direction.
- Implement the fixed typed scene reasons and visible result/event allowlists
  in the [Scene Host Contract v2](../spec/codegrid-scene-host-contract-v2.md);
  use registry identities at actual detection sites.
- Implement its profile_version 2 scene_limits and exact Level API/browser
  binding/portable ABI 2 transport, preserving v1 paths.
- Run the [conformance plan](scene-conformance-plan.md); advertise only the
  scenes that are registered and have direct execution evidence.

Current hosts remain ExactIO-only until these gates are implemented.

## Continuous case implementation evidence

`SceneCaseSession` now keeps one VM for each case and buffers committed output
while a scene work slice is insufficient. Atomic transitions reserve a worst-case
bound before cloning/dispatch and charge measured transition/copy work. No VM
tick is rerun to obtain more scene work. VM and scene budgets remain independent.
Initialization reserves world and observation state plus a conservative work
bound before constructing the candidate. Robot bounds include the map copy,
map scan, actor start effects, and initial observation; shared definition units
remain separate from the mutable world. Insufficient slices perform no world
initialization, and a bound above the per-call ceiling is a resource failure.
The session checks queue, cumulative input, scene frame/state/work, VM retained
state/output/tick/work limits and publishes world/input candidates together.
Generated instructions use the existing program-rule validator.

Ten case-session integration tests cover continuous READ observations, unread
queue order, small/large VM slices, pending scene output, discarded input-limit
candidates, HALT/constraint priority, exact tick/work completion, zero-tick Robot
goals, scene counters, VM rollback, and cancellation. This is case-level Rust
evidence only. Multi-case visible/hidden projection, feedback/event work and
reservations, API-instance aggregate ownership, host profile conversion, and
actual native/WASM comparison are still required. The complete Scene Host v2
resource/event contract is not yet implemented or advertised.

## Multi-case projection and feedback storage

`SceneEvaluationSession` now shares a validated level definition across fresh
cases. Each visible case contributes its cumulative VM/scene snapshot exactly
once relative to the prior visible baseline; hidden cases contribute no raw
metrics or public failure details. Cancellation retains only permitted visible
partial metrics and NotCompleted case feedback.

The typed feedback store supports exact event byte accounting and exclusive
stage/commit publication. Its prepared read/commit protocol permits response
capacity checks without acknowledging or marking an undelivered page. Actual
scene event generation and joint reservations with case/result publication are
not yet wired; the complete API-instance resource contract and host paths still
need implementation and cross-host verification.

## Debug execution verification

Native tests verify atomic event publication, reserved case termination,
actor ordering, terminal A handling, preservation of A effects on B failure,
MechanicalArm inspection disclosure and delayed buffers, initial goals,
Official feedback exclusion, and capacity reclamation after acknowledgement.
Definition unit totals are cached during author loading; case advancement does
not repeatedly scan all immutable maps. Static incomplete output does not
include dynamic `pending_actions` details. These checks do not establish host
v2 registration or browser/server WASM parity.
