# Deprecated feature and compatibility cleanup audit

Audit date: 2026-10-08. Status: cleanup executed for confirmed targets and API consolidation; F/READ is now implemented in a separate authorized language change. Scope: the current dirty working trees in `C:/source/codegrid-lang` and `C:/source/bf-steam-wt`; upstream base commit `0e9e22b7607d3145ee36d1f9587e8d0cd6f73681`. The initial audit was documentation only. The execution record below describes the subsequent authorized cleanup. User data was not deleted. Findings come from source inspection and reference searches, not execution or coverage measurement.

The [release policy](../AGENTS.md#release-and-compatibility-policy) governs this work. There is no user-published release, so historical compatibility is not a reason to retain implementation. Remove obsolete paths while updating their current consumers. Lack of a compatibility obligation does not make every old-looking name dead code or authorize deleting user data.

## Execution record

The obsolete IF_ZERO capability acceptance and field-free Full snapshot fallback
are removed downstream. The current response contract requires private/saved
register fields. MVP inventory/configuration helpers are removed; VM metrics now
exercise the canonical Full inventory. Active VM state tests moved to
src/state_tests.rs and basic_execution.rs with coverage preserved.

Only LevelApiV2/SafetyProfileV2 and SceneLevelSession remain public. API/profile
1 sessions and CLI/portable version-dispatch wrappers are removed. Shared
validation and result projection live in internal common.rs and CommonLimits;
these are current primitives, not historical readers. CLI defaults to API 2 and
rejects explicit API 1. level_abi_version returns 2 with no v2 alias export.

The 41 ExactIO observations were converted into current scene fixtures and
combined with 15 existing cases, preserving 56 complete native/WASM comparisons.
Current Level Core evaluator/schema helpers remain because they implement shared
logical validation, scoring and scene configuration; they are not deleted solely
for an old filename. Historical API host drivers were replaced with current
drivers, retaining live-buffer/limit checks.

The old MVP binary/fixture directories and importers are removed downstream.
Seven M0 draft TOMLs and their protocol-only index are removed upstream; active
Full coverage remains. No current Full artifacts were deleted. They still emit
the fields required by the tightened HTTP reader and keep truthful original
provenance. This cleanup does not change Full instruction execution semantics.

Historical series conversion is removed. Steam sends the current LevelSeries
through codegrid:open-series; the editor validates it and restores selection.
normalizeCustomLevelData retains current defaults and all create/edit callers;
the misleading migration name is removed without deleting current normalization.
Existing local storage is neither rewritten nor wiped.

The editor Scene WASM/binding and provenance are rebuilt and synchronized,
preserving explicit local-byte initialization and the narrow host declaration.
The recorded source base is 0e9e22b7607d3145ee36d1f9587e8d0cd6f73681 with dirty
source; evaluator identity is
codegrid-level-source-sha256:025dd6daea7d28d46f09de94c0a67204a156354eae95fe5f2647f0177c19d8c0.

F/directionless READ is implemented; see its separate implementation record.
Current server error identities, locale aliases, legality matrices and boundary
validation remain because they serve current behavior, not compatibility.

The additional final-reference review removed the upstream historical MVP
fixture directory and renamed package_wasm_mvp.py to package_wasm_full.py.
CI package names and the distribution guide now describe Full; local packaging
records Runtime API 3 and Server ABI 4. The local 13-payload package was built
under target/cleanup-package for verification; no release was published.

Verification completed: locked Rust workspace tests; formatting and error
registry checks; 56 complete native/Chromium/Node/Wasmtime scene comparisons
and WASM Debug traces; downstream workspace type checks; API-contract 21,
client-application 152, editor 67, Web 63 and API 40 unit tests; editor/Steam
builds; editor playtest E2E (5 passed, 3 existing viewport duplicates skipped);
Steam studio navigation/selection E2E (1 passed); packaged offline Electron
scene evaluation/persistence; actual Workerd comparison (73 endpoint-compatible
Full fixtures, eight require controls absent from that endpoint); and four
browser-to-Workerd integration tests. Existing physical-device/Steamworks and
production deployment limits remain outside these checks.

## Initial inspected cleanup targets

| Target and inspected evidence | Recommended change | Coupled work |
| --- | --- | --- |
| Downstream `packages/api-contract/src/level-generation.ts`: `legacyInstructions` includes IF_ZERO and IF_ZERO_UP/DOWN/LEFT/RIGHT, and the rules validator unions it with AUTHORING_INSTRUCTIONS. Its authoring prompt already calls IF_ZERO obsolete. | Delete acceptance of the five obsolete IF_ZERO capabilities. Do not delete MOVE, OUTPUT or HALT solely because they appear in the same array: they remain current explicit/default permissions. Replace the mixed legacy array with the exact current vocabulary. | Update generator/contract tests and prompts; compare downstream permission lists with shared Rust validation. Retain rejection tests for unknown capabilities. |
| Downstream `packages/api-contract/src/parse-codegrid.ts`, thread validation around lines 281–291: private_registers and saved frame fields are accepted only when present. | Remove field-free historical response acceptance. Require the exact current Runtime API projection, including nullable private_registers and required saved_registers/saved_register_pointer on frames where defined. | Verify actual Rust/server projections, HTTP DTO handling and browser/Worker fixtures together. Null and absent are distinct. |
| Upstream `crates/codegrid-model/src/lib.rs`: MVP_SOURCE_TOKENS explicitly describes a temporary earlier inventory; is_mvp searches it. `crates/codegrid-vm/src/config.rs`: VmConfig::mvp supplies inert old defaults. Searches found consumers in VM metrics unit tests and mvp_execution.rs. | Remove the temporary MVP inventory/classifier/configuration helper after converting remaining callers to the canonical Full inventory and explicit VmConfig::new. | Preserve useful arithmetic/movement/metric regression cases under Full terminology. Do not introduce another instruction table. Rerun reference search before removal; this audit is not a proof of all possible external callers. |
| Upstream `crates/codegrid-vm/src/state.rs` includes `tests/historical/state_tests_full_v2.rs` under cfg(test); the suite includes a test named if_zero_keeps_direction_for_a_nonzero_register that now constructs Direction with ConditionPrefix::Zero. | Rename/reorganize the live suite and stale test names around current conditional-prefix behavior. Delete obsolete expectations only after checking their bodies. | This is active test coverage, not an unused historical file. Preserve current semantics, rollback and concurrency cases. |
| Downstream `apps/api/src/codegrid/0.1.0/` contains old WASM and vendor manifest. The adjacent README describes it as historical and says both Worker entries use the Full module. | Remove the historical artifact directory and provenance-only MVP fixture/importer chain if a complete import/build/script search confirms no current consumer. | Update manifests, README and smoke/import scripts. Keep current full-v3 artifacts; verify Worker uses those bytes. README text alone does not prove no imports anywhere. |
| Seven upstream `tests/fixtures/CG-M0-*.toml` files: tests/README.md says embedded source is superseded draft syntax and not valid .cg. No CG-M0/index.toml references were returned by the inspected Rust/script/tool search. | Remove protocol-only draft fixtures from the active fixture area, or replace any unique useful observations with valid Full conformance cases before deleting the drafts. | Check fixture index, documentation and downstream copies. Do not confuse these drafts with the active conformance-v1.json suite. |

## Legacy Level API consolidation

There are two real execution surfaces, not just obsolete type names:

- `crates/codegrid-level-api/src/lib.rs` exports LevelApi/SafetyProfile and LevelApiV2/SafetyProfileV2, backed by separate session/profile files.
- `crates/codegrid-cli/src/main.rs` contains Legacy/Scene profile and API dispatch; its integration tests still directly instantiate the older API.
- `crates/codegrid-level-wasm-browser/src/lib.rs` exports both LevelSession and SceneLevelSession.
- `crates/codegrid-level-wasm-server/src/lib.rs` dispatches ABI/API 1 and 2 through VersionedApi::Legacy/Scene.
- Downstream `apps/editor/src/scene-playtest.worker.ts` imports SceneLevelSession. The module-local browser rules still explicitly require retaining legacy LevelSession.

Recommendation: consolidate authoring/evaluation on the current Scene API, which includes ExactIO, and remove the historical API/profile/session/browser/server/CLI branch rather than maintaining parallel version support. Before deleting the older path, map its current ExactIO tests, CLI inputs, helper tools, result projections, limits and documentation to the current API. Preserve required current evaluation functionality; removing compatibility must not accidentally remove a current feature.

The implementation task must update module-local AGENTS.md, public exports, safety profiles, ABI dispatch, scripts, fixtures and actual-host checks together. Older schema files and evaluator helpers may contain reusable validation or evaluation logic: trace their call graph before deletion. This audit recommends consolidation but does not claim that moving every caller is already complete.

## Current data normalization versus historical migration

Downstream `packages/client-application/src/editor/series.ts` exports migrateLegacySeries; editor main.ts still calls it to import old local levels, and series.test.ts tests that path. Remove the historical import/reader and its compatibility-only test when consolidating on the current series format. Preserve unrelated existing local user data; removing a reader does not require wiping storage.

Do not delete `migrateCustomLevelData` wholesale. Its inspected body in custom-levels.ts supplies current defaults and normalizes rules, extensions and presentation fields. It is called by CustomLevelController, series reading, and current Steam create/edit flows in app-runtime.ts. Separate any historical acceptance from current normalization, rename the latter appropriately, and update all callers. Optional current authoring fields and default initialization are not automatically backward compatibility.

## Instruction cleanup coupled to the approved F implementation

[Status Flag and directionless READ](status-flag-and-read.md) is implemented. The current instruction is `,` (code 44); directional READ forms, codes and permission names are removed. F and `?!` are implemented and synchronized with current consumers.

That change must remove Read(Direction) syntax/model/IR/code mappings and READ_UP/DOWN/LEFT/RIGHT permissions; update compiler views, VM empty-read direction handling, Custom READ, editor metadata/grammar, fixtures, authoring prompts and all downstream consumers. No old-code reservations or compatibility aliases are required. Current programs that still need exhaustion turns must be rewritten using READ followed by a flag-guarded direction and checked for changed tick/path behavior.

CMP, NEG, NAND, conditional prefixes ?0/?1/?2, Folded Blocks, Functions, Custom execution, and current Page/memory behavior are retained features. The prior proposal's independent POP was a wording mistake, not a current instruction to remove. F test coverage must preserve current valid functionality while adding the approved rules.

## Items that must not be deleted by keyword alone

| Item | Inspection result |
| --- | --- |
| Old IfZero/random source rejection and obsolete-byte tests | Rejection of malformed/unknown current input is valid testing. Remove compatibility-only promises, not the validator or all negative tests. Source/IR searches did not establish a live IfZero interpreter. |
| Primary/Attachment compatibility matrix | This is the current legal combination contract, not historical version compatibility. |
| IR/API/ABI/schema version validation | Validating the current boundary remains necessary. Rejecting an unsupported version is not supporting it. |
| Server error aliases | codegrid-wasm-server/src/lib.rs actively emits unsupported_api_version, source_payload_limit_exceeded, input_payload_limit_exceeded and vm_initialization_error. They can be standardized under the unpublished policy, but require simultaneous registry, transport, UI and test changes; they are not unused mappings. |
| Locale aliases and localized errors | normalize_error_locale aliases provide current language selection, not old release support. |
| Snapshot status consistency and limits | isSnapshotStatusCompatible checks current run/snapshot consistency; byte ceilings, safe integers, UTF encodings, buffer ownership and resource limits remain required. |
| Current versioned artifact directories and fixture filenames | A v3/v1 name alone does not show obsolete content. Current hosts consume Full v3 and conformance-v1.json. Retain until replaced through a real build. |
| Historical decisions and verification reports | Keep factual evidence, but remove active guidance that mandates unpublished compatibility. Do not present old host results as new acceptance. |

## Documentation cleanup

The root policy already supersedes unpublished compatibility requirements, but active guidance still contains stale preservation wording in the error-code specification, module-local rules, the conditional-prefix migration manual, older Level contracts and downstream API README. Update touched active sections during the corresponding cleanup; mark genuinely historical sections clearly instead of silently rewriting past decisions. Do not reserve identifiers or mandate version bumps merely to protect unpublished history.

## Execution order and completion gates

1. Remove the obsolete downstream IF_ZERO acceptance and field-free snapshot fallback, with focused contract tests.
2. Replace MVP inventory/configuration helpers and reorganize active historical-named tests without losing coverage.
3. Consolidate Level APIs and downstream series readers with every current caller accounted for.
4. Remove unreferenced historical artifacts/draft fixtures after checking all scripts, manifests and useful regression observations.
5. Implement F/directionless READ as its separately approved coherent language change when execution is authorized.

For each implementation batch, read module rules, preserve dirty user work, update documentation and downstream consumers, and run focused formatting/build/tests. Rebuild affected WASM and bindings with truthful provenance. Compare actual native/browser/server results for changed execution or host boundaries; perform downstream type checks and relevant editor/Worker tests. Do not implement semantics in TypeScript to bridge a removed Rust path.

The execution record supersedes the initial documentation-only status. The inspected targets are not an exhaustive whole-product dead-code claim; user programs were not deleted. F/directionless READ remains separate from this cleanup.
