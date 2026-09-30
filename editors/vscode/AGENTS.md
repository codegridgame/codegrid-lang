# Module Rules: VS Code CodeGrid Extension

## Responsibility

Provide CodeGrid file association, TextMate syntax highlighting, language configuration, conservative formatting, standalone editor assistance, and a Debug Adapter Protocol presentation layer. The native Rust LSP is optional. Run and Debug use the native CLI's versioned JSON-lines debug transport; package a current native executable for the target platform or accept an explicit executable path.

## Allowed dependency and call direction

- The extension calls VS Code APIs and may launch `codegrid-lsp` through an explicit user configuration.
- The extension may launch `codegrid debug --stdio` without a shell and project compiler source locations and VM snapshots into DAP threads, stack frames, variables, watches, and breakpoints. Global Tick stepping and all language behavior remain in Rust. Custom execution is atomic within an outer tick; do not advertise independently pausable Custom instructions.
- Keep UI/editor lifecycle in the extension; keep authoritative source validation and symbol resolution in shared Rust compiler/LSP APIs when available.
- Static metadata may describe presentation, snippets, and instruction help, but must stay consistent with the normative specs and canonical instruction inventory.

## Prohibited

- Do not classify failures by English text. Preserve native codes through debug protocol 2 and use stable editor codes for DAP/UI/transport failures.

- Do not implement a TypeScript parser, validator, instruction interpreter, or competing semantic rules.
- Do not require Rust, CLI, or LSP for standalone 0.1 highlighting, formatting, and basic editor assistance.
- Do not silently rewrite code during formatting or claim semantic support that the active language service does not provide.

## Validation

Test grammar/token scopes, formatting idempotence and preservation, provider behavior, activation, and the optional-LSP failure fallback. Keep English as the default package language. Explicitly requested localization catalogs, translated UI, and localized extension introductions may use their target languages. Preserve CodeGrid syntax, configuration keys, protocol fields, and stable error identifiers across locales.
