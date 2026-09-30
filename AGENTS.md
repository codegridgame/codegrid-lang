# AI Agent Rules for CodeGrid

These rules apply to all AI-assisted work in this repository.

## Language

- Use English for project content, especially externally visible files and text: source code comments, public documentation, examples, tests, fixture descriptions, diagnostics, CLI output, UI strings, commit messages, and pull request text. Use English names for new code identifiers.
- Write new files and new sections entirely in English. When editing an existing non-English section, translate the section being changed so that the result does not mix languages. Do not translate unrelated historical material without a task reason.
- Preserve literal CodeGrid instruction tokens, required file paths, third-party names, and verbatim user-provided data. The English-only rule does not change the `.cg` language syntax.
- Communicate with the user in Chinese by default. Chat messages are not project artifacts and are not subject to the English-only project-content rule. If the user requests another language for a particular response, follow that request.
- Do not impose an English-only requirement on user input. Translate user-provided wording when incorporating it into project files, unless it must be preserved verbatim.

## Sources of truth

- `spec/codegrid-source-spec.md` defines the complete Full `.cg` syntax and static validation contract. Update it only through an explicit recorded decision when evidence exposes a genuine gap or conflict.
- `spec/codegrid-vm-spec.md` defines the normative Full execution contract, including every Primary, Attachment, phase, error, determinism rule, work-unit rule, and raw metric. Its Section 17 lists direct conformance cases still required.
- `docs/decisions.md` records resolved Full decisions and remaining host questions. Historical drafts and protocol-only fixtures are evidence to review, not normative contracts.
- `spec/codegrid-error-codes.md` and its JSON registry define stable error identities, triggers, return surfaces and compatibility. Assign identifiers at detection; never infer them from message wording. Preserve published VM/WASM spellings and recorded transport aliases.
- Do not silently change either specification to make an implementation convenient. Record and resolve a genuine conflict before depending on one interpretation.

## Architecture

- Keep the Rust dependency direction one-way and consistent with this current workspace graph:

  ```text
  codegrid-syntax       -> codegrid-model
  codegrid-hir          -> codegrid-model, codegrid-syntax
  codegrid-ir           -> codegrid-model
  codegrid-compiler     -> codegrid-hir, codegrid-ir, codegrid-model, codegrid-syntax
  codegrid-vm           -> codegrid-ir, codegrid-model
  codegrid-runtime-api  -> codegrid-compiler, codegrid-ir, codegrid-model, codegrid-vm
  codegrid-cli          -> codegrid-compiler, codegrid-ir, codegrid-model, codegrid-syntax, codegrid-vm, codegrid-level-api
  codegrid-lsp          -> codegrid-compiler, codegrid-model, codegrid-syntax
  codegrid-wasm-browser -> codegrid-runtime-api
  codegrid-wasm-server  -> codegrid-runtime-api
  codegrid-level-core   -> codegrid-ir, codegrid-model, codegrid-vm
  codegrid-level-api    -> codegrid-level-core, codegrid-compiler, codegrid-ir, codegrid-model
  codegrid-level-wasm-browser -> codegrid-level-api
  codegrid-level-wasm-server  -> codegrid-level-api
  ```

  Native CLI check/run/debug intentionally compose compiler and VM APIs directly
  at the process boundary; evaluate delegates to `codegrid-level-api`.
  Browser/server language adapters use `codegrid-runtime-api` for
  their versioned host contract. Do not introduce dependency cycles or a
  parallel `codegrid-core` model.
- Keep model, syntax, HIR, IR, compiler, and VM independent of CLI, LSP transport, VS Code, browsers, JavaScript bindings, operating-system I/O, and game rules. Pass source, input, seed, limits, and results as data.
- Treat verified IR as the only executable VM input. Validate IR after decoding or deserialization; do not let hosts or adapters construct unchecked runtime programs.
- Keep `codegrid-cli`, `codegrid-lsp`, `codegrid-runtime-api`, `codegrid-wasm-browser`, and any future `codegrid-wasm-server` thin. File access and process exit codes belong to CLI; JSON-RPC and document lifecycle belong to LSP; the runtime API owns only versioned request/response conversion, request and handle checks, resource ceilings, and instance lifecycle; host ABI conversion belongs to the corresponding WASM adapter. None may duplicate source acceptance, instruction tables, compilation, or VM behavior.
- Design the same compiler and VM core for native and `wasm32`. Frontend and backend WASM hosts must pass explicit data and receive versioned results; JavaScript `Number` must not carry values that exceed its exact integer range, including `u64` seeds or unbounded Page/address values.
- Keep source spans as UTF-8 byte offsets in the core. Convert them explicitly to UTF-16 positions at LSP/editor boundaries; never assume byte offsets are editor columns.
- Keep VM behavior deterministic. Do not derive language results from wall-clock time, host randomness, thread scheduling, or unordered container iteration.
- Keep scoring, levels, Steam integration, and game UI outside this repository's language core.
  The Rust level evaluation layer lives above the language core as
  described in `docs/level-core-architecture.md`; logical level validation and
  scoring belong there, never in model/compiler/VM or host code. Its actual
  dependencies appear in the workspace graph above. Web, Steam, and backend level evaluation must
  invoke the shared Rust evaluator through WASM.

### Prohibited shortcuts

- Do not implement a second semantic parser, validator, instruction inventory, or interpreter in TypeScript, CLI, LSP, runtime API, browser code, or server code.
- Do not make the core read paths, environment variables, network, clocks, or host randomness, or depend on Node.js, Electron, VS Code APIs, `wasm-bindgen`, WASI, or a specific WASM runtime.
- Do not panic on user source, CLI input, or host-provided IR. Return structured diagnostics or validation errors; reserve assertions for internal invariants that cannot be influenced by external data.
- Do not make a `run` loop with separate semantics from `step`; bounded execution must repeatedly apply the same deterministic outer-tick transition.
- Do not expose animation timing, UI state, game scoring, level policy, or Steam integration from the language core.
- Do not claim native/WASM parity from compilation alone; compare execution results in actual native, browser-WASM, and server-WASM hosts before making that claim.

## Changes and verification

- Make the smallest coherent change that satisfies the task. Preserve unrelated user changes and do not claim a proposed component already exists.
- Add or update focused tests when changing language behavior, source acceptance, formatting, or cross-host boundaries. Use valid Full `.cg` fixtures for parser acceptance; review the seven old M0 fixtures and rewrite them before promoting them to conformance evidence.
- Check formatting, build, and relevant tests when the tools are available. Report exactly which checks ran and any verification limits; do not claim native or WebAssembly parity without running the relevant comparison.
- Update affected documentation and links when renaming files or changing public behavior. Keep examples consistent with the current specifications.

## Module-local rules

- Each implemented crate and editor package has a nested `AGENTS.md` that states its responsibilities, allowed call/dependency direction, and prohibited behavior. Read the closest applicable file before changing that module; nested rules supplement these repository-wide rules.
- Keep module-local rules in English and consistent with the actual Cargo/package dependency graph. Update the relevant `AGENTS.md` whenever an allowed dependency or module responsibility changes.
- Do not create empty crate directories or placeholder module rules for components that have not started implementation. Add module rules in the same change that starts implementing a crate or editor package.
