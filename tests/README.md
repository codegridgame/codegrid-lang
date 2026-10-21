# Full Language Conformance Fixtures

This directory contains host-neutral cases for observable CodeGrid behavior. Native Rust, CLI, LSP where relevant, browser WASM, and server WASM should reuse reviewed cases rather than define separate language semantics. The Game Runtime has its own game-level cases and invokes the language API; it does not define language conformance.

## Reviewed historical protocol cases

The seven protocol-only M0 TOML drafts and index have been removed. Their source
was never executable Full conformance evidence. Current Full fixtures and active
Rust tests cover wrapping, rollback, Repeat, thread order, Custom stack conflicts,
PRNG vectors and mutable-code scope.

## Retained diagnostic and runtime cases

`fixtures/source-diagnostics-v1.json` is a diagnostic smoke case and does not define the current diagnostic contract. `fixtures/conformance-v1.json` and its adjacent `.cg` programs form the shared Full execution suite, including functions, Attachments, stacks, memory, and Custom instructions. Compiler/IR/VM regression suites remain active, including VM src/state_tests.rs; the active Full specifications and conformance suite are normative.

## Existing host smoke cases

The shared Full runtime suite contains 113 cases, including leading, interior, trailing and all-empty Folded Block layouts. The earlier 81-case acceptance is historical; the current 113-case acceptance passed against native CLI, Wasmtime, Node browser/server WASM, and actual Chromium WASM, as recorded in [downstream synchronization](../docs/downstream-synchronization.md). The smoke harnesses compare the observable run projection, including events, output deltas, snapshots, threads, stacks, Page, mutable code, memory, errors, faults, and committed-step counts.

## Full acceptance suite

The versioned, host-neutral fixture index covers Full runtime behavior. Each valid case records source, seed, input, initial memory, generic step limits, expected status, complete observable state, errors, events, and language-neutral execution observations. Host work-unit ceilings are configured separately. Invalid cases record diagnostic spans and must never produce executable IR. Extend the suite when a normative requirement lacks a direct case.

Run the same reviewed cases through Rust/compiler layers, CLI, Runtime API, browser WASM, and server WASM. Compare all contract-visible results, including thread state, stacks, Page, memory observations, mutable code, input/output, errors, events, committed-step counts, and work-limit outcomes. No host may add semantic expectations of its own.
