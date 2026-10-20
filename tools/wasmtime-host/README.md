# CodeGrid Wasmtime Host Harness

This standalone Rust tool embeds the no-import `wasm32-unknown-unknown`
`codegrid-wasm-server` artifact in Wasmtime 49.0.1. It verifies Server ABI v6,
Runtime API v5, the configured linear-memory maximum, per-store memory and Wasm
stack limits, and all shared Full language conformance fixtures. For each case
it invokes the Native CLI as an oracle and compares the complete observable run
projection: status, ordered events, newly emitted output, and the full snapshot
including program mutation, threads, memory, committed-step summary, errors, and faults. It
writes complete Native CLI projections to
`target/wasmtime-host/native-cli-full-baseline-v1.json` for the Node and
headless-browser smoke runners. Fixture execution uses the same uniform
10,000-work-unit call ceiling as the browser smoke runners; a separate one-unit probe verifies bounded
work yielding and preservation of committed transitions. Additional probes
check the zero, per-call, and per-instance step budget errors. A disposable
session verifies that Wasmtime fuel exhaustion traps and causes the harness to
discard that session.

The tool is outside the main Cargo workspace and has no CodeGrid crate
dependencies. It provides no WASI or host imports. Wasmtime's memory limiter is
per linear memory; it does not bound Wasmtime bookkeeping, JIT compilation,
embedder allocations, or aggregate process memory. Fuel is a deterministic
WebAssembly operator budget, not a wall-clock deadline. The harness validates
only the Full language host contract; game execution policy belongs to the
game runtime.

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
