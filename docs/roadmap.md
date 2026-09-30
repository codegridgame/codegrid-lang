# Full Language Implementation Roadmap

This roadmap tracks the complete Full CodeGrid language and its local development hosts. Detailed work and acceptance evidence are in the [Full language task book](../tasks/spec-completion.md).

| Milestone | Deliverable | Acceptance gate | State |
| --- | --- | --- | --- |
| 0. Full contracts | Normative Full source and VM contracts; versioned CLI, Runtime API, and host contracts | Every Full behavior has an owner and an explicit rule; host versions and compatibility rules are documented | Complete: source, VM, CLI, Runtime API v3, browser binding, and server ABI v4 contracts are documented |
| 1. Source and compiler | Complete syntax, nested board definitions, symbol resolution, diagnostics, lowering, and verified Full IR | All structures, Primary instructions, Attachments, references, dimensions, and malformed forms have positive and negative cases | Complete; source/compiler conformance and IR boundary-verification suites pass |
| 2. Full VM | Deterministic multi-thread execution, transactions, calls/tail calls, Folded Blocks, Repeat, Custom invocations, stacks, memory, Page, randomness, code modification, errors, snapshots, and metrics | Full VM conformance suite covers success, conflicts, rollback, limits, and deterministic seed vectors; `run` uses the same transition as `step` | Complete; VM suites and all VM spec §17 direct coverage gates pass |
| 3. Local developer tools | Full Native CLI, Rust LSP, and VS Code language features over the shared compiler and VM | CLI configuration and output match the Full contract; editor diagnostics and positions match shared compiler spans; no duplicate semantics exist in adapters | Complete; CLI, LSP, editor suites, and real Extension Host—LSP/CLI parity test pass |
| 4. Runtime API and WASM | Runtime API v3 implementation, browser adapter, and server adapter | Same reviewed fixtures pass in native, actual browser-WASM, and actual server-WASM hosts with matching observable state and errors | Complete; all 63 Full fixtures match the complete Native CLI projection in Node, headless browser, and Wasmtime server hosts |
| 5. Full conformance and docs | Shared fixtures, reviewed historical tests, examples, architecture and usage documentation | Every normative Full requirement has direct evidence; historical test expectations have been reviewed and promoted explicitly | Complete; workspace, editor, and actual native/browser/server host acceptance checks pass |

Production authentication, game integration, deployment policy, and operational service budgets are outside this local development roadmap. A native build or a WebAssembly compile does not establish cross-host execution parity.

## Local editor execution and debugging

The VS Code extension now connects Run/Debug commands and F5/Ctrl+F5 to the
native CLI's versioned JSON-lines debug transport. Compiler-resolved source
maps drive cell breakpoints and UTF-16 source locations. The shared VM supplies
atomic Global Tick steps, thread and Function frames, registers, memory,
stacks, input/output, metrics, errors, and committed events. Custom execution
remains atomic within its caller tick. The Windows x64 VSIX bundles the current
native executable. See the [editor guide](../editors/vscode/README.md) and
[CLI transport contract](cli.md#editor-debug-transport).
