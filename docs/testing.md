# Full Language Testing Strategy

Stable error identifiers are checked with `node scripts/check_error_codes.js`. Focused suites assert source/IR codes, CLI and debug protocol 2 errors, LSP/CLI diagnostic parity, and browser/server diagnostic projections. The registry and normative catalog live in `spec/codegrid-error-codes.json` and `spec/codegrid-error-codes.md`. Compare codes and structured fields rather than message wording alone.

Test each behavior at the layer that owns it, then verify the public host boundaries. The Full source and VM specifications define normative acceptance. The active shared suite contains 75 Full execution cases; add direct cases whenever a normative behavior is not yet represented. Historical tests are supplemental evidence unless their expectations are reviewed against the specifications.

## Syntax, compiler, and IR

- Cover Main, Function, Custom, and Folded Block definitions; nested Custom paths; valid closing forms; dimensions; grid geometry; Entry rules; and reference resolution.
- Cover every Full Primary and Attachment, legal combinations, malformed tokens, duplicate or missing definitions, invalid scopes, and unsupported structures.
- Verify UTF-8 spans, comments, LF/CRLF handling, formatting, diagnostics, source-to-symbol mappings, and UTF-8-to-UTF-16 conversion at the LSP boundary.
- Verify complete IR structures, instruction encoding, call/tail-call metadata, version checks, and structured rejection of externally supplied invalid IR.

## VM

- Cover all instructions and Attachments, input/output, registers, pointers, stacks, Page, memory, seeded randomness, Repeat, self-modification, calls, returns, Resume ticks, Folded Blocks, and Custom execution.
- Exercise same-tick concurrency, tick-start reads, deterministic conflict detection, errors competing with HALT, atomic commit/rollback, caller-stack transactions, fresh Custom isolation, and Custom execution limits.
- Verify tail recursion reuses the required frame, Folded Block re-entry/resume rules, deterministic thread/invocation seed derivation, complete snapshots, raw metric sampling, and failed-work metrics.
- Compare bounded `run` with repeated `step` and rerun identical inputs/seeds to prove deterministic results.

## Local hosts

- Native CLI: source diagnostics, all execution settings, input conversion, initial memory, full result serialization, exit codes, runtime errors, and tick limits.
- LSP and VS Code: source lifecycle, full token/structure support, diagnostics, UTF-16 positions, formatting, completion, Hover, and symbol navigation over shared compiler data.
- Runtime API and adapters: version checks, exact wide integers, handle ownership/release, instance isolation, requests and response limits, and complete lifecycle behavior.
- Run one reviewed Full fixture set through the native CLI, an actual browser-WASM host, and an actual server-WASM host. Compare status, thread snapshots, registers/stacks/Page/memory, input/output, ticks, errors, events, and metrics.

## Completion evidence

Use valid Full `.cg` fixtures for accepted programs and diagnostics with expected source spans for rejected programs. Restore historical tests only after reviewing them against the Full contracts. Record exact commands, runtime/tool versions, host environments, and results. A successful `wasm32` build or one-host smoke run is not cross-host parity evidence.

