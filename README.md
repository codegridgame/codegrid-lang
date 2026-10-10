# CodeGrid Language

**Release policy:** No release has been published by the user. Until the user
explicitly publishes a release, update contracts and consumers directly without
backward-compatibility requirements. Internal version numbers and historical
verification reports are not release baselines. See [repository rules](AGENTS.md#release-and-compatibility-policy).

The current language includes fixed conditional prefixes `?0`–`?2` and `?!` (F=1), CMP `?=`, and random direction `??`; READ is directionless `,` (code 44), with F=0 on success and F=1 on exhaustion; Function registers are private copies and `$!` provides NEG (code 69); executable IR is format 3. See the [migration manual](docs/conditional-prefix-migration.md) for semantics, retired forms, compatibility, and verification.

All normal boards wrap horizontally and vertically. Main, Function and Custom
boards without an explicit Entry start at `(0, 0)`, facing Right. Explicit
Entries keep their configured positions and directions; they suppress the
default entry. For example, `, . ;` reads and outputs one byte, then halts,
without requiring `~>`. See [the recorded decision](docs/decisions.md#toroidal-boards-and-implicit-entries-2026-10-11).

CodeGrid is a grid-based programming language built around a shared Rust compiler and deterministic VM. The development target is the complete language, including multiple execution threads, functions, Custom instructions, Folded Blocks, stacks, memory, attachments, and deterministic randomness.

## Full language development status

The workspace implements the complete Full language across the shared Rust compiler and deterministic VM, native CLI, LSP/editor, Runtime API v3, and browser/server WebAssembly adapters. The local task book records the implementation and host verification evidence. Final acceptance requires all shared Full fixtures to produce matching observable results in native, browser-WASM, and server-WASM hosts.

The existing native CLI provides:

- `codegrid check <program.cg>` using the shared compiler;
- `codegrid run <program.cg>` using the compiler and verified IR in the shared VM.
- `codegrid debug --stdio` providing compiler source locations and atomic VM steps to the editor debugger.
- `codegrid evaluate <level.json> <program.cg>` using the shared Rust Scene API 2 for ExactIO, Robot and MechanicalArm; see [the CLI contract](docs/cli.md#evaluate-level-command).

The [VS Code extension](editors/vscode/README.md) opens `.cg` files with Run
and Debug buttons, F5/Ctrl+F5 launch, Main/Function/Folded Block breakpoints,
Global Tick stepping, call frames, state variables and watches. The Windows
x64 installation package includes the native execution runtime.

Full acceptance includes the execution settings, complete VM state and metrics, and host results defined by the Full contracts.

Local acceptance must compare the same Full fixtures in native, browser-WASM, and server-WASM hosts. A successful Rust or WebAssembly compile alone does not establish execution parity.

## Workspace layers

```text
codegrid-model       Shared values and instruction inventory
codegrid-syntax      Tokenization, source structure, UTF-8 byte spans
codegrid-hir         Resolved source-level representation
codegrid-ir          Versioned executable representation and verifier
codegrid-compiler    Static acceptance, diagnostics, verified IR lowering
codegrid-vm          Deterministic state transitions and snapshots
codegrid-cli         Native check/run/debug commands and host I/O
codegrid-lsp         Static language services over the Language Server Protocol
codegrid-runtime-api Versioned host request/response and instance lifecycle
codegrid-wasm-browser Browser WebAssembly adapter
codegrid-wasm-server Server WebAssembly adapter
codegrid-level-core  Logical level validation, ExactIO, metrics and rating
codegrid-level-api   Versioned level requests, profiles, handles and results
codegrid-level-wasm-browser Browser Level binding v1
codegrid-level-wasm-server  Portable no-import Level ABI v1
editors/vscode/       TypeScript editor client without language semantics
```

## Documentation

- [Level Core v1 specification](spec/codegrid-level-core-spec-v1.md) and [Level Core architecture](docs/level-core-architecture.md) define the shared Rust ExactIO evaluation layer and Web, Steam, and backend WASM integration boundaries.
- [ExactIO implementation contract](spec/codegrid-level-exactio-contract-v1.md) records the decided first-phase evaluation rules.
- [Rust level evaluation task book](tasks/level-core-exactio-v1.md) covers CLI and WASM delivery, Environment extension design, and actual-host result comparisons.

- [Error code specification](spec/codegrid-error-codes.md) defines all scoped identifiers and stable error categories.

- [Source specification](spec/codegrid-source-spec.md) defines complete Full `.cg` syntax and static acceptance.
- [VM specification](spec/codegrid-vm-spec.md) defines Full execution, errors, metrics, deterministic work accounting, and host boundaries.
- [Full language task book](tasks/spec-completion.md) defines the complete local implementation and verification work.
- [Native CLI contract](docs/cli.md) defines the local Full command options, versioned JSON result, and stable exit codes.
- [Architecture](docs/architecture.md), [roadmap](docs/roadmap.md), and [documentation index](docs/README.md) describe ownership, dependencies, and Full delivery status.
- [AI contribution rules](AGENTS.md) define project language, architecture, and verification requirements.

Retained historical `.cg` examples and protocol cases have been reviewed against the Full specifications before use as conformance fixtures. See [examples status](examples/README.md) and the [test fixture guide](tests/README.md).

## Verification

Run the checks relevant to the change and report exactly which ones ran. The general Rust workspace checks are:

```text
cargo fmt --all -- --check
cargo test --workspace
```

A successful `wasm32` build proves compile-time portability only. It does not prove actual browser/server execution or cross-host parity.
