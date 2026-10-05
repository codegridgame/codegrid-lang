# Level Error Identities v1

These identifiers extend the shared append-only
[error registry](codegrid-error-codes.json). They are assigned from typed
detection categories, never message wording. Existing compiler and VM codes
are forwarded unchanged. English messages are non-normative.

| Code | Detection and effect |
| --- | --- |
| `level.invalid` | Invalid logical JSON/schema/capability/metric; no validated level is returned. Details are typed reason and field path. |
| `level.unsupported_format_version` | Unsupported logical format; payload is not interpreted as v1. |
| `level.unsupported_scene_type` | No registered production scene; execution never falls back to ExactIO. |
| `level.program_rejected` | Initial structural or committed generated capability violation; evaluation ends without rating. |
| `level.evaluator_fault` | Reserved umbrella for future or otherwise unclassified evaluator fault categories; no v1 known fault currently emits this code. |
| `level.metric_overflow` | Checked level metric or aggregate arithmetic overflows; result status is Fault and no final metrics or rating are returned. |
| `level.vm_fault` | The shared VM reports a fault during level evaluation; result status is Fault and no final metrics or rating are returned. |
| `level.vm_initialization_fault` | Standard VM initialization fails for an evaluation test; result status is Fault and no final metrics or rating are returned. |
| `level.resource_limit` | Trusted evaluation tick/work/state/output ceiling prevents completion; no official success. |
| `level.cancelled` | Evaluation cancelled; no official success. |
| `level_api.invalid_profile` | Malformed, duplicate, unknown, zero, or unsupported host numeric/identity data; session not created. |
| `level_api.unsupported_profile_version` | Unsupported trusted profile version; session not created. |
| `level_api.unsupported_version` | Unsupported API version; no semantic dispatch. |
| `level_api.invalid_request` | Malformed, duplicate, unknown, incorrect-type request or unsupported operation; no semantic dispatch. |
| `level_api.invalid_configuration` | Unsupported mode/boundary or noncanonical/zero required integer; no evaluation starts. |
| `level_api.seed_required` | Seed omitted without an explicit host seed source; no evaluation starts. |
| `level_api.invalid_handle` | Released, stale, wrong-kind, or other-session handle; no referenced operation executes. |
| `level_api.handle_exhausted` | Checked handle/namespace capacity exhausted; no new handle is created. |
| `level_api.resource_limit` | Request/source/program/retained-state/handle/per-call ceiling exceeded; no requested operation starts. |
| `level_api.response_too_large` | A complete result/diagnostic response exceeds the ceiling; return a complete error, never partial JSON. Evaluation may already have advanced. |
| `level_api.shutdown` | Operation attempted after shutdown; state remains released. |
| `level_api.fault` | Shared API produced an invalid transport response; infrastructure failure, never player success. |
| `level_abi.unsupported_version` | Unsupported portable ABI version; no dispatch. |
| `level_abi.already_initialized` | Attempt to replace an initialized immutable profile; current session unchanged. |
| `level_abi.not_initialized` | Semantic request before trusted initialization; no dispatch. |

Logical validation reasons identify malformed JSON/UTF-8, duplicate/unknown or
missing fields, incorrect type, invalid integer/range, invalid ID/geometry,
unsupported capability/metric/evaluation type, duplicate capability, missing
visible tests, and trusted byte/test/data ceilings. Structural reasons identify
instruction/Attachment/memory permissions, board dimensions, Entry threads,
Function/Custom counts, and generated instruction/memory restrictions.
These reasons supplement the stable category and preserve exact field paths. A missing conditional-prefix permission is `AttachmentNotAllowed`, including in Folded Blocks, where its path identifies the folded cell. A guarded forbidden Primary remains `InstructionNotAllowed`. Obsolete `IF_ZERO` identifiers are unsupported capabilities when loading a level; no new stable error identity is introduced.

Test failures are `WrongOutput`, `IncompleteOutput`, or a forwarded VM runtime
code. Hidden test results allow only `HiddenTestFailed` and generic typed reason;
no private data, location, order, or metric contribution is included. Faults,
resource limits, and cancellation are distinct from player runtime failures.
For known v1 evaluator faults, the terminal result retains status `Fault` and
sets `failure.code` directly from the typed fault detected in the core. The
reserved `level.evaluator_fault` umbrella is not inferred from message text.

Portable buffer calls return zero on invalid live pairs, failed allocation,
or inability to reserve/write a complete response. Validate the called export
and do not interpret zero as JSON or player success. Reservation occurs before
semantic dispatch; an exceptional late response failure does not guarantee
rollback and must not be retried blindly.

## Planned scene API v2 refinement

The [Scene Host Contract v2](codegrid-scene-host-contract-v2.md#2-stable-failure-identity-and-typed-reasons)
selects existing level.test_failed/9027 with typed InvalidOutput,
IllegalOperation, and IncompleteGoal reasons for new dynamic scene failures.
WrongOutput and IncompleteOutput preserve existing byte-comparison identities;
VM errors/faults, resources, cancellation, and constraints retain their categories.
New author cross-field reason spellings, exact paths, visible details, and
HiddenTestFailed redaction are defined there. No new numeric identity or changed
v1 emitted behavior is introduced; v2 support remains unimplemented.
