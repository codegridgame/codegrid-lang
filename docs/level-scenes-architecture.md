# Six-Scene Code Architecture

Status: six protocols are executable through the native Rust API-2 session.
API-1 remains ExactIO-only. Browser/server v2 transports have initial actual-host comparison evidence;
complete protocol/resource coverage remains pending.

## Selection and engine mapping

| Product scene | Evaluation family | Correctness | Current delivery |
| --- | --- | --- | --- |
| ExactIO | ExactIO | Expected byte sequence, existing halt/constraint/outcome rules | Implemented |
| Baudot | ExactIO profile | Expected five-bit code sequence | API-2 and WASM v2 implemented; initial parity verified |
| QualityControl | ExactIO profile | One Item/Batch accept-or-reject decision | API-2 and WASM v2 implemented; initial parity verified |
| Elevator | Dynamic scene | All configured passengers appear and reach destinations | API-2 and WASM v2 implemented; initial parity verified |
| Robot | Dynamic scene | All required patrol points visited | API-2 and WASM v2 implemented; initial parity verified |
| MechanicalArm | Dynamic scene | Expected visible robot-state sequence | API-2 and WASM v2 implemented; initial parity verified |

Terminal is excluded. Robot is the selected identity; MaintenanceRobot in old
examples is not an alias. Robot's current goal is patrol completion, superseding
the earlier repair-device concept. The Rust catalog's SceneKind,
EvaluationFamily, SceneStatus, SceneDescriptor, and SELECTED_SCENES express
native API-2 registration rather than WASM deployment. Existing API-1 hosts
report only ExactIO and an empty scene_types list; API-2 reports the six scenes
that its loader/evaluator executes. Do not advertise planned protocols as executable.

## Shared execution boundary

All scenes use ordinary CodeGrid input/output; there are no game-specific
compiler or VM instructions. Keep definition validation, observation encoding,
action decoding, ordered world transitions, goals, and metrics in Rust above
the language core. Hosts own visualization and ABI conversion.

Baudot and QualityControl use ExactIO comparison with additional domain
validation. Do not add a second evaluator or compare correctness in JavaScript.
The selected author contract uses `format_version: 1` and distinguishes scene
definitions from executable programs. The current loader, examples, and fixtures
use this contract; no legacy scene-format branch is supported.

Dynamic scenes retain one VM throughout a case, append observations behind
unread input, and reset completely between cases. This accepted lifecycle
supersedes the fresh-VM-per-decision sketch in the earlier
[Environment extension design](level-environment-extension.md). Its ownership,
bounded work, cancellation, and privacy requirements remain useful, but its
reset-based execution loop cannot implement the new protocols unchanged.

The shared orchestrator owns case/VM lifecycle, output framing, work slices,
constraints, metrics, cancellation, and privacy-safe results. Each scene owns
validated definitions, persistent world state, observations/actions, goals,
failures, and bounded events. Hosts cannot supply semantic callbacks or build
unchecked executable definitions. Preserve the existing dependency graph.

This describes the current built-in Rust scene boundary. It is intentionally a
closed catalog and does not support player packages. The target architecture
for community scenes is documented in [Custom Scene architecture](custom-scenes-architecture.md):
package execution stays in the application host behind a versioned cooperative
call/reply protocol. Do not extend `SceneKind` or add game-host callbacks as a
substitute for that boundary. Current API v2 capabilities continue to list only
the six scenes implemented by this Rust evaluator.

## Implementation sequence and verification

1. Follow the accepted [continuous session design](scene-session-design.md).
   Migrate to the selected [Level JSON format v1 contract](../spec/codegrid-scene-level-json-v2.md);
   implement the [Scene Host Contract v2](../spec/codegrid-scene-host-contract-v2.md)
   failure, resource, result, and feedback contracts alongside it.
2. Implement domain validation for Baudot and QualityControl above ExactIO.
3. Introduce the shared continuous-case orchestrator with the first dynamic
   scene, then implement Robot, Elevator, and MechanicalArm using it. Add no
   empty crates or executable registration for unimplemented scenes.
4. Project actual supported capabilities, privacy-safe observations, metrics,
   and results through shared Level APIs; adapters stay thin.
5. Cover the [scene conformance plan](scene-conformance-plan.md) and Scene Spec acceptance matrix, case isolation, queue append,
   partial frames, round ordering, constraints, cancellation, hidden-data
   redaction, and deterministic slicing. Compare actual native, browser-WASM,
   and server-WASM results before claiming parity.
