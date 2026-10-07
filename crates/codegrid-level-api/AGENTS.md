# Module Rules: `codegrid-level-api`

## Responsibility

Provide Level Host API v1 and the independent native v2 scene lifecycle, shared exact JSON projection, trusted resource
profiles, source compilation, checked handles, and evaluation lifecycle.
The strict v2 profile loader converts trusted scene ceilings to core limits;
it does not by itself register executable capabilities. `LevelApiV2` owns
version-2 request dispatch and registers the three executable native scenes;
browser/server transport support and actual WASM parity require separate evidence.
The v2 result projection accepts only the privacy-safe multi-case result and
reuses common result fields and core scene failure serialization. It must never
project raw case-session results or infer failures from message text.
Re-export the shared model error translation helpers for host presentation.
Translations must not replace structured outcomes or expose hidden test data.

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

Scene feedback responses copy the core's retained event encodings after reserving
the complete envelope. Do not build a second serialized event representation or
confirm delivery before response capacity/allocation succeeds.
