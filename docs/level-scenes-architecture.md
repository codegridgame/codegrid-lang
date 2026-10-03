# Five-Scene Code Architecture

Status: selected product architecture. The Rust catalog is implemented; only
ExactIO evaluation is executable. This document does not define new wire fields
or scene protocols. See the [Environment design](level-environment-extension.md)
for the future typed scene interface and registration acceptance checklist.

## Selection and engine mapping

| Product scene | Evaluation family | Correctness | Current delivery |
| --- | --- | --- | --- |
| ExactIO | ExactIO | Exact expected output plus existing halt, constraint, and outcome rules | Implemented |
| Baudot | ExactIO | Entire encoded or decoded message matches expected bytes | Planned protocol and authoring |
| QualityControl | ExactIO | Every item's classification matches expected bytes | Planned protocol and authoring |
| Elevator | Environment | All passengers reach their destinations and configured constraints hold | Planned engine and protocol |
| Robot | Environment | All faulty devices are repaired and configured goals and constraints hold | Planned engine and protocol |

Terminal is excluded. Robot is the selected product identity. MaintenanceRobot
in historical specification examples is not a registered identity or a wire
alias; any migration or alias requires a recorded protocol decision.

`codegrid_level_core::scenes` contains SceneKind, EvaluationFamily, SceneStatus,
SceneDescriptor, and SELECTED_SCENES. This catalog expresses product selection,
not load acceptance. Keep host capabilities sourced from actual executable
support. The current API still reports ExactIO and an empty scene_types list.

## Shared execution boundary

All five scenes use ordinary CodeGrid input and output. Do not add MOVE,
REPAIR, ACCEPT, or elevator instructions to the compiler or VM.

Baudot and QualityControl reuse the existing validated ExactIO tests and
EvaluationSession. A level author supplies input and expected output as bytes;
the evaluator remains unaware of paper tape, characters, products, or artwork.
Their future domain validators and authoring transformations belong in the Rust
level layer, while visual labels and animation belong in the game host. Do not
add a second evaluator or compare correctness in JavaScript.

Keep the current level JSON unchanged: evaluation_type is ExactIO for these
three families of static tests. There is currently no product scene field.
A future versioned presentation identifier can distinguish the visuals without
changing correctness; it must have an approved schema before loader support.
Do not insert unrecognized fields into current strict level JSON.

Elevator and Robot use the proposed shared Environment session. A future
validated evaluation sum type will dispatch ExactIO to the existing session and
Environment to the scene orchestrator. Preserve opaque validated construction;
raw JSON must never directly construct executable definitions or state.

The Environment orchestrator owns decision lifecycle, fresh VM construction,
work slices, committed action output, constraints, metrics, cancellation, and
privacy-safe results. Each typed scene implementation owns validated definition,
goals, persistent state, observation encoding, action decoding, atomic world
transitions, goal predicates, scene failures, and bounded public events.
Use static Rust registration inside codegrid-level-core; hosts cannot provide
semantic callbacks. Follow the existing Environment interface design rather
than introducing a second competing scene trait.

## Protocol decisions still required

Baudot needs a fixed encoding variant, letter/figure shifts, framing, permitted
characters, and direction of conversion. QualityControl needs bounded item
records, classification values, framing, and test construction rules. Their
presentation data must not reveal hidden tests.

Elevator needs floor and passenger bounds, observations, command frames,
boarding and movement order, time progression, terminal failures, and metrics.
Robot needs map and position_id bounds, device states, observation frames,
action values, collision and repair rules, goal checks, and metrics. Position
followed by device state is the selected observation concept; its byte encoding
and numeric domains remain to be specified. Example action numbers from the
conversation are design examples, not published runtime tokens.

A lifecycle conflict must be resolved before Environment implementation: the
conversation suggests using VM memory across robot actions, but Level Core v1
requires a fresh standard VM for every Environment decision. Under the current
contract only world state persists. Supporting a continuous VM would require
an explicit recorded change covering memory, input framing, slicing, metrics,
and termination. This architecture preserves the current normative reset rule.

## Implementation sequence and verification

1. Specify Baudot and QualityControl protocols and fixtures; reuse ExactIO
   correctness, metrics, and result handling. Keep product metadata separate
   from executable Environment registration.
2. Resolve Environment framing, priorities, and the VM lifecycle decision in
   the decision log. Introduce a validated evaluation variant and shared
   Environment session only with the first actual scene implementation.
3. Implement Robot with typed validation, bounded observations/actions, atomic
   transitions, goals, and visible events; then implement Elevator using the
   same orchestrator. Add no empty scene crates or placeholder implementations.
4. Project registered capabilities and privacy-safe results through the shared
   Level API. Browser/server adapters remain ABI conversion only.
5. Test malformed definitions, invalid actions, initial/completed goals,
   constraints, cancellation, deterministic seeds, hidden-data redaction, and
   slice invariance. Compare actual native, browser-WASM, and server-WASM
   execution before claiming cross-host parity.

No new Cargo dependency is needed for the catalog. Scene execution remains
above model/compiler/VM, using the existing one-way workspace dependency graph.
