# CodeGrid Runtime API Specification

**Status:** Normative Full Runtime API v3 contract.
**Version:** Runtime API v3.
**Language semantics:** [Source specification](codegrid-source-spec.md) and [VM specification](codegrid-vm-spec.md)

## 1. Runtime API v3 and compatibility boundary

Runtime API v3 is the Full host contract. It supports the complete Full source
and verified-program surface defined by the [source specification](codegrid-source-spec.md),
and the execution state and outcomes defined by the [VM specification](codegrid-vm-spec.md).
It remains host-neutral and does not define browser JavaScript bindings, a
server byte ABI, CLI JSON, authentication, or game policy.

Every v3 request and success or error response carries `api_version: 3`.
Version checking happens before operation-specific validation. An endpoint
rejects any request whose version is not 3 with a structured
`UnsupportedVersion` error that reports both the received and supported
versions. It never silently upgrades a request or interprets it under another
API version. Source-language, executable-IR, CLI-result, Runtime-API,
browser-binding, and server-ABI versions are independent.

## 2. Common data and exact integer representation

The v3 contract uses the following host-neutral values:

- Source is UTF-8 text. Source limits count its UTF-8 byte length, not Unicode
  characters or editor columns. Compiler diagnostics retain UTF-8 byte spans.
- Input, output, register, memory, and instruction-code values are ordered
  bytes in `0..=255`. Input order is significant.
- `boundary_mode` is `exit` or `wrap`. `seed` is an unsigned 64-bit value.
  `custom_execution_limit` is a positive unsigned 64-bit number of internal
  Custom ticks per invocation, with its semantics defined by the VM
  specification.
- A memory address and a Page are signed arbitrary-precision integers. An
  initial-memory entry contains one address and one byte value. Initial
  memory initializes the outer execution context only; Custom invocations
  initialize their own memory according to the VM specification.
- Program and instance handles are nonzero unsigned 64-bit opaque values.
  Thread IDs, tick numbers, operation counters, stack high-water counters,
  and other VM counters retain their full declared integer width.

No browser or server boundary may round, truncate, saturate, or narrow a wide
integer. A JavaScript binding represents `u64` values and arbitrary-precision
signed integers with JavaScript `BigInt`. If a JSON transport is used, such
integers are canonical decimal strings, never JSON numbers. Byte values and
portable board coordinates/dimensions may use exact bounded numeric fields.
Server ABI field layout and the exact browser binding schema remain separate
contracts and are covered by the Full browser adapter contract and local host acceptance.

The initial-memory request is an ordered collection of `{address, value}`
entries. Addresses must be unique; duplicate addresses reject the request.
Zero-valued entries are valid and equivalent to absent cells. Snapshots expose
only the normalized nonzero sparse outer-memory entries, ordered by numeric
address. An adapter may use a different transport encoding only if it
preserves this exact mapping and validates it before instance creation.

## 3. Operations and request/response records

Every operation below includes `api_version: 3`. Every response includes the
same contract version. Rejections use a versioned structured error envelope;
they are not encoded as VM errors.

| Operation | Request fields | Success response |
| --- | --- | --- |
| `check` | `source` | Shared compiler diagnostics; it allocates no handle. |
| `compile` | `source` | Either `Compiled { program }` or `Diagnostics { items }`. A diagnostic result allocates no handle. |
| `program_view` | `program` | A read-only canonical `ProgramView` projection for inspection. |
| `create_instance` | `program`, `input`, `initial_memory`, and `configuration` | A new `instance` handle. |
| `step` | `instance` | One VM transition result, its ordered events and output delta, and the post-transition Full snapshot. |
| `run` | `instance`, positive `max_ticks` | `Halted`, `Error`, or `Yielded { reason }`, plus events from committed ticks in this call and the post-call Full snapshot. |
| `snapshot` | `instance` | A detached Full snapshot without advancing execution. |
| `release_program` | `program` | Acknowledgement that the program handle was released. |
| `release_instance` | `instance` | Acknowledgement that the instance handle was released. |

The transport-neutral record shapes are:

```text
CheckRequest       = { api_version: u32, source: Utf8Text }
CompileRequest     = { api_version: u32, source: Utf8Text }
ProgramViewRequest = { api_version: u32, program: ProgramHandle }
CreateInstanceRequest = {
  api_version: u32,
  program: ProgramHandle,
  input: Vec<u8>,
  initial_memory: Vec<{ address: BigInt, value: u8 }>,
  configuration: { boundary_mode: Exit | Wrap, seed: u64,
                   custom_execution_limit: positive u64 }
}
StepRequest        = { api_version: u32, instance: InstanceHandle }
RunRequest         = { api_version: u32, instance: InstanceHandle,
                       max_ticks: positive u64 }
SnapshotRequest    = { api_version: u32, instance: InstanceHandle }
ReleaseProgram     = { api_version: u32, program: ProgramHandle }
ReleaseInstance    = { api_version: u32, instance: InstanceHandle }
```

All success records use `SuccessEnvelope<T> = { api_version: 3, result: T }`.
Rejected calls use `ErrorEnvelope = { api_version: 3, error: ApiError }`;
`UnsupportedVersion` includes both `received` and `supported` versions.
`CompileOutcome` is `Compiled { program }` or `Diagnostics { items }`.
`StepResult` contains
`attempted_tick`, `committed_ticks`, `status`, `events`,
`newly_emitted_output`, `errors`, `fault`, and `snapshot`. `RunResult` contains
`status`, committed-tick `events`, `newly_emitted_output`, and `snapshot`.
`RunStatus` is `Halted`, `Error`, or `Yielded { reason }`. `FullSnapshot` has
the fields enumerated in Section 6. The exact serialized field spelling is
defined by the separately versioned browser and server transport contracts.

The canonical API error categories include `UnsupportedVersion`,
`SourceLimitExceeded`, `ProgramLimitReached`, `InstanceLimitReached`,
`InputLimitExceeded`, `InitialMemoryLimitExceeded`, `InvalidConfiguration`,
`DuplicateInitialMemoryAddress`, `UnknownProgramHandle`,
`UnknownInstanceHandle`, `ZeroTickBudget`, `RunTickBudgetExceeded`,
`InstanceTickBudgetExceeded`, `WorkUnitBudgetExceeded`, and
`HandleSpaceExhausted`. An adapter may add an ABI decoding error before it
calls Runtime API, but it must not relabel a compiler diagnostic or VM runtime
error as an ABI or API error.

`check` and `compile` accept Full source through the shared compiler. A
successful `compile` stores a verifier-produced `VerifiedProgram` and returns
only its opaque handle; `program_view` is read-only and cannot be submitted as
executable IR. If a future host operation accepts serialized IR, decoding must
feed the canonical IR verifier before any handle or instance is created. A
client assertion that data was compiled or verified is never authoritative.

`create_instance.configuration` contains exactly the execution settings
required from the host: `boundary_mode`, `seed`, and positive
`custom_execution_limit`. `input` is an ordered byte sequence. `initial_memory`
is the sparse outer-memory entry collection described in Section 2. No setting
is read from process state, environment variables, clocks, or host randomness.

Compiler rejection is a successful `compile` response with structured source
diagnostics, not an API-level failure. The diagnostics identify the source
token, directive, definition, or board using UTF-8 byte spans where available.

Each diagnostic includes the shared compiler stable `code`; `ApiError::code()` provides a stable host-neutral category. The [error code specification](codegrid-error-codes.md) lists preserved adapter aliases and additive diagnostic-field compatibility.
An API-level error covers malformed or over-limit host input, an unsupported
version, invalid execution settings, an unknown/released/foreign handle,
capacity exhaustion, or a deterministic work-limit result for `step`.

## 4. Program and instance handle lifecycle

Each `RuntimeApi` registry allocates handles locally, starting at a nonzero
value and never reusing a released value during that registry's lifetime.
Handles are references, not credentials, durable identifiers, or proof of
ownership. A handle copied from another registry, WASM module instance, or
process is invalid. Zero, malformed, released, and unknown handles are
rejected before use.

`compile` creates a program handle only after source compilation and
verification succeed and after the live-program and handle-space limits pass.
Invalid source returns diagnostics even if the program capacity is full.
`create_instance` requires a live verified program handle and creates an
isolated VM instance with fresh registers, threads, stacks, memory, code state,
metrics, and random streams as defined by the VM specification. Instances do
not share mutable VM state.

Releasing a program prevents new instances and program views through that
handle. Existing instances remain valid because they retain their verified
program. Releasing an instance discards its execution state. Later operations
on a released handle return the corresponding structured unknown-handle error.
Releasing one instance never releases another instance or its program.

The Rust registry is not implicitly synchronized. A host serializes mutating
operations on one instance and defines their accepted call order. A borrowed
snapshot or program view is valid only while its runtime borrow is alive; it
must not be retained as an owned handle or used to mutate/execute a program.
Detached snapshot responses are owned copies. Streaming projections use
borrowed views so serialization does not first clone retained VM state or
program IR.

## 5. Full execution results and call limits

`step` applies one outer Global Tick through the VM's single-tick transition.
Its response contains the attempted and committed tick numbers, status, raw
metrics, structured errors or VM fault, newly emitted output bytes, events in
VM order, and the complete post-transition snapshot. A failed VM tick reports
VM state (`Error`) and its VM diagnostics; it is not an API request rejection.
Terminal `step` calls are no-op transitions and return the unchanged terminal
snapshot. A running instance cannot start a tick after its cumulative committed
tick budget is exhausted.

`run` requires `max_ticks > 0`; the value must not exceed either the host's
per-call tick ceiling or the instance's remaining cumulative committed-tick
budget. The VM repeatedly applies the same transition as `step`. The result is
`Halted`, `Error`, or `Yielded` with one of these reasons:

- `TickSliceExhausted`: the requested bounded slice ended and the VM remains
  running.
- `WorkUnitBudgetExhausted`: the deterministic host work ceiling interrupted
  an attempted tick. The VM returns the post-interruption snapshot. Earlier
  completed ticks in this `run` remain committed; the interrupted tick is
  rejected atomically as defined by the VM specification.

The `run` response includes events and output produced by committed ticks in
that call, in tick order, and a complete post-call snapshot. A work-limited
`step` returns the distinct `WorkUnitBudgetExceeded` API error rather than a
VM error or a partial step response. Its interrupted tick has no committed
state or normative VM metric changes under the VM work-limit rule.

The host supplies immutable per-runtime ceilings for source bytes, live
compiled programs, live instances, input bytes, initial-memory entries, ticks
per `run` call, cumulative committed Global Ticks per instance, and
deterministic VM work units per API call. The per-call and per-instance tick
ceilings and the work-unit ceiling are positive. A zero count/byte ceiling
disables the corresponding capability or admits only an empty payload. These
ceilings are policy values supplied by the embedding host; this specification
does not select numerical defaults.

The configured `custom_execution_limit` is distinct from API tick and work
ceilings. It limits internal Custom ticks per invocation according to the VM
specification. A host may impose a separate maximum accepted configuration,
but v3 defines no universal quota value. `CustomExecutionLimitExceeded` is a
VM runtime error; it must not be converted into a host work-limit yield.

Deterministic work units are defined in [VM Spec Section 15](codegrid-vm-spec.md#15-deterministic-work-accounting), not by wall-clock time or `Operation Count`. Work limits bound thread dispatches only. They do not imply a bound on cumulative memory, stacks, output, event history, retained program state, or serialized snapshot size.

## 6. Full snapshots, metrics, and error separation

A v3 Full snapshot carries the full public snapshot defined by the VM
specification:

- overall VM status and committed Global Tick count;
- ten outer registers, normalized sparse outer memory, remaining input,
  accumulated output, and the mutable outer program copy;
- every outer thread's ID, CodeGrid and Board, coordinate and direction,
  register pointer, arbitrary-precision Page, data and instruction stacks,
  call frames, execution phase, and PRNG state;
- cumulative raw metrics, structured VM errors, and an optional VM fault.

Normal program-view cells contain nullable canonical token fields `prefix`,
`entry`, `primary`, and `attachment`. `prefix` is `?0`, `?1`, `?2`, or null;
it survives mutable Primary replacement and clearing. Folded Block arrays
contain nullable token strings, including any prefix before the Primary.
These views are read-only projections and cannot be supplied as executable IR.

Custom invocation state is synchronous and temporary within a caller's
transition; the snapshot does not invent persistent Custom instance handles.
Custom effects, failures, events, and memory accesses are represented according
to the VM contract. Raw metrics preserve all VM-defined identities and counts:
Global Tick, Operation Count, Used Cells, Used Memory Addresses, stack
high-water marks, and Instruction Variety. Ordered identity sets are serialized
in a stable order. Metrics must not be omitted, rounded, silently truncated,
or replaced with host-specific summaries.

VM runtime errors and faults are part of VM state and step/run results. API
errors describe rejected host operations and do not become VM errors. Compiler
diagnostics are source-validation results and do not allocate a program. A
yielded `run` is nonterminal; neither tick-slice exhaustion nor work-limit
exhaustion changes a running VM into a halted VM.

## 7. Browser and server adapter responsibilities

Adapters depend on Runtime API v3 and translate only transport/ABI values,
request and handle checks, host-selected resource ceilings, and instance
lifecycle. They do not parse source independently, duplicate source
acceptance or instruction tables, lower IR, or implement VM transitions.

The browser adapter reports the Runtime API version it implements, validates
all incoming values before calling the API, preserves wide integers exactly,
and manages handles within one module/runtime instance. It may expose borrowed
views through a streaming projection only while the underlying Rust borrow is
valid. Its local Full binding surface is documented in
[`codegrid-wasm-browser`](../crates/codegrid-wasm-browser/README.md).
JavaScript `Number` is never used for seed, handle, tick, thread ID,
counter, Page, or memory-address values that can exceed its exact integer
range.

The server adapter reports its own ABI version separately from Runtime API
v3. If it accepts source, it calls the shared compiler through Runtime API. If
it accepts serialized IR in a separately specified ABI, it decodes and
re-verifies that IR before execution. It must not trust a browser's program
view, claimed validation result, or client-generated execution result.

Both adapters preserve complete structured errors and responses. They do not
silently truncate source, byte input, diagnostics, events, snapshots, or metrics.
They must enforce explicit serialized request/response ceilings and a memory
policy before executing untrusted inputs; response-size failures must not be
returned as malformed or partial JSON. The exact transport and memory policies
are covered by the Full browser and server adapter contracts and local host acceptance.

The native CLI intentionally composes compiler and VM APIs directly and is
not required to route through Runtime API. CLI JSON, Runtime API v3, browser
binding, and server ABI schemas remain separate contracts.

## 8. Local host requirements

Runtime API v3 defines no authentication field or credential. Handles are not
authorization tokens. Authentication and authorization for a network-facing
or multi-user deployment remain an external product/security decision; they
are outside the local language/runtime contract and must be settled before
public remote exposure.

The following values and details are intentionally not guessed here:

- universal or production quota defaults. Local Browser and Server limits
  are selected in their own adapter contracts;
- production authentication/authorization policy and per-user quota model;
- host wall-clock deadlines, cancellation, process/isolate memory, and
  concurrency controls;
- distribution, signing, release, and compatibility policy for published
  artifacts, which is outside the local Full development scope.

These are deployment or packaging choices, not language semantics. Local
adapter limits are specified in the [Browser adapter contract](../crates/codegrid-wasm-browser/README.md)
and [Server adapter contract](../crates/codegrid-wasm-server/README.md). The
deterministic per-call work limit alone does not protect cumulative VM state
or response allocation. A Full parity claim requires executing the same
conformance cases in actual native, browser-WASM, and server-WASM hosts and
comparing status, threads, all public state, output, errors, events, and raw
metrics. Compilation or `wasm32` compilation alone is not parity evidence.

The Full local development scope and acceptance work are tracked in the
[Full task book](../tasks/spec-completion.md). This specification assigns
the host-neutral v3 contract; it does not approve production deployment or
settle the separate host gates listed above.

## Conditional prefix and CMP migration (2026-10-03)

The approved source/VM migration uses executable IR format 2, RandomDirection `??` (126), CMP `?=` (124), and fixed conditional prefixes `?0`–`?2`. Normal code-view cells expose an additive nullable `prefix` field containing the canonical prefix spelling. This field survives Primary mutation and clearing. Folded Block views keep their existing arrays of nullable strings; nonempty strings include any prefix followed by the Primary token. These are read-only projections, not executable interchange data.

The Runtime API v3, browser binding envelope, server ABI v4, CLI JSON schema 1, and debug protocol 2 remain unchanged; existing lifecycle and transport fields are preserved. Language source acceptance, IR version, capability vocabulary, and new code-view fields follow the recorded migration decision. Removed source forms and obsolete instruction bytes are not compatibility aliases. Consumers displaying cells should include the prefix.
