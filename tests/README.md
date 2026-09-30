# Full Language Conformance Fixtures

This directory contains host-neutral cases for observable CodeGrid behavior. Native Rust, CLI, LSP where relevant, browser/server WASM, and game adapters should reuse reviewed cases rather than define separate language semantics.

## Reviewed historical protocol cases

The retained TOML protocol cases record expected observations independently of a parser or VM. Their embedded `program.text` uses superseded draft syntax and is not valid `.cg` source. Treat them as review material; rewrite the source and verify expected traces against the Full specifications before promoting any case.

## Retained diagnostic and runtime cases

`fixtures/source-diagnostics-v1.json` is a diagnostic smoke case and does not define the current diagnostic contract. `fixtures/conformance-v1.json` and its adjacent `.cg` programs form the shared Full execution suite, including functions, Attachments, stacks, memory, and Custom instructions. Compiler/IR/VM historical suites are retained as supplemental regression evidence; the active Full specifications and conformance suite are normative.

## Existing host smoke cases

The shared Full runtime suite contains 63 cases. A complete Native CLI result baseline is generated from the suite and consumed by Node/Chrome browser smoke and the Wasmtime server host harness. These actual hosts compare the complete observable run projection, including events, output deltas, snapshots, threads, stacks, Page, mutable code, memory, errors, faults, and raw metrics.

## Full acceptance suite

The versioned, host-neutral fixture index covers Full runtime behavior. Each valid case records source, seed, boundary mode, input, initial memory, Custom execution limit, tick and work limits, expected status, complete observable state, errors, events, and raw metrics. Invalid cases record diagnostic spans and must never produce executable IR. Extend the suite when a normative requirement lacks a direct case.

Run the same reviewed cases through Rust/compiler layers, CLI, Runtime API, browser-WASM, and server-WASM. Compare all contract-visible results, including thread state, stacks, Page, memory observations, mutable code, input/output, errors, events, and metrics. No host may add semantic expectations of its own.
