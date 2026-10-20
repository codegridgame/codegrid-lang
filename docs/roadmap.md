# Full Language Implementation Roadmap

Language cleanup decisions and current instruction behavior are recorded in
the language specifications and decision log. The former mixed language/game
cleanup audit is archived in the Game Runtime history index.

**Implemented amendment (2026-10-08):**
[Status Flag and directionless READ](status-flag-and-read.md) records the F
register behavior, `?!`, directionless `,`, and empty POPADD flag. Language
behavior remains in the current language contracts; game permissions, Size,
Gas, and level restrictions are owned by the game runtime.

This roadmap tracks the complete Full CodeGrid language and its local development hosts. Detailed work and acceptance evidence are in the [Full language task book](../tasks/spec-completion.md).

| Milestone | Deliverable | Acceptance gate | State |
| --- | --- | --- | --- |
| 0. Full contracts | Normative Full source and VM contracts; versioned CLI, Runtime API, and host contracts | Every Full behavior has an owner and an explicit rule; host versions and compatibility rules are documented | Complete: source, VM, CLI, Runtime API v5, browser binding, and server ABI v6 contracts are documented |
| 1. Source and compiler | Complete syntax, nested board definitions, symbol resolution, diagnostics, lowering, and verified Full IR | All structures, Primary instructions, Attachments, references, dimensions, and malformed forms have positive and negative cases | Complete; source/compiler conformance and IR boundary-verification suites pass |
| 2. Full VM | Deterministic multi-thread execution, transactions, calls/tail calls, Folded Blocks, Repeat, Custom invocations, stacks, memory, Page, randomness, code modification, errors, snapshots, and metrics | Full VM conformance suite covers success, conflicts, rollback, limits, and deterministic seed vectors; `run` uses the same transition as `step` | VM suites and spec §17 pass; the generic work-unit guard, rollback, and 106-fixture server smoke pass on the current revision |
| 3. Local developer tools | Full Native CLI, Rust LSP, and VS Code language features over the shared compiler and VM | CLI configuration and output match the Full contract; editor diagnostics and positions match shared compiler spans; no duplicate semantics exist in adapters | Complete; CLI, LSP, editor suites, and real Extension Host—LSP/CLI parity test pass |
| 4. Runtime API and WASM | Runtime API v5 implementation, browser adapter, and server adapter | Same reviewed fixtures pass in native, actual browser-WASM, and actual server-WASM hosts with matching observable state and errors | Runtime API v5 / Server ABI v6 pass 106 shared fixtures in native CLI, Wasmtime, Node browser/server WASM, and actual browser WASM; downstream Game Runtime integration remains incomplete |
| 5. Full conformance and docs | Shared fixtures, reviewed historical tests, examples, architecture and usage documentation | Every normative Full requirement has direct evidence; historical test expectations have been reviewed and promoted explicitly | The 106 shared fixtures pass across current hosts; continue closing any remaining direct conformance cases listed in the VM specification |

Production authentication, game integration, deployment policy, and operational service budgets are outside this local development roadmap. A native build or a WebAssembly compile does not establish cross-host execution parity.

## Function registers and NEG implemented (2026-10-08)

[Function-private registers and NEG](function-registers-and-neg.md) are implemented, including observable private banks, independent NEG permissions and editor/debugger support. The earlier 81-case native CLI, browser, Node and Wasmtime comparison is historical; the former server WASM work-unit failure is resolved as documented in [downstream synchronization](downstream-synchronization.md). That implementation used executable IR format 3; Folded Block Attachments advance the current format to 4, and older IR must be recompiled.

## Local editor execution and debugging

The VS Code extension now connects Run/Debug commands and F5/Ctrl+F5 to the
native CLI's versioned JSON-lines debug transport. Compiler-resolved source
maps drive cell breakpoints and UTF-16 source locations. The shared VM supplies
atomic transition steps, thread and Function frames, registers, memory,
stacks, input/output, committed-step counts, errors, and committed events.
Prepared-step callers can inspect candidate-local `StepFootprint` observations.
Custom execution remains atomic within its caller transition. The Windows x64 VSIX bundles the current
native executable. See the [editor guide](../editors/vscode/README.md) and
[CLI transport contract](cli.md#editor-debug-transport).
