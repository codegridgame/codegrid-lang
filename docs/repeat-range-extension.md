# Repeat suffix range extension

The 2026-10-12 user decision extends current Repeat suffixes to `x2`–`x9`.
Each suffix executes its Primary once per VM transition for the stated total count.
Existing prefix reevaluation, cancellation, movement, conflicts, and rollback
remain unchanged. CALL/RETURN still reject Repeat; immediate output accepts it.
Counts `x0`, `x1`, `x10`, and non-ASCII digits remain invalid.

The normative contracts are [source syntax](../spec/codegrid-source-spec.md)
and [VM execution](../spec/codegrid-vm-spec.md). The explicit decision is recorded
in [decisions](decisions.md#repeat-suffix-range-extension-2026-10-12).

## Downstream synchronization

Game selection, editor presentation, level artifacts, and application
verification for this language change are recorded in the
[Game Runtime history index](../../bf-steam-wt/doc/codegrid-lang-history/README.md).

## Verification

Passed:

- Rust model, syntax, IR, compiler, VM, and LSP tests; focused crate formatting.
- Error registry validation: 247 scoped identifiers and 445 emitted references.
- 108 Full fixtures in native CLI, Node browser/server WASM, actual browser
  WASM, and Wasmtime, including `repeat-x9-executes-once-per-transition`.
- VS Code compilation and 107 focused grammar, metadata, formatter, and
  provider tests in an isolated editor session.
- Both repositories' whitespace diff checks.

Verification limits: the full VS Code suite exposed a residual mock LSP
completion (`#lsp-only`) in the standalone-provider test. Two old negative
`x6` expectations were corrected; all affected suites then passed in isolation.
Game-specific test results, formatting, builds, and artifact provenance are
recorded by the Rust Game Runtime repository.
