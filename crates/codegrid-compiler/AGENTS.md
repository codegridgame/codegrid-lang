# Module Rules: `codegrid-compiler`

## Responsibility

Provide the single authoritative pipeline for source parsing, diagnostics, name/scope validation, lowering, verified-program construction, canonical read-only program views, and compiler-resolved IR-path source maps with UTF-8 byte spans.

## Allowed dependency and call direction

- This crate may depend on `codegrid-syntax`, `codegrid-hir`, `codegrid-ir`, and `codegrid-model`.
- `codegrid-cli`, `codegrid-lsp`, and `codegrid-runtime-api` call compiler APIs; they must not reimplement source acceptance or validation.
- Lower executable output only through the `codegrid-ir` verifier/builder boundary.

## Prohibited

- Do not infer diagnostic codes from message text; assign them at detection and preserve verifier-origin codes.

- Do not execute programs or depend on `codegrid-vm`, CLI, LSP transport, editor, browser/WASM ABI, filesystem, or game policy.
- Do not fork the specifications, instruction inventory, parser, or symbol rules in a host adapter.
- Do not change normative specifications for implementation convenience.

## Validation

Use source-spec conformance tests, exact diagnostic/span tests, valid `.cg` examples, and verified-IR tests. Run compiler native and supported `wasm32` checks when available.
