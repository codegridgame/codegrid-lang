# CodeGrid for Visual Studio Code

## English

Standalone syntax highlighting, conservative formatting, and basic editor
assistance for the Full CodeGrid source language in `.cg` files. These editor
features work without Rust, the CodeGrid CLI, or a language server.

### Interface languages

The extension follows the VS Code display language. Supported languages are
English, German, French, Spanish (Spain), Traditional Chinese, Japanese,
Simplified Chinese, Korean, Portuguese (Brazil), and Russian. Select a language
with VS Code's **Configure Display Language** command and restart VS Code.
Unsupported display languages fall back to English.

Commands, settings, templates, status messages, standalone completion and hover
help, editor-owned execution messages, and debugger scopes are localized.
CodeGrid instruction spellings, instruction names, error identifiers, watch
paths, and serialized VM state remain stable. Source diagnostics, native debug
failures, VM exceptions and editor errors display stable identities and localized
messages. Original diagnostics remain available as related information; native
exception details remain available in the debugger. LSP-provided instruction
help and operating-system details retain their original text. The language core
does not depend on editor locale.

### Run and Debug

Open a `.cg` file and use the Run or Debug button in the editor title bar,
the editor context menu, or `CodeGrid: Run Current File` / `CodeGrid: Debug
Current File` in the Command Palette. F5 starts debugging; Ctrl+F5 runs
without debugging. A launch configuration is optional for the current file.
The native runtime compiles the current in-memory document, including unsaved
edits. Untitled documents must first be saved. Restart a session after editing
its source; locations refer to the source captured when that session launched.

The Windows x64 installation package includes the native runtime. Other
platforms can set `codegrid.runtime.path` to a matching `codegrid` executable
built from this repository. An empty path selects the bundled executable and
then PATH. The language server is not required for execution. Execution
requires a trusted workspace.

- Run writes output bytes and termination information to the Debug Console.
- Debug stops on entry by default and supports continue, pause, stop, and
  single-step. Step Over and Step Into both advance one atomic Global Tick
  across every live Outer thread. Step Out continues until the selected
  thread returns to a shallower Function call stack, a breakpoint is reached,
  or the VM terminates or hits a configured limit.
- Set line breakpoints on grid rows. A line breakpoint binds to its first
  cell. Use an inline/column breakpoint to select another cell on that row.
  Main, Outer Function, and their Folded Block cells support breakpoints.
  Custom execution completes atomically within its caller's Global Tick;
  internal Custom cells cannot be paused independently and their breakpoints
  remain unverified. Inspect their committed events in Last Tick Events.
- The Call Stack shows Outer threads, their current cell, and Function frames.
- Variables show registers, selected thread state (including Page, direction,
  pointer, stacks, phase and PRNG state), sparse memory, input/output, metrics,
  errors and the previous tick's events.
- Watch and Debug Console evaluation accept read-only state paths such as
  `registers[0]`, `thread.data_stack`, `memory`, or `metrics.global_tick`.
  They do not execute expressions or mutate VM state.
- Source errors prevent execution. Runtime errors and tick/work limits stop a
  debug session with state available for inspection. Stop closes the process.

Configure defaults through `codegrid.execution.*` settings or use a launch
configuration for a particular program:

```json
{
  "version": "0.2.0",
  "configurations": [{
    "type": "codegrid",
    "request": "launch",
    "name": "Debug CodeGrid",
    "program": "${file}",
    "stopOnEntry": true,
    "input": [65],
    "boundary": "exit",
    "seed": "0",
    "customLimit": "10000",
    "maxTicks": "100000",
    "maxWorkUnits": "1000000"
  }]
}
```

Seed and limits use canonical decimal strings to preserve the entire `u64`
range. `maxWorkUnits` bounds each atomic Global Tick; `maxTicks` bounds the
session. These are local development safeguards, not a production sandbox.
Reverse execution, conditional breakpoints, data breakpoints and arbitrary
expression evaluation are not implemented.

### Features

- Registers `.cg` files as the `codegrid` language and supplies a file icon.
- Highlights the Full Primary inventory, Entry and Empty cells, ReadCode,
  WriteCode, and Repeat suffix attachments, conditional prefixes `?0`–`?2`,
  CMP `?=`, random direction `??`, Main/Custom/Function/Folded Block paths,
  named `@end` paths, dimensions, comments, and malformed lexical tokens.
- Completes Full Primary and cell spellings, grammar-level attachment forms,
  structural directive paths, and named `@end` shapes. Standalone suggestions
  are static: they do not resolve the current definition scope, match a closing
  name to the open block, check board-context restrictions, or verify that a
  referenced Function, Custom CodeGrid, or Folded Block exists.
- Provides English hover descriptions for known instruction metadata,
  directives, Entry markers, and incomplete multi-character prefixes.
- Formats complete cell rows conservatively. It preserves comments, cell
  order, row shape, CRLF or LF, and whether the file ends with a newline. The
  formatter does not perform source-level static validation and declines
  structure forms it cannot preserve safely.
- Provides `CodeGrid: New CodeGrid File` with a Main Board template and a
  minimal one-row template.
- Provides `CodeGrid: Learn about CodeGrid 0.1` and a language status item.

The formatter is a presentation helper. It does not validate dimensions,
Entry counts, definition scope, or instruction placement, and it leaves source
unchanged when it cannot safely understand a construct.

### Optional native language server

Set `codegrid.languageServer.enabled` to `true` and set
`codegrid.languageServer.path` to a built `codegrid-lsp` executable. The LSP
adds compiler-backed diagnostics, structural completion and hover, document
symbols, and source formatting for current unsaved documents. The VS Code
Outline shows source definitions. The server is off by default; when it cannot
start or exits, the extension restores its standalone providers.

Build the server from the repository root with:

```powershell
cargo build --release -p codegrid-lsp
```

Then set the executable path to `target/release/codegrid-lsp` or
`target/release/codegrid-lsp.exe`. The extension does not download or install
the server.

### Development and tests

```powershell
npm install        # once
npm run compile    # type-check and build to out/
npm test           # run integration tests in an Extension Development Host
npm run package    # produce a .vsix
```

For a Windows x64 VSIX that includes the current native execution runtime,
install dependencies first and run from the repository root:

```powershell
.\scripts\package_vscode.ps1
```

The package is written to `target/packages/codegrid-vscode-0.3.0-win32-x64.vsix`.
`npm run package` alone packages files already present and does not rebuild the
native executable. Packaging scripts must use a target that matches the native
binary they include.

The Extension Development Host tests cover language registration, Full
TextMate scopes, token and attachment completions, static structural and
named-close suggestions, hover, formatting preservation and idempotence, CRLF
and final-newline handling, and optional LSP fallback. When
`CODEGRID_LSP_TEST_SERVER` points to a built server and a CLI executable is
available beside it (or through `CODEGRID_CLI_TEST_BIN`), the suite also checks
real LSP completion, hover, source symbols, and diagnostic parity with
`codegrid check`.

With `CODEGRID_CLI_TEST_BIN`, the suite also exercises the actual native
debug transport: entry and tick stepping, source positions, column breakpoints,
Function step-out, Folded Blocks, simultaneous threads, watches and variables,
input/output, pause/stop, execution ceilings, source errors, and a real VS Code
debug-session launch. Tests that need the native executable are skipped when
that environment variable is absent.

### Source of truth

The Full source and VM specifications define accepted `.cg` syntax and
instruction behavior. This extension contains presentation metadata and
lexical editing helpers; the Rust compiler and language service remain the
authorities for validation and name resolution.

### License

BSD 3-Clause. See the LICENSE file shipped with the extension.

### Stable error identifiers

Source errors show the compiler code in Problems and the Debug Console. Runtime errors retain their VM code. Debug/transport/editor failures show a bracketed stable category; native debug protocol 2 uses structured code/message errors. The installation package includes the [complete error specification](runtime/codegrid-error-codes.md) and its machine-readable registry beside the bundled runtime. The authoritative repository files are `spec/codegrid-error-codes.md` and `spec/codegrid-error-codes.json`. Seed, limits, source acceptance and VM behavior are unchanged.

Current Full syntax includes `,` READ and `?!` for F=1. READ never changes
direction. The debugger exposes active F in the Status flag scope and
`thread.status_flag`; suspended Function frames show saved caller F.
