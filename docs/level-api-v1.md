# Level Host API v1

This contract versions level operations independently of Language Runtime API
v3. The logical level format and evaluator contract remain version 1. All hosts
call `codegrid-level-api`; they must not implement level semantics.

## Lifecycle and operations

A host creates an isolated API with an immutable trusted safety profile.
`capabilities` reports supported versions, ExactIO, and no production scenes.
`load_level` validates original JSON text. `compile_program` invokes the shared
compiler once. Successful operations return opaque handles. `start_evaluation`
takes level/program handles, Debug or Official, explicit Exit/Wrap, a positive
Custom execution limit, and an optional shuffle seed. An omitted seed requires
an explicit host seed source; absence of that source is an API error.

`advance_evaluation` performs bounded work and returns Pending or a terminal
result. `evaluation_result` returns Pending or the same retained terminal
result. Release removes the associated retained state; releasing a pending
evaluation cancels it. Shutdown drops all retained state and rejects further
operations. Handles are API-instance-local, monotonic, never recycled, and
checked for kind, ownership, release, and exhaustion. They are not durable IDs.
Releasing a level/program does not invalidate evaluations already started from
it. Host slicing never exposes VM snapshots, test order, or raw events.

## Wire representation

Requests use `api_version: 1` and an `operation` name. Responses use
`schema: "codegrid.level.response"`, `api_version: 1`, and one of `ok`,
`pending`, `result`, `source_rejected`, `level_rejected`, or `error`.
Errors have a stable `code`, a human English `message`, and typed details.
Source diagnostics preserve their shared compiler code and UTF-8 byte spans.
Level errors preserve their category, typed reason, and JSON field path.

Every u64 value, seed, metric, work counter, handle, and source offset is a
canonical unsigned decimal string: `0` or a nonzero leading digit followed by
digits, with no sign or leading zeros. Bytes, versions, geometry, and stars
are JSON numbers. Collections with key semantics use stable lexical ordering.
Original logical JSON is transported as text, so its wide numeric literals are
never parsed through JavaScript Number. Unknown fields and duplicate keys are
rejected at request/profile boundaries as well as in logical levels.

A result includes level ID/version, evaluator contract/build identity, mode,
effective boundary, Custom limit, profile ID/version, shuffle and derived VM
seeds, terminal status, permitted failure details, visible test results,
constraint outcomes, partial/final metrics, scoring values and targets,
directions, per-metric ratings, and optional overall rating. Replay includes
SHA-256 of the exact UTF-8 source and level JSON text, and SHA-256 of the Rust
profile's canonical JSON representation. The profile representation uses struct
field declaration order, numeric exact u64 values, and compact JSON; clients
receive only its digest. Hosts retain exact inputs and artifact identity.
Debug success certifies only the visible subset. Hidden failures expose only
`HiddenTestFailed` and an optional typed category/reason. Hidden input, output,
indices, order, snapshots, events, and metrics never enter any response.

## Safety and bounded responses

The named test profile is [level-local-v1](../fixtures/levels/profiles/local-v1.json).
It is local verification data, not a production recommendation. Hosts choose
trusted deployment profiles; neither source nor level JSON can replace them.
All count/byte/work limits are positive and checked before retaining inputs.
Limits cover source and level bytes, test count, total test bytes, boards/cells,
retained state, handles, output bytes, per-test committed ticks, per-call work,
cumulative dispatched work (including rolled-back retries), and response bytes.
Runtime metric counters remain independent from safety accounting.

`max_state_bytes` bounds retained API source/level/result encoded data, rather
than guessing Rust heap allocations. Each evaluation reserves its input clones
and a conservative feedback/result allowance before creating a handle. The
effective allowance is returned as `configuration.safety.max_feedback_bytes`.
It is the smaller of available state capacity and the larger of the response
ceiling or the fixed-result base reservation. If the base cannot fit, starting
the evaluation fails as a host resource error. This reservation remains charged
until release or shutdown. The exact bound is specified in
[safety accounting](level-safety-accounting.md).
`max_state_units` bounds deterministic VM
logical storage cardinality; its VM accounting includes program cells, threads,
stacks, memory, metric identities, and byte queues. It is not a physical heap-byte
claim. Embedding runtimes must enforce physical memory ceilings. Transient Custom
execution is bounded by deterministic work and runtime memory controls.

After a work interruption the next slice may grow geometrically up to the
per-call ceiling. A retry starts the entire uncommitted tick again. Cumulative
dispatched work is charged for every attempt. If the ceiling cannot permit
progress, terminate as ResourceLimitExceeded, never RuntimeError or success.
At an exact safety ceiling a fully completed evaluation can succeed; otherwise
resource termination is distinct. Cancellation and faults have no final metrics
or rating. A response exceeding its ceiling becomes one complete bounded error;
it is never truncated. Projection bounds are checked before building result or
source-diagnostic JSON collections, and serialization uses a writer that refuses
to grow past the response ceiling. Conservative bounds may reject a response
whose eventual encoding would be smaller; this is trusted resource policy.
Trusted level byte/test/data ceilings produce `level_api.resource_limit`, not
`LevelInvalid`. Malformed logical levels retain their distinct level category.
Host allocation failures/traps are host infrastructure
failures, not player test failures.

## Compatibility

Registry v2 adds four-digit `error_number` strings to errors, rejected-source
diagnostics, failed test outcomes, and failed evaluation results. Runtime outcome
arrays retain `codes` and add parallel `error_numbers`; both redact hidden runtime
details. Original status/code fields retain their meanings. See the
[complete numeric table](../spec/codegrid-error-numbers.md).

New unsupported operations/versions fail explicitly. Language APIs and adapters
keep their current versions and contracts. Profile, format, evaluator, API,
browser binding, portable ABI, and build identities are independent. Clients
must query capabilities and must not treat a local Debug result as an official
backend certification.
