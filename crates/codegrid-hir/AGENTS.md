# Module Rules: `codegrid-hir`

## Responsibility

Represent resolved source-level structure and symbol relationships while preserving canonical source spans.

## Allowed dependency and call direction

- This crate may depend on `codegrid-model` and `codegrid-syntax` only.
- `codegrid-compiler` may build and consume HIR; downstream hosts consume compiler APIs rather than implementing their own name-resolution pass.

## Prohibited

- Do not depend on IR execution, the VM, CLI, LSP transport, editor APIs, browser/WASM bindings, filesystem, or game policy.
- Do not introduce a second source parser or diverge from syntax crate spans and tokens.
- Do not encode runtime state or host-specific handles in HIR.

## Validation

Test symbol identity, scope rules, source-span preservation, and malformed/incomplete input behavior through the owning compiler tests.
