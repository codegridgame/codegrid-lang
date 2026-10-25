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
status. The 2026-10-20 sections below record successive integrations; the later
section is current.

Game decision and verification history formerly stored here has moved to the
[Game Runtime history archive](../../bf-steam-wt/doc/codegrid-lang-history/README.md).

## 2026-10-20 Folded Block suffix Attachments (synchronized)

Lang implementation commit `f593fad6b7f9f46101421d6d6e02452487be2661` was pushed to
`codex/game-domain-extraction`. Game Runtime Cargo.toml and Cargo.lock pin
that immutable revision without a local Cargo override. The current language
contracts are IR format 4, Runtime API 5, and Server ABI 6. No user release
or production deployment was created.

Folded definition cells accept compatible ReadCode, WriteCode, and Repeat
suffixes. Repeat retains the internal cell until completion or prefix
cancellation. A WriteCode attempt that would install a forbidden Folded
Primary preserves the Primary and instruction-stack item and records no
WriteCode operation or code-write effect. Outer invocation suffixes such as
`$0x3` remain invalid. Game Runtime applies suffix permissions, static
instruction-kind accounting, and whole-program instruction limits.

Web/Steam Full browser assets and the API Full server artifact are rebuilt
from the implementation revision. Their manifests preserve the observed
`source_dirty=true` build status and 67,108,864-byte memory maximum.
Browser SHA-256:
`8c73a65e1e9df0d637ad372d2f4100dfb01dc0139a53727127438df264e8e98c`.
Server SHA-256:
`c87672d43f7f849cc6df09f14515085dd01385dd3a6944cb486e1e6dda8a6bc4`.

The same new 2,115,510-byte Level WASM is vendored by Steam, Web, Editor,
and API. Its SHA-256 is
`3761a707b5103dc3ad809a16d4e9665a2b50fa7a85cdf9413e981df36a564364`.
Actual WASM capabilities match the recomputed evaluator identity
`game-runtime-source-sha256:63b96532f5b6b1f913ec7cf250a7bae21d7fbc41b9ffd6ce0bc6bf2f43822fc9`.
The 290-file Rust source digest is
`11831b296e15443c26c5b5272f8f052f05669bbb86376b43316858683a87ecff`;
Cargo.lock SHA-256 is
`a7e36b70d3faf690d079fa811c8dda5c843dc5a6deebc3d0cc5b687201e5e8d5`.
All five Level manifests record the actual dirty game-source tree, matching
current/artifact-origin digests, and the 268,435,456-byte memory maximum.
Editor/API retain explicit-byte initialization. Web and API read evaluator
identity from the vendor manifest instead of maintaining stale literals.
Old Full v4 generated files were removed through recoverable Git deletions.

Validation passes:

- Lang Rust workspace, formatting, error-registry/catalog consistency,
  106 shared fixtures in native CLI, Wasmtime, Node browser/server WASM,
  and actual browser WASM; VS Code Extension Host and actual LSP checks.
- Game Runtime locked Rust workspace tests, formatting, release WASM build,
  and 83 actual Chromium Worker scene executions with Debug traces. Four
  new shared Official/Debug cases cover folded Repeat and forbidden writes;
  the native evaluator also checks that the rejected write adds no operation.
- Downstream full unit suites, workspace type/architecture checks, all four
  application builds, actual local Workerd Full/Level smoke, Worker dry-run
  bundling, and all artifact-source/hash integrity checks.
- Web-to-Workerd browser checks: 5 passed.

Additional application checks: Editor playtest reports 13 passed and three
configured skips across four viewports. Five Steam Electron cases pass in the
initial run, including production file:// scene loading with networking
blocked; the remaining two pass in the final targeted run after aligning
fixtures and locators with current description, permission, fixed-cell,
initial-cursor, and result-dialog behavior. No product UI behavior was changed
for these test updates. The final Steam type check also passes.
Earlier missing-field compilation failures and stale Level artifact integrity
failures are resolved by the immutable pin and actual rebuild above.

## 2026-10-20 Folded Block invocation Attachments and Instruction Codes (synchronized)

Lang commit `5aa85edd7281a6d1bb0ca967b36f0eb98dfaeb79` is pushed to
`codex/game-domain-extraction`. IR format 5, Runtime API 6, and Server ABI 7
are the current unpublished contracts. `$0`–`$9` use Instruction Codes 84–93;
RETURN `]` remains a Primary but has no code. ReadCode is accepted on every
valid Primary and is an inert, uncounted no-op when that Primary has no code.
Folded Block calls accept ReadCode, WriteCode, and Repeat suffixes. Whole-call
Repeat rechecks the call-site prefix; deferred call-site attachments run only
after successful completion. A dynamically installed Folded Block call with
no local definition reports structured VM error 3011,
`FoldedBlockWithoutDefinition`.

The Game Runtime and application consumers were updated against the immutable
Lang revision without a local Cargo override. The downstream repository was
already dirty at HEAD `b7ac7b6e8bff4da8ffc135e5bbd0ec149296884d` and remains
dirty; its 302-file Rust tree SHA-256 is
`d96ae9e23cb6b9a9d788891bd7f4962c115b72e0a2eea4e530e4f73d70ca7e0a`, and
`rust/Cargo.lock` SHA-256 is
`cb3b35cae5db45bca08bf69545ad91feb224df257cb06fb0f5af978e2d86377c`. Its
Level evaluator identity is
`game-runtime-source-sha256:e325494a2e130290eccd0ee9d87b3d0d27c6ef4dd7d0b612e8a497d1bc5aff25`.
Four Level WASM copies (Steam, Web, Editor, API) are byte-identical: 2,119,887
bytes, SHA-256
`062511bd751bc6e7fb67d0b983c1b6d29b6d7ef97fcd8587f84e77213db96a5d`.
Web and Steam use the same Full browser WASM (1,075,524 bytes,
`9595332c368b837a022b4d9549830748fc50a4d700bbe44fcc09385b5f7a6d60`); API
uses the Full no-import server WASM (1,069,910 bytes,
`a24fbadc3544f3a093bdbf806c365a87103fad387af0cb538462e7e239a49d65`). Both
Full artifacts were rebuilt from the pushed Lang commit with a 67,108,864-byte
linear-memory maximum.

The eight downstream manifests were hashed after the final rebuild:

| Manifest | SHA-256 |
| --- | --- |
| Steam Level | `e1119a52f533ce79707401d5d5bc308ee8515e8552f81f76784234148c955a3f` |
| Web Level | `a02af90cad803018876b43f84084d89b5d7cdf320a281d7f96b2a763f1ff9d3f` |
| Editor Scene vendor | `21f8353e2f8f38368d49ae020d4d2800b58c2093fbda27eac30474629b70ae63` |
| Editor Level provenance | `f3170d9187994a2cd8d747d9705896e6367de3374b2eeb104c92485aeffb6232` |
| API Level | `94257e840e58abeddf09bcd551c5a0adba114033a71fc1b224475c66ea6b9b0c` |
| Steam Full (Runtime API 6) | `6a1e4272bf998ede053028b5640a5201c17a1b13ac6bafbd510cd30b8a397642` |
| Web Full (Runtime API 6) | `6a1e4272bf998ede053028b5640a5201c17a1b13ac6bafbd510cd30b8a397642` |
| API Full (Runtime API 6 / Server ABI 7) | `d63aae49486147d695fea6be34410df2212e40839865055714acd65c78f4db7c` |

Validation passes: Lang locked workspace tests, formatting, and error-registry
catalog checks; all 109 shared Full fixtures in native CLI, Wasmtime 49.0.1,
Node browser/server WASM, and actual Chromium browser WASM; locked downstream
Rust tests and formatting; 95 actual Chromium Worker Scene API 2 executions
with Debug traces; root TypeScript checks and 887 workspace tests including
Workerd smoke; API Worker smoke and dry-run bundle; four application builds;
Steam Full tests (16), Web-to-Workerd browser tests (5), and Editor offline
WASM tests. The downstream worktree remains uncommitted; it was not cleaned or
reset because it contains other user changes. The only untracked Lang file is
`scripts/__pycache__/package_wasm_full.cpython-314.pyc`, outside Cargo and
binding build inputs. No user release or production deployment was created.

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

## 2026-10-21 Empty Folded Block slots

The game workspace pins pushed implementation revision `0336b6bdf5ce0952afe362aea9da199dbe9f4809`. Empty folded cells are skipped without dispatching or changing their physical coordinates. Full browser/server WASM and the four Game Runtime Level consumers have been rebuilt from the pin, with no local Cargo override. Native language workspace tests, VM tests and formatting pass. All 113 shared fixtures pass against native CLI, Wasmtime, Node browser/server WASM and actual Chromium WASM. The game-side synchronization record contains exact artifact identities and the separately reported pre-existing game workspace test/format failures. No user-published release or production deployment was made.

## 2026-10-25 NEG eight-bit complement

The user changes NEG to eight-bit bitwise complement. `$!` / Instruction Code 69 now computes `!value`, equivalently `255 - value`: `0→255`, `170→85`, `5→250`, and `255→0`. The implementation, source/VM contracts, decisions, language assistance, and conformance fixtures are updated. The change does not alter instruction encoding, IR structure, Runtime API v6, or Server ABI v7.

Lang commit `87a283c17abbd0cb05e57c7b6b1221714c43bb94` is pushed. The Game Runtime pins that immutable revision in Cargo.toml and Cargo.lock without a local override. Steam and Web Full browser WASM are identical at 1,076,522 bytes, SHA-256 `49c25703f944599ec67dae5a9b48bb1dcb0b403dcac8d938d45c794716a201f2`. API Full server WASM is 1,070,902 bytes, SHA-256 `1d527d06f524112415cc82de91a98fb3726c0ba14aa0d6e2109ee1c30ae781ae`. All use the 67,108,864-byte memory ceiling.

Steam, Web, Editor, and API Level WASM copies are identical at 2,318,048 bytes, SHA-256 `d8647a0e333d3af37560af63594401f3156ac24fc5248d3e0da14722426ded01`. Evaluator identity: `game-runtime-source-sha256:5e6b1d36ab5945f47cc6d36d15920c0f418469a0adf240016ff374cb63c992be`. The dirty 315-file Rust tree SHA-256 is `8b54eb49e3d14cae2291c6c6ddfa4a180dcf0e0d516096e7a32ca357a2bfb24a`; Cargo.lock SHA-256 is `8d185dcdaa370fb144f3b3c451c215b3369d2d8a7c38b07a6444194b52b792db`. Generated Level JS and declarations are unchanged; Editor/API retain their explicit-byte patches. All consumer manifests and provenance records contain the current pin and hashes.

Verification passed: Lang formatting and workspace tests; 114 shared Full fixtures in native CLI, Wasmtime, Node browser WASM, actual Chromium WASM, and Node server WASM; the focused Game Runtime NEG regression; 101 actual Chromium Level API-2 Worker cases and Debug traces; Web artifact checks (3), Editor WASM integrity (1), API Level evaluation (10), and local Workerd Full/Level smoke; Editor, Web, Steam, and API builds. The Steam Official runtime WASM suite is 11/12 because of an existing folded-return dwell assertion. The Game Runtime library suite has five existing MechanicalArm protocol failures (74/79 pass); workspace rustfmt reports existing differences in scene files; Steam typecheck reports six existing Loom tuple errors and a stale Terminal fixture without `welcomeMessage`. The downstream worktree remains uncommitted and no product deployment was made.