# Module Rules: `codegrid-lsp`

## Responsibility

Provide a native LSP process and editor-language features over current, possibly unsaved CodeGrid documents.

## Allowed dependency and call direction

- This crate may depend on `codegrid-compiler`, `codegrid-syntax`, and `codegrid-model`.
- Use compiler diagnostics and symbol identities for semantic features; use syntax APIs for spans, token context, and formatting.
- The VS Code extension may launch this process as an optional editor service.

## Prohibited

- Do not omit shared diagnostic codes or derive them from messages. JSON-RPC protocol errors retain their standard numeric categories.

- Do not depend on or start the VM for static diagnostics, completion, hover, definitions, references, symbols, or formatting.
- Do not duplicate compiler validation, instruction inventories, symbol resolution, or formatting rules.
- Do not put logs on protocol stdout; convert UTF-8 byte spans to UTF-16 positions explicitly and do not serve stale document versions.
- Do not make the standalone VS Code 0.1 experience depend on Rust or this process.

## Validation

Test JSON-RPC framing/lifecycle, ordered incremental edits, document versions, UTF-16 conversion (including non-ASCII and CRLF), every advertised feature, and diagnostics parity with `codegrid check`.
