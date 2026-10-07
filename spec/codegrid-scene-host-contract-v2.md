# CodeGrid Scene Host Contract v2

Status: decided integration contract; Rust scene core, v2 profile loader, native API-2 dispatch, and the selected format-v1 author loader/examples are implemented. Full direct protocol/resource coverage and production host deployment evidence remain open.
Recorded: 2026-10-05.
Authorities: [Scene Spec v1](codegrid-scene-spec-v1.md),
[Scene Level JSON format v1](codegrid-scene-level-json-v2.md), and the
[continuous session design](../docs/scene-session-design.md).

## 1. Versions, compatibility, and capabilities

Introduce Level Host API 2, trusted safety profile 2, browser level binding 2,
and portable Level ABI 2 when this contract is implemented. Language Runtime
API 3 and language server ABI 4 are unchanged. Logical author format and
scene protocol versions remain independently versioned. Existing Level API,
profile, browser binding, and portable ABI 1 remain supported on their existing
path; do not silently reinterpret a v1 request or profile as v2.

Keep the operation names and opaque-handle lifecycle from
[Level API v1](../docs/level-api-v1.md), with `api_version: 2`. Responses retain
`schema: "codegrid.level.response"` and use api_version 2. Portable transport
uses abi_version 2, exact buffer ownership, original JSON text, and the existing
packed response-pointer/length convention. An artifact/session dispatches by
explicit version; incompatible profile/API combinations fail before evaluation.
No v2 operation executes through a v1 fallback.

API 2 accepts the selected Scene Level author envelope with
`format_version: 1`; there is no compatibility branch for the superseded
format-2 scene draft. API 1 keeps its existing request and author-data contract
unchanged. API 1's version identity does not imply that it accepts the current
Scene Level JSON shape, and API 2 does not silently project an older author
shape into the scene envelope. Preserve exact original level bytes for replay
hashing. Supporting API 2 alone does not imply every planned scene runs.

Capabilities report exactly:

| Field | Value |
| --- | --- |
| api_version | 2 |
| level_format_versions | Array of actually loadable numeric versions |
| profile_versions | [2] for an API-2 session |
| scene_protocol_version | 1 |
| evaluation_types | Actually registered family names |
| scene_types | Actually executable scene names, never planned names |
| scene_feedback | Boolean, true only after this feedback operation is implemented |

Only identities executed by the selected API session can be reported as
executable; domain validators or protocol-only code do not qualify. Querying capabilities does not allocate
a level or VM. Build/evaluator identities remain available through the existing
v1 common identity fields. Host build and deployment attestations are separate.

## 2. Stable failure identity and typed reasons

Use the existing append-only level code/number registry. Do not introduce a
second scene error namespace or assign a different number to an existing code.
All new scene distinctions are typed `reason` values assigned at detection.
The v1 behavior and emitted reason values do not change.

| Detection | Public result status | Stable code | Typed reason |
| --- | --- | --- | --- |
| Illegal active action/domain output | TestFailed | level.test_failed (9027) | InvalidOutput |
| Legal action with forbidden interaction/table state | TestFailed | level.test_failed (9027) | IllegalOperation |
| Successful HALT before a dynamic goal/frame is complete | TestFailed | level.test_failed (9027) | IncompleteGoal |
| Wrong expected byte/MechanicalArm robot-state byte | TestFailed | level.wrong_output (9025) | WrongOutput |
| Static comparison HALT before expected bytes | TestFailed | level.incomplete_output (9026) | IncompleteOutput |
| VM runtime error | RuntimeError | Existing forwarded VM code(s); aggregate level.runtime_error (9028) | RuntimeError |
| Initial or generated program capability violation | ProgramRejected | level.program_rejected | Existing typed structural/generated reason |
| Applicable metric limit exceeded | ConstraintExceeded | level.constraint_exceeded (9029) | ConstraintExceeded |
| Checked scene metric/counter arithmetic overflow | Fault | level.metric_overflow | NumericOverflow |
| VM fault/initialization fault | Fault | Existing level.vm_fault / level.vm_initialization_fault | Existing typed fault |
| Trusted evaluation ceiling prevents completion | ResourceLimitExceeded | level.resource_limit | ResourceLimitExceeded |
| Cancelled evaluation | Cancelled | level.cancelled | Cancelled |

InvalidOutput is scene-specific: an action outside 0–4, floor outside 0–9,
an invalid Robot or MechanicalArm action. IllegalOperation includes occupied
GRAB/DROP or a table's unmet preconditions. Neither is a VM runtime error.
Do not turn blocked Robot movement or specified no-ops into IllegalOperation.
OutputMismatch in the logical SceneFailure enum maps to WrongOutput; it does
not introduce an alias for a new public code.

A visible scene failure has exactly code, error_number, reason, and details.
English human messages are optional presentation text and never identify errors.
details is an object containing only fields applicable to that failure:

| Optional details field | Domain / meaning |
| --- | --- |
| tick | Canonical u64 string, committed tick containing the relevant action |
| frame_index | Canonical u64 string; one-based complete action frame |
| actor | A or B |
| value | Offending byte 0–255 |
| output_index | Canonical zero-based u64 string for expected-sequence comparison |
| expected, actual | Byte values for WrongOutput only |
| interaction | Input, Output, BufferLeft, BufferRight, Worktable, or Unavailable |
| worktable_index | Number 0–3, using the author worktables array order |
| pending_actions | Number 0 or 1 at incomplete dynamic HALT |
| runtime_errors | For RuntimeError only: array of exact forwarded VM code/error_number pairs |

Omit irrelevant details; never serialize hidden true inspection or an entire
world through a failure. For initialization failures tick may be absent. A
frame index is assigned only when a complete frame is collected; an incomplete
HALT may include pending_actions without frame_index.

### Author validation reasons

Malformed author files use existing level.invalid with path and a typed reason;
unsupported versions/scenes keep their dedicated existing codes. Reuse existing
MissingField, UnknownField, IncorrectType, InvalidInteger, IntegerOutOfRange,
DuplicateCapability, UnsupportedCapability, UnsupportedMetric, and JSON/UTF-8
reasons where applicable. Add these v2 reason spellings, not new numeric codes:

| Reason | Trigger |
| --- | --- |
| SceneFamilyMismatch | scene_type does not match evaluation_type |
| InvalidActorCount | Selected actor count not 1 or 2 |
| InvalidMapSize | Robot map width, height, or terrain row count is not 16 |
| InvalidTerrainRow | Terrain row is not exactly 16 ASCII characters |
| InvalidTerrainCell | Terrain contains a character other than `.`, `0`, or `1` |
| DuplicateMapIndex | Repeated index within the sparse colors array or within objects |
| InvalidObjectPosition | Color or object index addresses VOID terrain |
| UnsupportedMapObject | Robot object type is not start, patrol, trigger, or door |
| InvalidRobotId | Start object robot is not A or B |
| InvalidDirection | Start direction string is not up, right, down, or left |
| DuplicateRobotStart | More than one start object names the same robot |
| ActorCountMismatch | Start objects do not match configured robot_count |
| MissingPatrolPoint | No required patrol point |
| DuplicateSceneId | Repeated patrol ID, repeated trigger ID, or repeated door ID within its own type |
| InvalidDoorReference | A trigger ID has no matching door ID |
| UnpairedDoor | A door ID has no matching trigger ID |
| InvalidWorktableSlots | MechanicalArm table array length not four |
| UnsupportedWorktableType | Table enum unknown |
| InvalidRobotState | Expected state has illegal color/reserved bits/flags/EMPTY |
| InvalidRobotSequence | Empty MechanicalArm input/expected or expected longer than input |

Range violations in numeric indices, IDs, colors, or byte flags retain
IntegerOutOfRange rather than duplicating those meanings. Invalid direction
strings use InvalidDirection. Wrong JSON type is detected before domain or
cross-field checks. Duplicate sparse indices and IDs point at the later
record's `.index` or `.id` field. InvalidObjectPosition points to the sparse
record's `.index`. InvalidDoorReference and UnpairedDoor point to the
unmatched trigger or door `.id`. If several cross-field failures exist,
inspect cases, colors, and objects in author order; check duplicate indices and
same-type IDs before mechanism pairing.
All reported errors are typed, not chosen by matching message text.

## 3. Trusted Safety Profile v2

A profile is trusted host data, never embedded in author level JSON. It has
the exact required v1 fields, sets profile_version to 2, and adds one required
scene_limits object. Preserve all v1 field names, domains, reservation policies,
and resource categories. All ceilings, including these new fields, are positive
canonical u64 decimal strings. Reject missing, unknown, duplicate, zero,
noncanonical, or out-of-range values. No implicit defaults.

| scene_limits field | Scope and check |
| --- | --- |
| max_input_queue_bytes | Maximum unread VM input bytes per active case, including initial input and every append; reserve before enqueue |
| max_total_input_bytes_per_test | Cumulative initial/enqueued bytes per case, even after READ removes them; charge only complete published appends |
| max_scene_state_units | Total retained scene definition/world/frame/candidate/observation units across the API instance; reserve before retaining/cloning |
| max_scene_frames_per_test | Complete collected action frames per dynamic case, check before actor dispatch |
| max_scene_work_per_call | Per-advance internal scene work ceiling; distinct from VM dispatch work |
| max_total_scene_work | Cumulative actual scene work per evaluation, including retries/draft transitions |
| max_scene_events_per_evaluation | Total retained unacknowledged visible Debug event records per evaluation; no silent dropping |
| max_scene_feedback_bytes | Retained encoded feedback allowance per evaluation; reserved against max_state_bytes at start |

Both old and new ceilings apply; satisfying one never bypasses another.
The existing max_total_test_bytes additionally charges typed scene author data
as its canonical compact JSON UTF-8 length after validation, summed across
all cases including hidden cases. The original level JSON length remains
charged separately under max_level_bytes. No implicit physical heap guarantee.

Scene state units count one for each retained map cell, trigger association,
door-state record, actor, robot instance, table slot, buffer
slot, set entry, frame byte, expected state byte, pending observation byte,
retained feedback event record, and retained round-change record. Owned
observation byte vectors in retained feedback count independently from the VM
input queue and world observations.
Scalar fields embedded in one such record add no extra units. Count immutable
definitions and independent mutable copies separately; a robot moved from
hand to table is still one instance, not two. Shared references count a single
owned definition. VM input bytes are additionally bounded by VM/state-unit
accounting, not charged again as pending observations after ownership transfer.
All reservations are released when their owning handle/evaluation is released.

Prepared feedback pages count their owned event, round-change, and observation
copies independently from stored events. Check complete page peak capacity
before copying; an unsuccessful read changes no cursor or delivery watermark.

Published events retain their immutable JSON encoding. Charge its actual UTF-8
encoding work during staging, including failed attempts. Paging/replay copies
those encoded bytes into a bounded transport envelope and does not re-encode
scene content. Cached encoding bytes remain in feedback retention until storage
is acknowledged and released.

Scene work units are one per actor byte decoded, scene record inspected or
updated, and observation/event byte encoded. Combined inspection/update of
one record charges both operations. A reattempt charges its actual units again.
Pause only at resumable draft boundaries; never publish half a scene transition.
The draft holds tick output/frame/world progress while the VM remains paused.
When the cumulative ceiling prevents draft completion, return ResourceLimitExceeded
and discard unpublished candidate state. If a transition cannot be safely
resumed, reserve its worst-case bound before starting it. Level-load validation
is bounded by trusted author byte/data ceilings, not by advance work counters.

Feedback/events are charged only for visible Debug cases. Lack of feedback
capacity causes ResourceLimitExceeded; do not silently omit required events.
Response overflow retains level_api.response_too_large; callers must not blindly
retry an advance that may already have progressed. Existing bounded-result
reservation requirements remain mandatory. Profile max_response_bytes must
support the fixed v2 envelope/error base (at least 512 bytes); larger visible
feedback is bounded by its separate allowance. Hosted runtimes still enforce
physical memory/fuel/stack limits.

## 4. Evaluation result v2

Keep v1 common terminal statuses, evaluator/build/replay identity, configuration,
constraints, partial_metrics, final_metrics, scoring, and rating semantics.
Add level_format_version, scene_type, scene_protocol_version=1, and use
visible_cases instead of v1 visible_tests. Each visible case is a tagged record:

| Field | Domain |
| --- | --- |
| source_index | Canonical zero-based u64 string; index of a visible author test |
| scene_type | Exact selected scene identity |
| outcome | Passed, WrongOutput, IncompleteOutput, SceneFailure, RuntimeError, or NotCompleted |
| failure | Null or visible typed failure defined in Section 2 |
| comparison | Static/MechanicalArm comparison object or null |
| summary | Allowed scene summary object, no raw VM state |

For static scenes comparison has exactly input, expected_output, and actual_output
byte arrays. MechanicalArm comparison has exactly expected_output and actual_output;
dynamic Robot comparison is null. MechanicalArm never
includes hidden true_inspection author records here. For a hidden case emit no
visible_cases entry. Passed cases have failure=null. Resource/cancel/fault
interruptions use NotCompleted, with the evaluation's typed failure category
at the common result level, not a fabricated player failure.

Summary fields are explicit by scene:

| Scene | summary fields |
| --- | --- |
| ExactIO | {} |
| Robot | frames, rounds, visited_patrol_points, required_patrol_points |
| MechanicalArm | frames, rounds, matched_output_robots |

All summary counters are canonical u64 strings. Scene rounds are public feedback,
not new scoring names. frames counts complete accepted-for-dispatch frames;
rounds counts completed round-end transitions. Terminal A may finish a goal
without completing round end. Summaries follow the last published scene state,
not discarded resource drafts. Partial metrics do not become final metrics on
failure. No summary exposes hidden-case values or hidden case indices/order.

Keep HiddenTestFailed as the only hidden failure identity. It may carry the
coarse categories SceneFailure, RuntimeError, or IncompleteGoal, but no actor,
byte, tick, location, event count, or detailed operation reason. Hashes identify
the trusted full level for replay; they are not permission to expose its data.

## 5. Debug scene feedback operation

API 2 adds scene_feedback, with exactly api_version, operation,
evaluation_handle, after_sequence, and max_events. Handle and cursor are
canonical u64 strings; max_events is a positive canonical u64 string bounded
by trusted retained event/response ceilings. It reads only this evaluation's
visible Debug events; Official evaluations reject it as
level_api.invalid_configuration. Read after Pending or terminal. No direct
VM snapshot, hidden definition, world mutation, arbitrary case selection, or
inspection of a hidden current case is supported.

Response data has exactly events, next_sequence, and has_more. Every event
has sequence, source_index, scene_type, tick, frame_index, actor, kind, data.
Sequences start at 1 and increase only for published visible events; cursor 0
means before the first. tick/frame_index/actor may be null at initialization
or non-actor transitions. Other wide values use decimal strings.

after_sequence acknowledges all visible events up to that sequence and permits
their storage release. It must be no greater than the highest sequence actually
delivered to that handle. Repeating a retained cursor is safe and never
advances the VM. A cursor older than already acknowledged storage is rejected
as level_api.invalid_request; never pretend missing events are an empty page.
A read returns at most max_events complete events and a next_sequence pointing
to the last returned event (or the input cursor for an empty page). Reading
does not acknowledge newly returned events until a later request sends their
cursor. This operation deliberately differs from evaluation advancement and
is not retried as a substitute for an uncertain advance result.

## 6. Event kinds and safe observation payloads

| kind | Exact data fields |
| --- | --- |
| CaseStarted | observation, nullable for empty/event-based initial input |
| ActionApplied | action byte, effect object |
| RoundCompleted | rounds counter, observation byte array (empty for single-arm event input), transitions array |
| InputAppended | bytes array; exactly the newly appended permitted observation/event bytes |
| RobotProduced | state byte, output_index counter |
| CaseEnded | outcome and nullable visible failure |

ActionApplied effect contains a scene_type tag and these required visible values:
Robot from_position/
to_position and from_direction/to_direction; MechanicalArm interaction,
from_orientation/to_orientation, and before_state/after_state (255 means empty).
Position/direction/state/action values are JSON numbers; counts and indices
are decimal strings. MechanicalArm's interaction is a public interaction name
and worktable_index is additionally required only for Worktable. Interaction
identifies the slot faced before the action; Unavailable names the lower-arm
orientation 0. Unpopulated table slots retain their Worktable/index identity
for feedback even though the gameplay interaction is unavailable. No-op actions
are events with unchanged values. Tables/buffers may be named
by public slot index but may not reveal hidden robot true_inspection. Never
include author source records or a raw world snapshot. Unknown kinds/fields
are errors for a fixed version, not silently ignored schema extensions.

Initialization publishes CaseStarted, then InputAppended if bytes are queued,
then CaseEnded for an initial terminal goal. For a frame, publish A's effect
events then B's if dispatched; RobotProduced follows its ActionApplied.
RoundCompleted follows actual round-end publication, then InputAppended.
CaseEnded is last for a visible case. A scene-failure event can retain completed
A effects but emits no RoundCompleted/input append. A resource-discarded draft
emits no speculative events. Single-arm successful grabs use InputAppended
for event-based bytes; RoundCompleted observation is empty rather than
inventing a fixed snapshot.

Event publication/reservation is all-or-none per resumable scene draft. Store
committed actor effects needed on B failure, and reserve their visible event
capacity before publication. Official/hidden execution never constructs these
debug payloads or increments the public event sequence.

RoundCompleted transitions contains only published visible world changes,
in fixed order: table promotions by worktable_index, buffer promotions left
then right, followed by opened doors in numeric ID order.
Irrelevant transition families are empty for a scene. Each record is exactly:

| kind | Additional fields |
| --- | --- |
| TableReady | worktable_index number 0–3, state byte |
| BufferReady | buffer Left or Right, state byte |
| DoorOpened | door_id number 0–255 |

These are public derived scene state, never hidden author records. Inspection
state is revealed only when the Inspection transformation completes, not before.
The sequence order is for serialization only; all promotions occur at the
specified round boundary and never permit B to use A's same-round Pending item.

## 7. Acceptance and delivery

This contract freezes design, not runtime support. Before advertising API 2:

- Implement typed reason detection, v2 profile validation/accounting, exact
  wide values, result variants, feedback cursors, and privacy allowlists in Rust.
- Verify unchanged v1 outcomes/error numbers and explicit mixed-version rejection.
- Test hidden cases, event acknowledgement, stale/future cursors, constrained
  responses, zero/malformed ceilings, queue growth, counter overflow, and
  resource-draft discard without partial publication.
- Compare protocol traces/results under different work slices; scene events and
  semantic metrics must match, though Pending counts/actual retry work may differ.
- Run actual host/ABI comparisons before claiming browser/server parity.

No current v1 capability, profile, result schema, or executable scene registration
is changed merely by this document. Numeric deployment defaults remain host
policy, not gameplay; a local profile example must be labelled accordingly.

Profile replay hashing uses the v1 canonical representation convention: compact
JSON in the existing Rust profile field declaration order, followed by scene_limits
in its table order above, with exact numeric u64 values rather than transport
strings. Include profile_version 2 and every required field. Actual serialized
host requests still use decimal strings. Feedback acknowledgement requests form
part of the deterministic host-call sequence; compare hosts with the same
acknowledgement policy, not a fast-draining host against a stalled one. Feedback
backpressure may terminate a configured Debug session and must not be silently
removed to manufacture a parity claim.

Removed scene compatibility: Baudot, QualityControl, and Elevator are unsupported.
Published error identities remain reserved for compatibility. Removed scene
reason spellings are not part of the current validation table.
The current runtime catalog is ExactIO, Robot, MechanicalArm.
