# Level WASM Transports v1

Both adapters invoke [Level Host API v1](level-api-v1.md). They contain conversion,
buffer ownership, and lifecycle glue only. Level policies and output projections
live in the shared Rust API.

## Browser binding v1

The browser artifact exports a session object initialized with trusted profile
JSON. `request(json)` accepts a Level API v1 request as UTF-8 JSON text and
returns complete response JSON text. `shutdown()` releases retained state.
The JavaScript caller uses strings for wide values and passes original logical
JSON text unchanged. A worker owns one session and schedules bounded advance
requests; the main thread presents permitted responses. No hidden trace or VM
snapshot operation is exported. Binding identity is independent from API v1.

## Portable byte ABI v1

The portable artifact imports nothing, including WASI. It exports linear
`memory`, `level_abi_version()`, `level_alloc(length)`,
`level_dealloc(pointer, length)`, and `level_request(pointer, length)`.
All pointers/lengths use wasm32 unsigned integers. The request function returns
a packed u64 with response pointer in the high 32 bits and length in the low
32 bits. The embedding runtime must preserve this u64 exactly.

Allocation registers a buffer; only its exact live pointer/length pair is valid.
Request bytes must occupy a registered buffer and be valid UTF-8. Input and
response buffers have separate ownership. The host deallocates each once, with
its exact returned length; stale/forged buffers cannot be dereferenced. Failed
allocation returns zero. The adapter bounds individual bytes, aggregate retained
buffer bytes, and buffer count. It checks bounds before any memory access.
Requests initialize one module-local session with a trusted profile, then invoke
shared API operations. Shutdown releases semantic session state; the host still
releases live transport buffers. A fresh module instance is the isolation unit.

The exact transport envelopes are:

```json
{"abi_version":1,"api_version":1,"operation":"initialize","profile_json":"<original trusted profile JSON>"}
{"abi_version":1,"api_version":1,"operation":"request","request_json":"<original Level API request JSON>"}
{"abi_version":1,"api_version":1,"operation":"shutdown"}
```

Initialization is permitted once. Shutdown closes the module session permanently.
Transport objects reject unknown and duplicate fields. Keeping profile and API
requests as original text preserves duplicate detection and exact integer data.
Responses add `abi_version: 1` to the complete shared API envelope. Before
initialization, request operations return `level_abi.not_initialized`; repeated
initialization returns `level_abi.already_initialized`.

Individual transport buffers are capped at 8 MiB, retained allocation capacity
at 24 MiB, and live buffer count at 1024. A response reserves 8 MiB before
dispatch; ownership thereafter uses the actual returned response length, while
retained accounting includes full allocation capacity. Response buffers cannot
be submitted as requests. `level_dealloc` returns 1 for an exact live pair and
0 for stale or mismatched pairs. `level_request` returns 0 for an invalid pair
or inability to reserve its response; no API operation is dispatched in either
case. Invalid UTF-8 in a live input returns a complete invalid-request response.
Released pointers can be reused by the allocator; callers must not retain stale
pointers after release. An exact pair identifies a currently live allocation,
not a durable identifier across allocations.

Browser inputs are JavaScript strings checked against the same 8 MiB UTF-8
ceiling before Rust conversion. The `LevelSession` constructor accepts trusted
profile text; `request` returns JSON text and `shutdown` closes its shared API.
Both wasm32 builds require `CODEGRID_WASM_MAX_MEMORY_BYTES`, an explicit positive
65536-byte multiple no greater than 4294967296, encoded as the module's maximum
linear memory. This maximum does not bound JavaScript heap or process memory.

ABI requests/responses include `abi_version: 1` and `api_version: 1`. Version
errors are complete bounded responses. A trap is an infrastructure failure and
must never be interpreted as a terminal player result. Hosts enforce runtime
memory/fuel/stack ceilings in addition to deterministic evaluator ceilings.

## Integration and verification

Web uses an actual browser worker. Backend and desktop use an actual selected
portable WASM runtime, or a compatible browser host for desktop UI. Steam SDK,
authentication, saves, leaderboard publication, clocks, and deployment controls
remain host responsibilities. This repository cannot certify an unavailable
game/backend embedding. Record the chosen runtime and integration route, artifact
digest, effective profile, and fixture projection in comparison reports.

Compare completed semantic responses against direct Rust and native CLI with
the same source, trusted level, profile, boundary, and seeds. Different Pending
sequences are allowed; different cumulative retry costs must be reported. Build
success alone is not parity evidence. Malformed requests, exact wide integers,
handle isolation, cancellation, release, response ceilings, and buffer lifecycle
require actual transport tests.

The Windows repeat command is:

```powershell
./scripts/test-level-wasm.ps1 -MaximumMemoryBytes 67108864
```

It builds the independent level artifacts with the explicit memory ceiling,
generates web bindings using `wasm-bindgen --target web`, runs native adapter
tests, executes the portable module with Node WebAssembly, executes the browser
binding in a real Chromium module worker, runs the portable Wasmtime 49.0.1
harness, and compares complete terminal results
from the shared fixture manifest against actual native CLI subprocesses.
Set `CODEGRID_BROWSER` to select Chrome or Edge. Cargo, the wasm32 target,
wasm-bindgen CLI, Node, and a Chromium browser must already be available. The
standalone Wasmtime tool builds its pinned Rust runtime dependency on first use.

The repeat command also runs shared negative transport probes in Node and the
browser worker, plus Wasmtime source/level/scene/error/response and fuel-trap
probes. Portable Node probes count and aggregate buffer ceilings and verify that
failed response reservation occurs before semantic dispatch. See
[build provenance](level-build-provenance.md) for pinned inputs and the recorded
source/artifact/toolchain identity.

Reports under `target/level-portable-report.json`,
`target/level-browser-report.json`, and `target/level-cli-report.json` retain
complete per-case semantic results. WASM reports include artifact SHA-256,
runtime identity, and the effective trusted profile. The comparator also reads
`target/level-wasmtime-report.json` when present; regenerate every report against
the same current manifest and build before comparing. An unavailable production
backend or Steam embedding remains an explicit integration dependency.
