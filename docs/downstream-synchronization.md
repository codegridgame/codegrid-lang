# Mandatory downstream synchronization

## Current boundary and synchronization status (2026-10-18)

The game-domain implementation has been moved out of this repository in the
current working trees. `codegrid-lang` retains `.cg` syntax, compilation,
verified IR, deterministic VM transitions, neutral step/work-unit safeguards,
the language CLI/LSP/VS Code tools, and Full Runtime hosts. The Rust Game
Runtime in `C:/source/bf-steam-wt/rust/` owns levels, scenes, game budgets and
metrics, case selection, game errors, and result judgment. Game-only Level
crates, the game `evaluate` CLI command, and Level/Scene fixtures are absent
from the Lang workspace.

The Game Runtime also owns interactive case debugging. A Debug session names
one visible source test index; the runtime steps that case, compares its output
with its expectation, and stops with that case's result. It does not aggregate
or judge the full level. Generic language debugging remains in Lang.

The source split and current consumers are verified in these local working
trees, with one reproducibility limit: the Game Runtime Cargo files still pin
Lang revision `9e07063c8f7cf5734f86b285c8a9264c9307bfe`, which predates the
neutral prepared-step APIs consumed by the game runtime. The current Rust
workspace and Level WASM artifacts were built with the explicitly recorded
local Lang patch override. They run and pass the recorded checks, but a clean
checkout cannot rebuild them from the configured pin until the Lang changes
exist at an immutable revision. Do not replace the pin with an absolute local
path or describe the override as a published revision. Exact source and
artifact identities are in the [Game Runtime synchronization record](../../bf-steam-wt/doc/codegrid-lang同步规则.md).

Game decision and verification history formerly stored here has moved to the
[Game Runtime history archive](../../bf-steam-wt/doc/codegrid-lang-history/README.md).

## Generic VM work-unit smoke failure observed (2026-10-18)

The configured work-unit ceiling did not bound one Custom candidate transition
in the server WASM smoke. The release artifact build succeeded. The exact build
and failing smoke commands were:

```powershell
$env:CODEGRID_WASM_MAX_MEMORY_BYTES='67108864'
cargo build --locked --release --target wasm32-unknown-unknown -p codegrid-wasm-server
node crates/codegrid-wasm-server/tests/server-smoke.mjs
```

It failed on fixture `custom-wrapping-loop-reaches-internal-limit` in
[`tests/fixtures/conformance-v1.json`](../tests/fixtures/conformance-v1.json).
This is fixture 59 of 105. The case runs
`@main\n~> #0 ;\n@end main\n@C0\n@size 2x1\n~> +\n@end C0\n`
with seed `0`, empty input, and `max_steps: "10"`; server limits include
`max_work_units_per_call: "1000000"`. The primary trap is Rust allocation
failure (`RuntimeError: unreachable`) through
`std::alloc::rust_oom` / `handle_alloc_error` / `RawVec::reserve`, then
`codegrid_vm::engine::full::execute_primary`, `evaluate_thread`, and
`Vm::step_with_budget_and_footprint`. The request is processed by
`codegrid_server_process_request` in
[`crates/codegrid-wasm-server/src/lib.rs`](../crates/codegrid-wasm-server/src/lib.rs).

The harness then calls `codegrid_server_free_buffer` from the `dispatch()`
`finally` block in
[`server-smoke.mjs`](../crates/codegrid-wasm-server/tests/server-smoke.mjs).
That produces a secondary `core::cell::panic_already_borrowed` panic because
the WASM trap interrupted processing while the request-buffer borrow was held.
The secondary cleanup panic does not explain or replace the primary VM OOM.
At the observed source lines, `dispatch()` cleanup is at `server-smoke.mjs:100`
and the Rust export is `codegrid-wasm-server/src/lib.rs:325`. An isolated
Node reproduction that initializes the server, compiles only this source,
creates one instance, and runs it with the same host limits captured both
stacks (symbol chains summarized here):

```text
PRIMARY run: RuntimeError: unreachable
  rust_oom -> alloc::raw_vec::reserve -> codegrid_vm::engine::full::execute_primary
  -> codegrid_vm::engine::full::evaluate_thread
  -> Vm::step_with_budget_and_footprint
CLEANUP free_buffer: RuntimeError: unreachable
  core::cell::panic_already_borrowed -> codegrid_server_free_buffer
```

This fixture remains a generic VM work-unit conformance case. Its
`expected.process_exit_code` value (`7`) remains in Lang because the native CLI
fixture test uses it to assert the language CLI exit status. Removing the
duplicate game-field absence assertions changed no VM implementation, fixture
source, CLI expectation, run request, or work ceiling.
No server-smoke result was recorded before these working-tree edits, so there
is no before/after evidence to attribute the observed failure to that cleanup.
The run used a dirty worktree based on Lang HEAD
`9e07063c8f7cf5734f86b285c8a9264c9307bfe2`; no complete patch or artifact
hash was recorded. Treat generic VM work-unit
bounding as unresolved until the same server smoke passes after the VM guard is
repaired. Do not count downstream server verification as complete meanwhile.

## Every-change requirement

The [release policy](../AGENTS.md#release-and-compatibility-policy) takes
precedence over legacy-preservation requirements below. No user-published
release exists as of 2026-10-08. Synchronize current contracts and consumers
directly; do not add compatibility readers, retain old artifacts, reserve old
codes, or require migrations/version bumps for unpublished versions. Continue
to preserve unrelated user data and record truthful artifact provenance.

For every Lang change, inspect its downstream impact before reporting
completion. Changes to syntax, instruction permissions, compilation, VM
behavior, transition/work-unit metrics, language error identities, or public language
API/WASM contracts must include the affected Rust Game Runtime and application
consumer updates in the same task. Game-specific scene, level, scoring, and
limit changes are authored in `C:/source/bf-steam-wt/rust/`; they do not make
`codegrid-lang` the owner of those contracts. Do not leave a required language
integration update as an optional follow-up.

Documentation-only and internal changes may have no downstream effect. Establish
that by checking the affected contracts and consumers, and report the concrete
reason. A clean upstream test run is not evidence of downstream compatibility.
This is an AI workflow requirement; it does not install a filesystem watcher.

## Required workflow

1. Read both repositories' root `AGENTS.md` files and the closest applicable
   module rules before editing. Read the downstream
   [synchronization rules](../../bf-steam-wt/doc/codegrid-lang同步规则.md).
   Inspect both working trees and preserve unrelated user changes.
2. Identify changed normative contracts and every affected downstream consumer.
   Search imports, host adapters, generated assets, schemas, examples, prompts,
   error displays, and tests. The inventory below is a starting point, not an
   exhaustive file whitelist.
3. Update upstream implementation, specifications, decisions, and focused tests
   as required by its rules. Update downstream callers and authoring contracts
   against the same final behavior. Keep execution semantics in the shared Rust
   compiler/VM/evaluator; do not add a TypeScript semantic implementation.
4. Rebuild affected WASM modules and JavaScript bindings with the pinned
   toolchain. Synchronize the complete generated dependency set, including
   snippets when present. Update provenance and vendor manifests using actual
   source identities, versions, and file hashes. Never relabel an old binary as
   the new implementation.
5. Update downstream examples, fixtures, AI generation prompts, documentation,
   and module rules where their behavior or responsibilities changed. Add
   compatibility readers only for an explicitly published baseline; do not silently rewrite
   player programs or conflate a historical filename with a schema version.
6. Run relevant upstream tests and downstream type checks, unit tests, builds,
   and actual-host integration checks. Verify the affected execution path, not
   just artifact compilation. Fix synchronization failures before completion.
7. Report the upstream source identity, affected downstream files or consumers,
   checks performed, and any remaining limitations. If no synchronization was
   needed, state the reviewed change and evidence for that conclusion.

## Current downstream consumers

| Upstream change | Downstream areas to inspect |
| --- | --- |
| Full source, compiler, or VM semantics | `C:/source/bf-steam-wt/rust/crates/codegrid-game-runtime/` language adapter, current WASM consumers, and relevant app conformance tests |
| Full browser Runtime API and language behavior | Current Full browser WASM assets and binding, Web runtime host, transport adapters, browser integration tests, and Rust Game Runtime adapter |
| Full server ABI and Runtime API | Current Full server WASM assets, Worker runtime host, `packages/api-contract/`, Worker smoke and conformance fixtures, and Rust Game Runtime adapter |
| Language instructions, permissions, diagnostics, or error identities | Rust language adapter, editor/authoring validation, templates, prompts, examples, and language diagnostic presentation |
| Raw VM transition/work-unit fields or error behavior | Rust Game Runtime adapter and language host consumers that read these fields; verify the affected Runtime API, WASM, CLI, and debug contracts explicitly |

Discover current paths and versions before each update. Archived downstream
reports do not establish that their former paths or versioned artifacts remain
active.

### Artifact-specific requirements

- Language editor/browser assets come from `codegrid-wasm-browser`; keep their
  source/build provenance truthful and preserve explicit local-byte loading
  where the host requires it. Game Runtime WASM is built and versioned by the
  Rust Game Runtime repository, not by a Lang level crate.
- Web Full assets come from `codegrid-wasm-browser`; server Full assets come from
  `codegrid-wasm-server`. Update host constructor arguments, requests, versions,
  result/status/error shapes, quotas, and projections whenever contracts change.
  Retain old versioned artifacts only when an explicitly published baseline
  requires them; current unpublished consumers use current artifacts directly.
- Keep source commit/dirty-state identity and fixture hashes truthful. Regenerate
  provenance through the owning repository's build tools. Language API/ABI
  versions and game level `format_version` are separate contracts.
- Keep downstream strict TypeScript rules. Generated declarations must not
  introduce prohibited `any`; maintain the existing narrow host declarations.

## Verification and completion

Select checks according to the affected consumer. The downstream root provides
`npm run typecheck`, `npm run build:editor`, `npm run test:e2e:editor`,
`npm run build:web`, and `npm run build:steam`. Inspect current workspace scripts
for editor artifact integrity, Web browser/server parity, actual Worker
conformance, and editor offline Electron acceptance. Run the Lang Full VM/API
checks plus Rust Game Runtime tests and relevant actual-host comparisons for the
affected boundary; the removed Lang scene-WASM harness is no longer an active
verification entry point.

Do not claim browser tests establish real Steamworks, physical controller,
installer, audio, or cloud acceptance. Do not deploy, enable production runtime
features, commit, or push merely to satisfy this synchronization rule; follow the
user's authorization and repository rules for those actions.

If a repository, toolchain, required host, or check is unavailable, report
**downstream synchronization incomplete**, naming the affected files, blocker,
completed work, and remaining verification. Do not silently skip the consumer
or report the overall mechanism change as fully complete.
