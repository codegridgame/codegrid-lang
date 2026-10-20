# Full Runtime API v6

Runtime API v6 is the host-neutral lifecycle and snapshot contract for compiling
source and running isolated VM instances. It contains language-runtime state
and generic host protections. Game rules, game counters, resource budgets, scoring, selection,
and game-specific execution quotas belong to game hosts.

The API version is `6`. Browser and server adapters must reject other API
versions. The server byte ABI has its own version, currently `7`.

## Host limits and configuration

`HostLimits` contains these trusted host ceilings:

- `max_source_bytes`, `max_compiled_programs`, `max_instances`,
  `max_input_bytes`, and `max_initial_memory_entries`;
- `max_run_steps_per_call` and `max_total_steps_per_instance`;
- `max_work_units_per_call`.

Count and byte limits may be zero to disable the corresponding capacity. Step
and work-unit limits must be positive. A work-unit ceiling bounds a single VM
transition, including nested Custom dispatch; if the ceiling interrupts a
transition, that transition is rolled back while earlier committed transitions
remain committed. Hosts must separately bound response and retained-state
serialization where needed.

`RuntimeConfiguration` contains only `seed`. Instance requests also carry a
verified program handle, input bytes, and optional sparse initial memory. A
memory entry has a signed arbitrary-precision decimal address and one byte
value; duplicate addresses are rejected and zero values are normalized away.

## Operations

The Runtime API supports check, compile, program view, create instance, one-step
execution, bounded run, snapshot, and explicit release of program and instance
handles. Handles are local to one `RuntimeApi` and are never reused during its
lifetime. The API has no game-level thread selection or reposition operation.

`RunRequest.max_steps` is a positive per-call transition ceiling. It is also
bounded by `max_run_steps_per_call` and remaining
`max_total_steps_per_instance`. `StepResponse` and snapshots report
`committed_steps`; a step result also reports `attempted_transition_id`.

## JSON representation

Wide integers, handles, source byte offsets, memory addresses, pages, random
states, transition IDs, steps, and counters are canonical decimal strings.
Byte values, coordinates, dimensions, registers, and small identifiers are JSON
numbers.

A Run response has `api_version`, a tagged `status`, `events`,
`newly_emitted_output`, and a full `snapshot`. Its statuses are:

- `{ "kind": "halted" }`
- `{ "kind": "error" }`
- `{ "kind": "yielded", "reason": "step_slice_exhausted" }`
- `{ "kind": "yielded", "reason": "work_unit_budget_exhausted" }`

The snapshot contains VM `status`, `committed_steps`, registers, sparse memory,
remaining input, accumulated output, mutable runtime program, threads, the
committed-step summary, structured runtime errors, and an optional fault. Step
results include the same snapshot plus `attempted_transition_id`,
`committed_steps`, ordered events, output delta, errors, and optional fault.

The only persistent execution counter in Runtime API snapshots is
`committed_steps`. Candidate-local `StepFootprint` observations and dispatch
counts (`work_units` and `custom_internal_dispatches`) are available only
through the Rust VM prepared-step API; they are not accumulated or projected
through Runtime API, CLI, browser, or server JSON. No game resource or counter
metrics are part of this API.

Thread snapshots distinguish `fold`, `fold_repeat`, and `fold_resume` phases.
All three phases contain `outer_repeat`, which is `null` when the Folded Block
call has no Repeat suffix, or `{ "total": number, "completed": number }` while
an outer Repeat is in progress. `total` is 2 through 9 and is the requested
number of whole-block invocations; `completed` is the number of successful
invocations completed so far, from 0 through `total`. `fold_repeat` also
contains `fold_id`, `internal_position`,
`internal_direction`, `saved_outer_direction`, `total`, and `completed` for a
Repeat attached to the current internal cell. The thread stays at that internal
cell until the inner Repeat completes or its conditional prefix cancels it.
For an outer Repeat, each successful whole-block invocation increments
`outer_repeat.completed`; the call-site prefix is rechecked before each later
invocation.
Debug and source-location consumers must resolve both `fold` and `fold_repeat`
against the Folded Block's internal cell, rather than its outer call site.
Mutable `runtime_program.folded_blocks` tokens retain each cell's prefix and
suffix when WriteCode changes or clears its Primary.

Runtime error objects contain `code`, `error_number`, `transition_id`,
`scope`, and structured `details`. Custom scope reports `internal_step`.
`FoldedBlockWithoutDefinition` details include `thread_id`, `board`, `position`,
and `fold_id` when a dynamically installed Folded Block call has no local
definition.
Host/API errors remain separate from VM errors. Relevant execution-budget API
error codes are `zero_step_budget`, `run_step_budget_exceeded`,
`instance_step_budget_exceeded`, and `work_unit_budget_exceeded`.

## Host adapters

The browser adapter exposes Runtime API v6 through JavaScript and applies
request, response, retained-state, step, and work-unit ceilings. It uses decimal
strings for wide integer inputs and outputs.

The server adapter exposes Runtime API v6 over the no-import byte ABI v7. Its
JSON requests include both `abi_version: 7` and `api_version: 6`; its responses
include both versions. Buffer ownership and response bounds are server-host
transport concerns and do not change VM semantics.
