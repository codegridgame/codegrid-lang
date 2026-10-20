# Mandatory downstream synchronization

## Boundary and last fully synchronized baseline (2026-10-18)

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

The source split and consumers at this baseline were verified against immutable Lang
revision `e27e29b1a8b0bf1fd8957c0ad5a25d1e6bada1e6`. The Game Runtime Cargo
manifest and lockfile pin this revision. Its current Rust workspace and four
Level WASM artifacts were rebuilt with `--locked` and without a local Cargo
override. The neutral prepared-step APIs consumed by the Game Runtime are
available at the pinned revision. Exact source and artifact identities are in
the [Game Runtime synchronization record](../../bf-steam-wt/doc/codegrid-lang同步规则.md).

This is a dated successful baseline, not the current suffix-Attachment integration
status. The 2026-10-20 section below records the newer dependency and pending build.

Game decision and verification history formerly stored here has moved to the
[Game Runtime history archive](../../bf-steam-wt/doc/codegrid-lang-history/README.md).

## 2026-10-20 Folded Block Attachments — downstream synchronization incomplete

Full source and verified IR now allow each Folded Block cell to carry any
compatible suffix Attachment. Repeat follows its normal per-transition rules
while preserving Folded Block progress. WriteCode skips a Primary forbidden in
the target Folded Block as a complete no-op: the Primary and instruction-stack
item remain, and no WriteCode metric or code-write effect is recorded. The
current contracts are IR format 4, Runtime API 5, and Server ABI 6.

The Web, Steam sandbox, API contract, and Worker consumers now target Full v5.
Their browser and server artifacts were rebuilt from Lang commit
`8e42e77602371ccde56ce4e8a60d4dc9ce64481d` with `source_dirty=true`,
`wasm-bindgen 0.2.129`, and a 67,108,864-byte linear-memory ceiling. The
browser WASM hash is `8c73a65e1e9df0d637ad372d2f4100dfb01dc0139a53727127438df264e8e98c`;
the server WASM hash is `c87672d43f7f849cc6df09f14515085dd01385dd3a6944cb486e1e6dda8a6bc4`.
Both new Folded Block cases pass native CLI, Wasmtime, browser Node/browser, and
server Node checks; all 106 shared fixtures match the native CLI result.
Downstream type checks, Steam tests, Web and Steam builds, API build, local
Workerd smoke, and five Web-to-Workerd browser tests pass.

The follow-up review removed the obsolete folded-suffix diagnostic from the
registry and all generated ten-locale catalogs, shared the ordinary/folded IR
Attachment validator, and fixed VS Code FoldRepeat source frames. The actual
Extension Host full run reports 138 passing and one initially pending optional
LSP test; a four-test follow-up with the actual LSP passes, including that case.
The seven encodable
forbidden Primary forms are checked for WriteCode no-op behavior. Current
documentation indexes, task status, API dimension guidance, and downstream
provenance now distinguish the successful language hosts from the pending
Game Runtime build. Rebuilt Full artifact hashes are recorded above. Steam's
11-test sandbox boundary suite and actual Worker folded Repeat/forbidden-write
probes pass.

The Rust Game Runtime source now validates Folded Block suffix Attachments and
includes them in static instruction-kind metrics and instruction limits. Its
Cargo manifest still pins immutable Lang revision
`8e42e77602371ccde56ce4e8a60d4dc9ce64481d`, which predates the new
`FoldedBlock.attachments` IR field. `cargo test --locked -p
codegrid-game-runtime` therefore fails because that pinned dependency has no
`attachments` field. The four Game Runtime Level WASM artifacts cannot be
rebuilt or truthfully marked as containing the new semantics until a new
immutable Lang revision is available and pinned. The downstream Web artifact
integrity suite consequently reports two failures for stale Game Runtime
provenance; the Full v5 artifact checks pass.

The old Full v4 browser and server files remain in their former directories;
active consumers use the new v5 paths. Removing those generated directories
was refused by the command safety policy, so the no-old-artifacts requirement
is also incomplete. Do not report the cross-repository change as fully
synchronized until the Game Runtime is rebuilt from a new immutable Lang
revision, the v4 files are safely removed, and the complete downstream suite
passes.

## Historical VM work-unit smoke failure and current verification (2026-10-18)

An earlier server WASM smoke run, based on Lang revision
`9e07063c8f7cf5734f86b285c8a9264c9307bfe2`, failed to bound one Custom
candidate transition. The release artifact built, but the exact smoke sequence
below failed:

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
fixture test asserts the language CLI exit status. The earlier failure was
reproduced on a dirty worktree based on Lang revision
`9e07063c8f7cf5734f86b285c8a9264c9307bfe2`.

The VM work-unit guard was subsequently updated to detect repeated Custom
semantic states with bounded memory, count the remaining dispatches exactly,
and roll back the uncommitted candidate. On 2026-10-18, the same issue was
rechecked against immutable Lang commit
`e27e29b1a8b0bf1fd8957c0ad5a25d1e6bada1e6`. `cargo fmt --all -- --check` and
`cargo test --workspace` passed. The release server WASM smoke passed all 105
shared fixtures; the Wasmtime 49.0.1 harness passed all 105 with complete
Native CLI parity and regenerated the current baseline; browser Node WASM and
headless-browser runs each passed all 105 fixtures against that baseline.
These checks resolve the recorded smoke failure for this revision. They do not
establish general process-memory, cancellation, concurrency, or wall-clock
limits for production hosts.

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
