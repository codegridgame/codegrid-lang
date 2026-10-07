# Scene Level JSON v1 Examples

These complete author documents illustrate the selected
[Scene Level JSON format v1 contract](../../spec/codegrid-scene-level-json-v2.md).
The [manifest](manifest.json) records positive and negative validation cases;
it is test-plan metadata, not a logical level file. These examples describe the
author contract and do not by themselves establish loader acceptance or scene
execution conformance.

| Scene | Valid author document | Invalid counterpart |
| --- | --- | --- |
| ExactIO | [exactio.json](exactio.json) | [exactio-invalid.json](exactio-invalid.json): unknown config field |
| Robot | [robot.json](robot.json) | [robot-invalid.json](robot-invalid.json): duplicate object index |
| MechanicalArm | [mechanical-arm.json](mechanical-arm.json) | [mechanical-arm-invalid.json](mechanical-arm-invalid.json): DEFECT plus processed |

Robot configuration contains the static map once under `scene_config`; each
test record contains only `visible`. The map uses 16 terrain rows (`.`/`0`/`1`),
sparse colors, and sparse mutually exclusive start/patrol/trigger/door objects.
Equal trigger/door IDs pair a mechanism. See the Robot section of the author
specification. MechanicalArm demonstrates direct packing of an inspected
NORMAL BLUE robot, with expected visible state 33.

No program is embedded; program rules and scoring describe example author choices. The examples establish shape and invalid data cases, not solvability, scores, or native/WASM parity. Loader validation and full execution conformance are covered by separate Rust and host tests; these documents alone do not establish either result.

Additional ExactIO examples cover byte boundaries and classification using
generic input and expected-output arrays. Unsupported scene identifiers are
covered directly by Rust schema tests and WASM transport tests.
