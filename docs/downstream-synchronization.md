# Mandatory downstream synchronization

This document is a required completion gate for AI-assisted changes in
`C:/source/codegrid-lang`. The downstream application repository is
`C:/source/bf-steam-wt`. This requirement was requested on 2026-10-06 because
language and scene mechanism changes were not reaching application consumers.

## Every-change requirement

The [release policy](../AGENTS.md#release-and-compatibility-policy) takes
precedence over legacy-preservation requirements below. No user-published
release exists as of 2026-10-08. Synchronize current contracts and consumers
directly; do not add compatibility readers, retain old artifacts, reserve old
codes, or require migrations/version bumps for unpublished versions. Continue
to preserve unrelated user data and record truthful artifact provenance.

For every change, inspect its downstream impact before reporting completion.
Changes to syntax, instruction permissions, compilation, VM behavior, scenes,
level JSON, errors, metrics, limits, scoring, or public API/WASM contracts must
include the corresponding downstream updates in the same task. Do not wait for
a separate user reminder or leave synchronization as an optional follow-up.

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
   and module rules where their behavior or responsibilities changed. Preserve
   explicit compatibility readers for old saved data; do not silently rewrite
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
| Scene evaluator, Scene Host API, level schema, scoring, debug events | `apps/editor/src/scene-wasm/`, editor worker/playtest adapters, `packages/client-application/`, `packages/api-contract/`, authoring fixtures and AI generation |
| Full browser Runtime API and language behavior | `apps/web/public/codegrid-full/v3/`, Web runtime host, transport adapters, browser integration tests |
| Full server ABI and Runtime API | `apps/api/src/codegrid/full-v3/`, Worker runtime host, `packages/api-contract/`, Worker smoke and conformance fixtures |
| Instructions, conditions, permissions, error identities | Authoring DTOs, validation, templates, prompts, examples, diagnostic presentation and compatibility readers |
| Shared game or terminal contracts | `packages/game-core/`, `packages/game-language/`, `packages/game-content/`, `apps/terminal/` and their tests; inspect actual dependencies before changing these separate execution paths |

The inventory reflects the 2026-10-06 synchronization. Discover current paths
and versions before each future update; do not assume the listed versioned
directories remain the active consumers.

### Artifact-specific requirements

- Editor assets come from `codegrid-level-wasm-browser` and the generated level
  build provenance. Preserve the host's explicit local WASM-byte initialization
  and Steam offline `file://` support. Its generated JavaScript has a deliberate
  default-loader patch: retain upstream provenance and record the patched file
  hash separately in `editor-vendor-manifest.json`. Verify both identities.
- Web Full assets come from `codegrid-wasm-browser`; server Full assets come from
  `codegrid-wasm-server`. Update host constructor arguments, requests, versions,
  result/status/error shapes, quotas, and projections whenever contracts change.
  Retain old versioned artifacts only as deliberate compatibility history.
- Keep source commit/dirty-state identity and fixture hashes truthful. Regenerate
  provenance through the repository's build tools. API/ABI versions and level
  `format_version` are distinct and must come from their respective contracts.
- Keep downstream strict TypeScript rules. Generated declarations must not
  introduce prohibited `any`; maintain the existing narrow host declarations.

## Verification and completion

Select checks according to the affected consumer. The downstream root provides
`npm run typecheck`, `npm run build:editor`, `npm run test:e2e:editor`,
`npm run build:web`, and `npm run build:steam`. Inspect current workspace scripts
for editor artifact integrity, Web browser/server parity, actual Worker
conformance, and editor offline Electron acceptance. Upstream scene WASM checks
are available through `scripts/test-scene-wasm.ps1`; inspect current language
adapter smoke scripts for native, browser, and server comparisons.

Do not claim browser tests establish real Steamworks, physical controller,
installer, audio, or cloud acceptance. Do not deploy, enable production runtime
features, commit, or push merely to satisfy this synchronization rule; follow the
user's authorization and repository rules for those actions.

If a repository, toolchain, required host, or check is unavailable, report
**downstream synchronization incomplete**, naming the affected files, blocker,
completed work, and remaining verification. Do not silently skip the consumer
or report the overall mechanism change as fully complete.
