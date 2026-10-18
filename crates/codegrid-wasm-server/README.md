# CodeGrid Server WebAssembly Adapter

This crate exposes [Runtime API v4](../../docs/runtime-api-spec.md) through the
no-import Server ABI v5. It owns byte-buffer transport, JSON conversion, request
validation, trusted host ceilings, and the Runtime API session. It does not
implement language execution or game policy.

The target is `wasm32-unknown-unknown` and imports no host functions. Each
module instance owns one runtime session. A host requiring separate isolation
scopes creates separate module instances and does not share one session among
untrusted tenants.

## Exported no-import ABI

- `codegrid_server_abi_version() -> u32` returns `5`.
- `codegrid_server_alloc_buffer(length: u32) -> u32` allocates a zero-filled
  request or response buffer. Zero signals a rejected length or allocation
  failure.
- `codegrid_server_process_request(pointer: u32, length: u32) -> u64` handles
  one request and returns `(response_pointer << 32) | response_length`. Zero
  means the range is invalid or a complete response buffer could not be
  reserved; a reservation failure does not dispatch the request.
- `codegrid_server_free_buffer(pointer: u32) -> u32` releases a buffer and
  returns `1` on success or `0` for an unknown pointer.

The host writes UTF-8 JSON into a request buffer, calls the processor, copies
the response, then frees both buffers. Pointers are transient offsets into one
module instance's linear memory. Requests and responses are limited to 8 MiB;
the module retains at most 16 MiB across at most 1024 buffers. Before
initialization and after shutdown, the processor reserves the fixed 256-byte
control response. After initialization, it reserves the complete configured
response ceiling before dispatch.

## Versioned JSON contract

Each request carries `abi_version: 5`, `api_version: 4`, and an `operation`.
Each response includes both version fields. The versions describe the server
transport and Runtime API independently.

`initialize` takes immutable trusted ceilings in `host_limits`:

```json
{
  "abi_version": 5,
  "api_version": 4,
  "operation": "initialize",
  "host_limits": {
    "max_source_bytes": 4194304,
    "max_compiled_programs": 256,
    "max_instances": 64,
    "max_input_bytes": 1048576,
    "max_initial_memory_entries": 65536,
    "max_run_steps_per_call": "1000000",
    "max_total_steps_per_instance": "10000000",
    "max_work_units_per_call": "1000000",
    "max_response_bytes": 8388608,
    "max_instance_state_bytes": 8388608
  }
}
```

Count and byte limits may be lowered but not raised above adapter ceilings.
Per-call steps, per-instance steps, and per-call work units must be positive.
Response and snapshot limits must each be at least 256 bytes, and the snapshot
limit cannot exceed the response limit. Runtime count and byte ceilings may be
zero. All ceilings come from trusted host configuration rather than a client
request.

Supported operations are `check`, `compile`, `program_view`,
`create_instance`, `step`, `run`, `snapshot`, `release_program`,
`release_instance`, and `shutdown`:

- `check` and `compile` take UTF-8 `source`. Compile returns diagnostics or a
  decimal-string program handle with the canonical program view.
- `create_instance` takes `program`, an `input` byte array, decimal-string
  `seed`, and optional `initial_memory`. Each memory entry is
  `{ "address": "<canonical signed decimal>", "value": <0..255> }`.
  Missing memory means an empty array. Duplicate addresses are rejected;
  zero-valued entries are accepted and normalized away.
- `step`, `run`, and `snapshot` take a decimal-string `instance` handle. `run`
  also takes a positive decimal-string `max_steps`.
- Release operations take the corresponding decimal-string handle.
  Instances retain their verified program after its program handle is
  released. `shutdown` closes the session permanently.

Bytes, coordinates, dimensions, registers, and small identifiers are JSON
numbers. Every wide integer, handle, source byte offset, Page/address,
thread/transition ID, step counter, counter, and PRNG state uses a canonical
decimal string.

Run status strings are `halted`, `error`, `step_limit_reached`, and
`work_limit_reached`. The last two represent step-slice and deterministic
work-unit yields. Both preserve the full post-call snapshot. A work-unit limit
keeps earlier committed transitions and rolls back the interrupted
transition.

## Full projections

Snapshots contain VM `status`, `committed_steps`, registers, normalized sparse
memory, remaining input, accumulated output, mutable runtime program, threads,
a committed-step summary, structured errors, and an optional fault. Each thread includes its
code grid, ID, board, position, direction, register pointer, status flag, page,
stacks, call frames, phase, and random state. Compile and program-view
responses include verified Outer and Custom code-grid views.

`committed_steps` is the only execution counter in the snapshot projection.
Candidate-local `StepFootprint` observations are available only through the
Rust prepared-step API and are not serialized by the server ABI. Runtime errors
contain a stable `code`, `error_number`, `transition_id`, `scope`, and
structured details. Custom scope reports `internal_step`. API request errors,
compiler diagnostics, VM errors, VM faults, and bounded-run yields remain
distinct. No game resource accounting or game counter metrics appear in this contract.

The response serializer streams borrowed views into a bounded counter/writer
instead of constructing a large `serde_json::Value` tree. If a retained
snapshot exceeds `max_instance_state_bytes`, the adapter releases that
instance and returns a structured error. If only an event or result exceeds
the response ceiling, it returns a response-limit error and retains the
runtime state for a later bounded operation.

These controls do not bound compiler allocations, transactional staging,
serializer formatting, process memory, or wall-clock duration. Those remain
responsibilities of the embedding server.

## Local validation

Native tests exercise ABI v5 request validation, lifecycle, exact integer and
memory conversion, and shared Full fixtures. `server-smoke.mjs` exercises the
no-import exports, buffers, configured memory maximum, ABI v5 operations,
Full projection fields, and host ceilings in Node's WebAssembly host. These
checks do not establish production-host compatibility or peak-allocation
safety.

```sh
CODEGRID_WASM_MAX_MEMORY_BYTES=67108864 cargo build --release --target wasm32-unknown-unknown -p codegrid-wasm-server
node crates/codegrid-wasm-server/tests/server-smoke.mjs
```
