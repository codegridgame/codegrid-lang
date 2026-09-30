# CodeGrid Server WebAssembly Adapter

This crate implements Server ABI v4 over Runtime API v3. The host-neutral
Full contract is defined by the [Runtime API specification](../../spec/codegrid-runtime-api-spec.md)
and the [VM specification](../../spec/codegrid-vm-spec.md). This adapter owns
only byte-buffer transport, JSON conversion, request checks, trusted resource
ceilings, and the Runtime API session. Source is compiled by the shared
compiler; client-provided IR is never accepted.

The target is `wasm32-unknown-unknown` and imports no host functions. Each
module instance owns one Runtime API session. A host requiring separate
isolation scopes creates separate module instances and must not share one
session among untrusted tenants.

## Exported no-import ABI

- `codegrid_server_abi_version() -> u32` returns `4`.
- `codegrid_server_alloc_buffer(length: u32) -> u32` allocates a zero-filled
  request or response buffer. Zero indicates a rejected length or allocation
  failure.
- `codegrid_server_process_request(pointer: u32, length: u32) -> u64` processes
  an allocated request and returns `(response_pointer << 32) | response_length`.
  Zero indicates an invalid range or inability to reserve the whole response;
  a reservation failure does not dispatch the request.
- `codegrid_server_free_buffer(pointer: u32) -> u32` releases an allocated
  buffer and returns `1` on success or `0` for an unknown pointer.

The host writes UTF-8 JSON into a request buffer, calls the processor, copies
the response bytes, and frees both buffers. Pointers are transient offsets into
one module instance's linear memory. Requests and responses are limited to
8 MiB; the module retains at most 16 MiB across at most 1024 buffers. The host
must free buffers promptly. Before initialization and after shutdown, the
processor reserves the fixed 256-byte control-response capacity. After
initialization, it reserves the configured complete response ceiling before
dispatch.

## Versioned JSON contract

Every request supplies `abi_version: 4`, `api_version: 3`, and an `operation`.
The two version fields are independent. Version 3 Runtime API semantics do
not make the incompatible older Server ABI request layout valid. Every
response includes both version fields and the operation where one is known.

`initialize` takes immutable host-selected ceilings in `host_limits`:

```json
{
  "abi_version": 4,
  "api_version": 3,
  "operation": "initialize",
  "host_limits": {
    "max_source_bytes": 4194304,
    "max_compiled_programs": 256,
    "max_instances": 64,
    "max_input_bytes": 1048576,
    "max_initial_memory_entries": 65536,
    "max_run_ticks_per_call": "1000000",
    "max_total_ticks_per_instance": "10000000",
    "max_work_units_per_call": "1000000",
    "max_response_bytes": 8388608,
    "max_instance_state_bytes": 8388608
  }
}
```

These values may be lowered but not raised above the adapter ceilings shown.
The response and snapshot limits must each be at least 256 bytes, and the
snapshot limit cannot exceed the response limit. Runtime count and byte
ceilings may be zero; per-call ticks, cumulative ticks, and work units must be
positive. All configuration comes from the trusted embedding server, not the
requesting client.

Supported operations are `check`, `compile`, `program_view`, `create_instance`,
`step`, `run`, `snapshot`, `release_program`, `release_instance`, and
`shutdown`:

- `check` and `compile` take a UTF-8 `source` string. `compile` returns either
  structured diagnostics or a decimal-string program handle with its canonical
  `ProgramView`. `program_view` takes a `program` handle and returns that view
  without recompiling.
- `create_instance` takes `program`, an `input` byte array, `boundary_mode`
  (`"exit"` or `"wrap"`), decimal-string `seed`, positive decimal-string
  `custom_execution_limit`, and optional `initial_memory`. Each initial-memory
  item is `{ "address": "<canonical signed decimal>", "value": <0..255> }`.
  Missing initial memory means an empty array. Duplicate addresses are rejected;
  zero-valued entries are accepted and normalized away by the Runtime API.
- `step`, `run`, and `snapshot` take a decimal-string `instance` handle. `run`
  also takes a positive decimal-string `max_ticks`.
- Release operations take the corresponding decimal-string handle. A program
  may be released after instance creation; each instance retains its verified
  program. `shutdown` closes the session permanently.

The response schema uses bounded JSON numbers for bytes, coordinates, board
dimensions, registers, and small identifiers. Every `u64`, handle, tick,
counter, PRNG state, arbitrary-precision signed Page/address, and UTF-8 source
byte offset is encoded as a canonical decimal string. No wide value is carried
through a floating-point JSON number.

`run` response status strings are `halted`, `error`, `tick_limit_reached`, and
`work_limit_reached`; the latter two represent Runtime API v3 yielded results
for tick-slice and deterministic work-unit ceilings respectively. Both yield
statuses preserve the complete post-call snapshot. A work-limited call keeps
earlier committed ticks and discards the interrupted tick as defined by the
VM contract.

## Full projections

The `snapshot` object returned by `snapshot`, `step`, and `run` contains:

- VM `status`, `committed_ticks`, ten `registers`, normalized sparse `memory`,
  `remaining_input`, accumulated `output`, and mutable `runtime_program`;
- all Outer `threads`, each with `code_grid`, `id`, `board`, `position`,
  `direction`, `register_pointer`, decimal-string `page`, `data_stack`,
  `instruction_stack`, `call_frames`, `phase`, and decimal-string
  `random_state`. Instruction-stack entries are their normative Instruction
  Code bytes; EMPTY is code 32;
- cumulative raw `metrics`, structured `errors`, and optional `fault`.

Runtime `memory` is an address-sorted array of nonzero `{address, value}`
entries. `runtime_program` is the current mutable Outer CodeGridView, including
its Main and Function boards and Folded Block contents. The canonical
compile/program-view result contains the verified Outer and Custom CodeGrid
views.

Full raw `metrics` contain decimal-string `global_tick`, `operation_count`,
`used_cell_count`, `used_memory_address_count`, and the three stack high-water
values, plus ordered `used_cells`, `used_memory_addresses`, and
`instruction_variety`. Used-cell identities include CodeGrid, Board, optional
Folded Block, and coordinate. Memory identities include an Outer or
Custom-invocation space and the exact address. Identity collections follow
the VM's canonical order.

`step.result` contains attempted and committed ticks, VM status, per-step metric
summary, ordered committed `events`, newly emitted output bytes, errors, fault,
and its full post-transition snapshot. `run` contains committed events and
output from that call plus the complete post-call snapshot. Events use a `kind`
tag (`cell_reached`, `input_consumed`, `register_changed`, `memory_changed`,
`code_changed`, or `thread_changed`), include an Outer/Custom `scope`, and
preserve the Runtime API event order. Runtime errors preserve their stable
code, decimal-string global tick, scope, and full resource/participant details.
API request errors, compiler diagnostics, VM errors, VM faults, and bounded-run
yields remain distinct.

Adapter errors use `{ "error": { "code", "message", "details" } }` with
stable machine-readable codes. An oversized response is never returned as
partial JSON. Compiled handles that cannot be returned due to response limits
are released. If a retained Full snapshot exceeds `max_instance_state_bytes`
after creation, step, run, or snapshot projection, the instance is released and
a structured state-limit error is returned. Event or result output that exceeds
only the response ceiling returns a response-limit error; the committed runtime
state remains available for a later snapshot or bounded call.

Serialization streams borrowed snapshot, event, and compiler view fields into
a bounded counter/writer rather than building a large `serde_json::Value` tree.
The final bounded byte vector coexists briefly with its reserved ABI response
buffer. These controls do not bound compiler allocations, VM transactional
staging, serializer formatting, total process memory, or wall-clock duration.

## Local validation

Native tests exercise ABI v4 request validation, lifecycle, wide integer and
initial-memory conversion, and all 63 shared Full conformance cases. The
`server-smoke.mjs` runner additionally exercises the no-import Wasm exports,
buffer lifecycle, configured memory maximum, ABI v4 operations, Full fixture
fields, and resource ceilings in Node's WebAssembly host. These local tests do
not establish production-host compatibility or peak-allocation safety.

Build and smoke locally:

```sh
CODEGRID_WASM_MAX_MEMORY_BYTES=67108864 cargo build --release --target wasm32-unknown-unknown -p codegrid-wasm-server
node crates/codegrid-wasm-server/tests/server-smoke.mjs
```

The standalone [Wasmtime host harness](../../tools/wasmtime-host/README.md)
still expects the superseded ABI v3/API v2 contract and has not yet been
migrated to this Full ABI v4. It is not a v4 conformance check. Host fuel,
wall-clock deadlines, process memory, concurrency, and cancellation remain the
embedding server's responsibility.
