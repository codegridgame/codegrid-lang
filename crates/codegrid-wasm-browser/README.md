# CodeGrid Browser WebAssembly Adapter

This crate exposes the shared compiler and VM through a browser and Node
WebAssembly binding. The host-neutral Full contract is
[Runtime API v3](../../spec/codegrid-runtime-api-spec.md); this adapter only
converts JavaScript values, applies browser-host resource limits, and manages
runtime handles. It does not define source syntax or execution behavior.

## JavaScript API

Create a BrowserRuntime for each host isolation boundary:

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
  "10000",   // maximum run ticks per call
  "100000",  // maximum ticks per instance
  "1000000", // deterministic work units per call
);
~~~

The first six limits are nonnegative 32-bit integers. Response and instance
snapshot limits are 32-bit integers of at least 256 bytes. Run, lifetime-tick,
and work-unit limits are positive canonical unsigned decimal strings; keep
them as strings rather than converting them to JavaScript Number.

The binding exposes api_version, check, compile, program_view,
create_instance, step, run, snapshot, release_instance, and
release_program. Each operation returns a JSON string with api_version: 3.
Handles, source spans, Page values, memory addresses, thread IDs, ticks, and
other wide integers use canonical decimal strings. Byte values, register
indexes, slots, dimensions, and coordinates use JSON numbers.

compile(source) returns a runtime-local program handle. To create an instance,
pass the handle, a Uint8Array, a JSON string containing the initial memory
entries, and a JSON string containing the execution configuration:

~~~javascript
const created = JSON.parse(runtime.create_instance(
  compiled.outcome.program,
  new Uint8Array([65, 0, 255]),
  JSON.stringify([{ address: "-1", value: 12 }]),
  JSON.stringify({
    seed: "18446744073709551615",
    custom_execution_limit: "1000",
  }),
));
~~~

run(instance, maxTicks) takes a positive decimal-string tick limit. Its
response includes the bounded result, ordered events and a complete Full
snapshot. step(instance) applies one outer Global Tick. snapshot reads state
without advancing execution. Release both instance and program handles when
they are no longer needed; each handle belongs to one BrowserRuntime.

The adapter checks JavaScript string UTF-8 byte lengths and Uint8Array byte
lengths before copying them into WebAssembly memory. Initial memory is limited
by both entry count and request bytes; addresses must be canonical signed
decimal strings and values must be bytes. Responses and retained instance
snapshots have separate byte limits. A snapshot exceeding its configured
limit causes that instance to be released and a structured error returned.

Runtime API work-unit limits count VM dispatches. They are independent of
browser fuel and tick-slice limits. JSON byte limits do not cap temporary
allocations, JavaScript heap use, total WebAssembly linear memory, wall-clock
time, or browser-process memory.

## WebAssembly memory ceiling

Every build targeting wasm32-unknown-unknown requires
CODEGRID_WASM_MAX_MEMORY_BYTES. Set it to a positive multiple of 65,536 no
greater than 4,294,967,296. For a 64 MiB module ceiling:

~~~powershell
$env:CODEGRID_WASM_MAX_MEMORY_BYTES = "67108864"
~~~

This maximum applies only to the module's WebAssembly linear memory. It does
not cap JavaScript heap use, browser-process memory, wall-clock time, or
concurrency.

## Build and verify locally

Install the wasm32-unknown-unknown Rust target and a wasm-bindgen CLI
matching the version in Cargo.lock. The smoke runners use the workspace
conformance suite at tests/fixtures/conformance-v1.json.

~~~powershell
$env:CARGO_TARGET_DIR = "target/browser-full"
$env:CODEGRID_WASM_MAX_MEMORY_BYTES = "67108864"
cargo test -p codegrid-wasm-browser
cargo check --target wasm32-unknown-unknown -p codegrid-wasm-browser
cargo build --release --target wasm32-unknown-unknown -p codegrid-wasm-browser
wasm-bindgen target/browser-full/wasm32-unknown-unknown/release/codegrid_wasm_browser.wasm --target web --out-dir target/browser-full/browser-bindings
wasm-bindgen target/browser-full/wasm32-unknown-unknown/release/codegrid_wasm_browser.wasm --target nodejs --out-dir target/browser-full/browser-node-bindings
cargo build --release -p codegrid-cli
cargo build --release --target wasm32-unknown-unknown -p codegrid-wasm-server
cargo run --manifest-path tools/wasmtime-host/Cargo.toml --release -- target/browser-full/wasm32-unknown-unknown/release/codegrid_wasm_server.wasm
node crates/codegrid-wasm-browser/tests/run-node-smoke.mjs
node crates/codegrid-wasm-browser/tests/run-browser-smoke.mjs
~~~

The Node and headless Chrome/Edge runners execute all shared Full fixtures
against generated bindings, normalize their complete Runtime API results, and
compare every case with the Native CLI baseline generated by the Wasmtime
harness. This checks ordered events, newly emitted output, and full snapshots,
alongside lifecycle, quotas, exact integer serialization, deterministic
execution, instance isolation, and the configured WebAssembly memory maximum.
Run the Wasmtime harness first so both browser runners can read its baseline.
Full native/server/browser parity is established only when these actual-host
comparisons pass.
