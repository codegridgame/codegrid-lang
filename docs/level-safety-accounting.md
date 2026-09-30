# Level Safety Accounting v1

This local implementation contract refines the trusted resource profile in the
[ExactIO contract](../spec/codegrid-level-exactio-contract-v1.md). None of these
counters modifies language metrics or claims to measure physical heap bytes.

## Retained representation and VM storage

`max_state_bytes` accounts original logical-level JSON bytes and source bytes
for retained handles. Each evaluation reserves both originating representations
again because it owns independent clones. Each evaluation also reserves an
encoded feedback/result allowance at start and returns it as
`configuration.safety.max_feedback_bytes`. Reservations are checked against the
same `max_state_bytes` total and released with the evaluation handle. Decoded/compiled
storage also has level/test data, program cell/board, and VM unit ceilings.
Physical allocator overhead, stack, and transient objects need host memory
controls; the portable artifact has a fixed maximum linear memory.

The evaluator checks `max_state_units` before dispatch and at committed tick
boundaries. One unit is
one immutable or mutable code cell, register, queued input/output byte, thread,
data/instruction stack entry, call frame, allocated memory cell, used-cell metric
identity, used-memory metric identity, dynamic instruction-kind identity, or
retained runtime error. Immutable code includes all Custom/Function/Folded code;
mutable outer code is counted independently. Unbounded Page/address magnitudes
add `ceil(bit_length / 8)` units for every retained instance; zero adds zero
magnitude bytes. These are logical units, not a serialized VM snapshot or a
heap estimate. No snapshots or metric identity sets are sent to level clients.

For fixed v1 result fields, the conservative encoded base reservation is 16,384
bytes plus six times the UTF-8 lengths of level and profile identities. This
bounds the fixed schema, maximum decimal widths, eleven metrics, eleven
constraints, eleven scoring entries, replay digests, and rejection/configuration
fields. It is schema accounting overhead, not a recommended deployment ceiling.
Each visible record adds 256 bytes plus six times its input, expected-output,
actual-output byte counts and runtime error-code string lengths (including four
bytes per code for array/string punctuation). Checked arithmetic rejects overflow.
The evaluator checks the next record's bound before cloning or retaining its
payload; exhausted feedback ends as ResourceLimitExceeded without final metrics
or rating. Terminal feedback is moved into the result, avoiding retained copies.

The API checks the result bound before constructing its JSON projection. Source
diagnostic projections have a separately checked bound of 256 base bytes plus
256 bytes per diagnostic and six times its code/message UTF-8 lengths. A bounded
logical-level error projection uses 512 bytes plus six times the field-path
UTF-8 length, avoiding unbounded author-controlled unknown-field diagnostics.
The bounded
serialization writer refuses growth beyond `max_response_bytes`. These bounds
may reject conservatively; no oversized JSON is built and then truncated. A
minimal complete API error remains available under the validated response
minimum. Core and compiler transient values remain subject to physical host
memory controls.

Generated-code diagnostics retain the committed scope, static cell, generated
Primary, and trusted source test index inside the opaque evaluation session.
Public projection includes only the permitted rejection reason; hidden context
and generated instruction payloads never enter progress or result responses.

Custom-local committed state is discarded at invocation completion. Its
transient growth is limited by the atomic work ceiling and embedding memory
controls. The evaluator cannot resume an interrupted Custom or retain its
partial execution. Core-only native callers must supply physical process limits
when evaluating hostile data; deterministic unit ceilings act at commit.

## Work and tick accounting

`Vm::step_with_work_accounting` returns the count actually dispatched in every
attempt, including interruptions and errors. The neutral addition does not
change VM transitions, rollback, event order, or normative metrics. The level
session charges that count to cumulative work. Per-call work is bounded, and
the per-test tick ceiling counts committed Global Ticks.

An interrupted outer tick rolls back and returns Pending. Retry may use a larger
budget up to the trusted per-call ceiling. If even that ceiling cannot execute
the tick, evaluation returns ResourceLimitExceeded. Repeated small requests
consume cumulative work and can end with the same resource outcome. Different
retry sequences may have different safety work totals although successful
semantic results match. Exact-ceiling completion needs no further work and can
pass; a ceiling with unfinished work cannot pass.

## Deployment policy

[local-v1.json](../fixtures/levels/profiles/local-v1.json) is a named repeatable
test configuration. Its values do not assert production suitability. Backends
select immutable trusted profiles and retain full logical levels; submissions
cannot replace the profile or supply official results. Client-shipped hidden
tests are not secrets. Cancellation, runtime traps, allocation failures, and
host deadlines cannot award ratings or certify player success.
