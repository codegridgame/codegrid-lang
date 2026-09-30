# Module Rules: `codegrid-level-api`

## Responsibility

Provide Level Host API v1, shared exact JSON projection, trusted resource
profiles, source compilation, checked handles, and evaluation lifecycle.

## Allowed dependencies

- Depend on `codegrid-level-core` for all logical validation, evaluation,
  metrics, constraints, and rating.
- Depend on `codegrid-compiler` for source compilation and diagnostics.
- Use `codegrid-ir` and `codegrid-model` for foundational verified data types.
- CLI and level WASM adapters call this crate and perform only host conversion.
- The build script may read repository sources solely to embed a deterministic
  fingerprint. Runtime evaluation must remain independent of filesystem I/O.

## Prohibited

- No parser, instruction inventory, VM interpreter, metric or rating duplicate.
- No filesystem, process, clock, host randomness, WASI, or browser dependencies.
- No unchecked IR, hidden data projections, stale/cross-instance handles,
  lossy wide integers, or truncated apparent-success responses.
- Profiles are immutable host configuration; level/source data cannot relax them.

## Verification

Test profile/request validation, exact integers, source/level rejection,
isolation, release/shutdown/cancellation, bounded responses, and shared results.
