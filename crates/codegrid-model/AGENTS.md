# Module Rules: `codegrid-model`

## Responsibility

Own the canonical language-level values, identifiers, enums, and instruction inventory shared by the compiler, VM, and adapters.

Own the generated, data-only four-digit error identity lookup shared by hosts.
The JSON error registry defines these identities; this module does not detect
host errors or implement transport, editor, or level policy.

## Allowed dependency and call direction

- This crate is the dependency root for semantic Rust crates. It must not depend on another workspace crate.
- `codegrid-syntax`, `codegrid-hir`, `codegrid-ir`, `codegrid-compiler`, `codegrid-vm`, `codegrid-runtime-api`, `codegrid-cli`, and `codegrid-lsp` may depend on this crate.
- Keep public types deterministic, host-neutral, and usable on supported native and WebAssembly targets.

## Prohibited

- Do not add parsing, name resolution, validation, IR execution, VM state, filesystem access, process APIs, clocks, randomness, or game/editor policy here.
- Do not add browser, JavaScript, WASM ABI, LSP, CLI, or operating-system dependencies.
- Do not duplicate the canonical instruction inventory in a downstream Rust or TypeScript module.

## Validation

Add focused unit tests for model invariants. Check formatting and the model crate's native and supported `wasm32` builds when toolchains are available.
