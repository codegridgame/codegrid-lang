# Repeat suffix range extension

The 2026-10-12 user decision extends current Repeat suffixes to `x2`–`x9`.
Each suffix executes its Primary once per tick for the stated total count.
Existing prefix reevaluation, cancellation, movement, conflicts, and rollback
remain unchanged. CALL/RETURN still reject Repeat; immediate output accepts it.
Counts `x0`, `x1`, `x10`, and non-ASCII digits remain invalid.

The normative contracts are [source syntax](../spec/codegrid-source-spec.md)
and [VM execution](../spec/codegrid-vm-spec.md). The explicit decision is recorded
in [decisions](decisions.md#repeat-suffix-range-extension-2026-10-12).

## Downstream synchronization

Source base: `b9a71a0d1d2c497f903e6f33c038f4281273d75c`, with
`source_dirty=true`. Existing unrelated changes in both repositories were
preserved. WASM builds use the pinned toolchain and a 67,108,864-byte linear
memory maximum. No format/API version changes or compatibility readers were added.

Updated `C:/source/bf-steam-wt` consumers:

- Selection suffix inventory and applicability, fixed-cell transport types and
  acceptance, editor Repeat presentation, focused authoring tests, and instruction
  reference documentation.
- Web Full and Steam selection browser WASM sets; API Full server WASM and
  complete 108-case fixture suite; actual hashes and dirty source identities.
- Editor Scene WASM and provenance. Generated JavaScript and snippets are
  synchronized completely; unchanged binding bytes retain their actual hashes.
  The editor's explicit-local-byte loader patch is preserved and verified.

Evaluator identity:
`codegrid-level-source-sha256:b5be1e478c920fc8518dc59e7fcc94e1866333f57bf027f614968eb46b140f3d`.
The independent TypeScript game/terminal instruction inventories contain no
Repeat suffix implementation or `x2`–`x5` acceptance rule to change.

## Verification

Passed:

- Rust model, syntax, IR, compiler, VM, and LSP tests; focused crate formatting.
- Error registry validation: 247 scoped identifiers and 445 emitted references.
- 108 Full fixtures in native CLI, Node browser/server WASM, actual browser
  WASM, and Wasmtime, including `repeat-x9-executes-once-per-tick`.
- 63 scene results in native CLI and Node portable WASM; actual browser scene
  executions and Debug traces.
- VS Code compilation and 107 focused grammar, metadata, formatter, and
  provider tests in an isolated editor session.
- Downstream whole-workspace type checks; client-application 228, api-contract
  27, editor 76, Web 62, API 45, and Steam 269 unit tests.
- Local Workerd: 98 endpoint-compatible Full fixtures; ten require controls
  outside the HTTP endpoint.
- Editor, Web, and Steam builds; packaged offline Electron editor loading,
  bundled WASM evaluation, and persistence.
- Both repositories' whitespace diff checks.

Verification limits: the full VS Code suite exposed a residual mock LSP
completion (`#lsp-only`) in the standalone-provider test. Two old negative
`x6` expectations were corrected; all affected suites then passed in isolation.
Whole-workspace Rust formatting reports pre-existing unformatted level-core
work; those unrelated edits were preserved. No production deployment or
physical Steamworks/device acceptance is claimed.
