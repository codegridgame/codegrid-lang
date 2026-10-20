# CodeGrid Browser WebAssembly Adapter

This crate exposes the shared compiler and VM through a browser and Node
WebAssembly binding. The host-neutral contract is [Runtime API v5](../../docs/runtime-api-spec.md).
The adapter converts JavaScript values, applies browser resource limits, and
manages runtime handles; it does not define source syntax or execution behavior.

## JavaScript API

Create one `BrowserRuntime` for each host isolation boundary:

~~~javascript
const runtime = new BrowserRuntime(
  1_048_576, // max source UTF-8 bytes
  128,       // compiled program handles
  128,       // VM instances
  65_536,    // input bytes
  65_536,    // initial memory entries
  1_048_576, // request bytes
  1_048_576, // response bytes
  1_048_576, // retained instance snapshot bytes
  "10000",   // maximum steps per call
  "100000",  // maximum steps per instance
  "1000000", // deterministic work units per call
);
~~~

The first eight limits are nonnegative 32-bit integers, except response and
instance snapshot limits which must be at least 256 bytes. Step and work-unit
ceilings are positive canonical unsigned decimal strings; keep them as strings
instead of converting them to JavaScript `Number`.

The binding exposes `api_version`, `check`, `compile`, `program_view`,
`create_instance`, `step`, `run`, `snapshot`, `release_instance`, and
`release_program`. Every response uses `api_version: 5`. No game-level Gas,
game counter policy, selection, or host repositioning operation is exposed.

`compile(source)` returns a runtime-local program handle. Create an instance
with its handle, input `Uint8Array`, initial-memory JSON, and seed-only runtime
configuration:

~~~javascript
const created = JSON.parse(runtime.create_instance(
  compiled.outcome.program,
  new Uint8Array([65, 0, 255]),
  JSON.stringify([{ address: "-1", value: 12 }]),
  JSON.stringify({ seed: "18446744073709551615" }),
));
~~~

`run(instance, maxSteps)` takes a positive decimal-string transition ceiling.
Its response contains committed events and output deltas plus a full snapshot.
`step(instance)` applies one VM transition; `snapshot(instance)` reads state
without execution. Run results distinguish `step_slice_exhausted` from
`work_unit_budget_exhausted`. A work-unit interruption rolls back the current
transition while preserving earlier committed transitions. Release program
and instance handles when finished; handles belong to one `BrowserRuntime`.

Handles, source byte offsets, page values, memory addresses, thread IDs,
transition IDs, steps, random states, and counters use canonical decimal
strings. Byte values, registers, slots, dimensions, and coordinates use JSON
numbers. The adapter checks source UTF-8 and `Uint8Array` byte lengths before
copying them into WebAssembly memory. Initial memory is limited by entry count
and request bytes; addresses must be canonical signed decimal strings and
values must be bytes.

Responses and retained instance snapshots have separate byte limits. A
snapshot exceeding its configured limit causes that instance to be released
and a structured error returned. Work-unit limits count VM dispatch work and
are independent of browser fuel. JSON byte limits do not cap temporary
allocations, JavaScript heap use, total WebAssembly linear memory, wall-clock
time, or browser-process memory.

## WebAssembly memory ceiling

Every build targeting `wasm32-unknown-unknown` requires
`CODEGRID_WASM_MAX_MEMORY_BYTES`. Set it to a positive multiple of 65,536 no
greater than 4,294,967,296. For a 64 MiB module ceiling:

~~~powershell
$env:CODEGRID_WASM_MAX_MEMORY_BYTES = "67108864"
~~~

This maximum applies to the module's WebAssembly linear memory. It does not cap
JavaScript heap use, browser-process memory, wall-clock time, or concurrency.

## Build and verify locally

Install the wasm32-unknown-unknown Rust target and a `wasm-bindgen` CLI matching
the version in Cargo.lock. The smoke runners use the workspace conformance
suite at `tests/fixtures/conformance-v1.json`.

~~~powershell
$env:CARGO_TARGET_DIR = "target/browser-full"
$env:CODEGRID_WASM_MAX_MEMORY_BYTES = "67108864"
cargo test -p codegrid-wasm-browser
cargo check --target wasm32-unknown-unknown -p codegrid-wasm-browser
cargo build --release --target wasm32-unknown-unknown -p codegrid-wasm-browser
wasm-bindgen target/browser-full/wasm32-unknown-unknown/release/codegrid_wasm_browser.wasm --target web --out-dir target/browser-full/browser-bindings
wasm-bindgen target/browser-full/wasm32-unknown-unknown/release/codegrid_wasm_browser.wasm --target nodejs --out-dir target/browser-full/browser-node-bindings
cargo build --release --target wasm32-unknown-unknown -p codegrid-wasm-server
node crates/codegrid-wasm-browser/tests/run-node-smoke.mjs
node crates/codegrid-wasm-browser/tests/run-browser-smoke.mjs
~~~

The Node and headless Chrome/Edge runners execute the shared Full fixtures
against generated bindings and compare the Runtime API v5 projections. Run the
server harness separately when a native baseline is needed. Native unit tests
and Node smoke tests do not establish production-browser memory or concurrency
safety.
