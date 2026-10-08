# Wasmtime Host Harness Rules

## Responsibility

This standalone tool embeds the built `codegrid-wasm-server` artifact in the
official Wasmtime Rust runtime. It verifies the server ABI, no-import boundary,
runtime limits, and the shared Full conformance fixtures.
The `level-parity` binary also verifies the independently versioned Level ABI
2 against actual native CLI scene responses using the shared scene manifest.

## Dependencies and boundaries

- Keep this package outside the Cargo workspace. It consumes the built WASM
  artifact and fixture files as data; it must not depend on CodeGrid crates.
- Keep Wasmtime dependencies here. Do not add a server-runtime dependency to
  `codegrid-wasm-server` or any semantic crate.
- Do not provide WASI, host imports, filesystem access, network access, or a
  second CodeGrid compiler or VM.
- Do not present this harness as the production game backend or as an aggregate
  process-memory, concurrency, or deployment control.

## Verification

Build the server artifact with `CODEGRID_WASM_MAX_MEMORY_BYTES`, then run this
tool with the artifact path and the same configured memory value. Keep the
Wasmtime dependency pinned and review fuel, memory, and stack behavior whenever
the pinned Wasmtime release changes.

`level-parity` verifies ABI/API 2 against shared format-v1 scene levels,
complete native CLI results, and permitted Debug event traces. The current manifest includes converted ExactIO regression cases; preserve
exact original input text. No historical ABI-1 path remains.
