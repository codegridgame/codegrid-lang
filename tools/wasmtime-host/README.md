# CodeGrid Wasmtime Host Harness

This standalone Rust tool embeds the no-import `wasm32-unknown-unknown`
`codegrid-wasm-server` artifact in Wasmtime 49.0.1. It verifies the Server ABI
v4 exports and configured linear-memory maximum, applies per-store memory and
Wasm stack limits, initializes one persistent module instance, and executes all
63 shared Full fixtures through the exported JSON ABI. For each case it invokes
the Native CLI as an oracle and compares the complete observable run projection:
status, ordered events, newly emitted output, and the full snapshot including
program mutation, threads, memory, metrics, errors, and faults. It writes the
same complete Native CLI projections to
`target/wasmtime-host/native-cli-full-baseline-v1.json` for the Node and
headless-browser smoke runners. Since the fixture file has no per-run
work-unit field, fixture runs use a uniform 1,000,000-unit ceiling; a separate
one-unit probe verifies deterministic work-limit yielding and preservation of
committed deltas. A disposable session also verifies that Wasmtime fuel
exhaustion traps and causes the harness to discard that session.

The tool is outside the main Cargo workspace and has no CodeGrid crate
dependencies. It provides no WASI or host imports. Wasmtime's memory limiter is
per linear memory; it does not bound Wasmtime bookkeeping, JIT compilation,
embedder allocations, or aggregate process memory. Fuel is a deterministic
WebAssembly operator budget, not a wall-clock deadline. This harness does not
implement the production game backend or its process, concurrency, or
cancellation policy.

Build the Native CLI and server module, then run the harness from the repository
root. Run it before the browser smoke tests so they can consume the generated
Native CLI baseline:

```powershell
$env:CODEGRID_WASM_MAX_MEMORY_BYTES = "67108864"
cargo build --release -p codegrid-cli
cargo build --release --target wasm32-unknown-unknown -p codegrid-wasm-server
cargo run --manifest-path tools/wasmtime-host/Cargo.toml --release -- target/wasm32-unknown-unknown/release/codegrid_wasm_server.wasm
```

The Wasmtime crate is pinned to 49.0.1. Wasmtime 49 requires Rust 1.96 or
newer. The memory environment value must match the value used to build the
server artifact.
