# Gas, Size and Custom implementation record

Date: 2026-10-11. Scope: Rust language execution, level evaluation and existing
host consumers. Source base: `4f9068d20285594a36225744942d1ccf8524047e`, dirty
working tree. This is an implementation record, not a published release.

## Implemented contract

- Tick remains the deterministic outer transition count. Step and bounded Run
  use the same transition, including work-budget interruption.
- Gas schedule 1 charges executed operations, distinct memory locations and
  aggregate stack peaks. All arithmetic is checked. A positive trusted Gas
  ceiling applies per runtime instance/test; the default is 100,000,000.
- Gas limit failure rolls back the attempted Tick and its output/events while
  retaining representable attempted Gas. Overflow preserves the preceding exact
  representable Gas ledger and returns a structured error. Failed runs do not
  receive a successful score.
- Size counts nonempty cells, explicit Entry, every Folded Block reference and
  its nonempty definition body once. Unused Functions count; implicit starts
  add no cell. There is no invented fixed/locked-cell mechanism.
- Current authoring/scoring contracts use `gas_used`, `size`, `max_gas` and
  `max_size`. Gas breakdowns are observable metrics, not scoring dimensions.
  Visible-case Gas is summed; static Size is measured once.
- Production Custom dispatch returns `CustomDisabled` (3013). Parsing,
  validation and retained implementation remain present. Unexecuted definitions
  and skipped conditional uses remain accepted. No host can enable execution.
- Errors `GasLimitExceeded` (3011) and `GasCounterOverflow` (3012) retain stable
  structured identities across native, runtime API and WASM projections.
- Host Gas quantities use exact decimal strings at JavaScript boundaries;
  schedule identity is numeric 1. Current contracts were updated directly.

The complete operation table and resolution are recorded in
[decisions](decisions.md), the [VM specification](../spec/codegrid-vm-spec.md)
and [independent arithmetic checks](gas-schedule-validation.md).

## Downstream synchronization

`C:/source/bf-steam-wt` now has matching authoring DTOs, trusted scene profiles,
Full browser/server requests, metric readers and Custom error presentation.
Web, API, Steam selection and editor WASM artifacts were rebuilt and copied
with actual hashes and source dirty-state provenance. The editor's explicit
local-byte loader patch is preserved and verified against original binding
bytes. The separate TypeScript game interpreter remains outside this task.

Evaluator identity:
`codegrid-level-source-sha256:8f640c94ee728ab6656c94c6e132b3f88ad7a228528deca323e7d456ef3db900`.
No star-map UI, solution repository, production deployment or Git publication
was performed.

## Verification

- `cargo check --workspace --tests`, `cargo test --workspace --no-fail-fast`,
  `cargo fmt --all -- --check`, and the error-registry validator passed.
- Four release WASM modules were built with locked dependencies and a 64 MiB
  linear-memory ceiling; generated bindings use pinned wasm-bindgen 0.2.129.
- All 107 Full fixtures match complete native CLI results in actual browser,
  Node and Wasmtime 49.0.1 hosts. Lifecycle, quotas and work interruption passed.
- All 58 scene results agree across native, browser Worker, Node and Wasmtime;
  Debug traces agree across the three WASM hosts.
- Local Workerd passed 97 HTTP-compatible fixtures; ten require controls that
  the endpoint does not expose. Browser-to-Workerd integration passed five
  tests, including CustomDisabled and exact Gas fields. Worker dry-run bundling
  passed without deployment.
- Downstream API-contract tests passed 21, focused Web host tests passed 30,
  and editor tests passed 68, including artifact integrity. Steam production
  build and sandboxed Electron offline scene evaluation passed.
- Editor and Web builds passed. Focused Rust playtest E2E passed five tests
  across desktop/mobile/compact/tablet layouts, with three intentional skips.
- Downstream whole-workspace typechecking remains unsuccessful in existing
  Steam/terminal TypeScript game consumers: obsolete directional READ and
  `#`/`?` tokens and missing `GridCell.prefix`. These unrelated dirty-tree
  changes were preserved. This prevents claiming the complete synchronization
  gate is clean, despite passing Rust/WASM execution paths.
- A broad editor E2E attempt was interrupted after existing AI/Loom fixtures
  failed because their immediate-output programs violate current permissions.
  That suite is not reported as passing; focused Rust playtest checks passed
  as recorded above.

Production Steamworks, installers, cloud deployment and physical hardware
acceptance have not been verified.
