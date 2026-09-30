# Module Rules: `codegrid-syntax`

## Responsibility

Own source text representation, lexing, syntax parsing, source spans, line indexing, and conservative source formatting.

Assign stable diagnostic/helper error codes at their detection site, following
`spec/codegrid-error-codes.md`. Codes must not depend on English message wording.

## Allowed dependency and call direction

- This crate may depend only on `codegrid-model` among workspace crates.
- `codegrid-hir`, `codegrid-compiler`, `codegrid-cli`, and `codegrid-lsp` may call its public APIs.
- Preserve UTF-8 byte offsets in core spans; editor protocols convert them at their boundary.

## Prohibited

- Do not perform semantic name resolution, full static validation, IR lowering, or VM execution; those belong to HIR/compiler/IR/VM layers.
- Do not read files or depend on CLI, LSP transport, VS Code, browser, JavaScript, WASM runtime, or game APIs.
- Do not silently normalize or discard source details needed by diagnostics or formatting.

## Validation

Test tokenization, incomplete/malformed input recovery, spans (including UTF-8 and CRLF), and formatter stability. Run native and supported `wasm32` checks when available.
