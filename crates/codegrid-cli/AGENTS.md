# Module Rules: `codegrid-cli`

## Responsibility

Provide the native `codegrid check`, `codegrid run`, `codegrid evaluate`, and `codegrid debug --stdio` interfaces, including command-line parsing, file/input I/O, exit codes, and human or JSON presentation. The debug transport owns process lifecycle and UTF-8-to-UTF-16 source location conversion, while each step delegates to the same bounded VM Global Tick transition. The level command delegates level loading, compilation, validation, evaluation, metrics, constraints, and scoring to `codegrid-level-api`.

## Allowed dependency and call direction

- Compose `codegrid-compiler`, `codegrid-vm`, and the representation APIs from `codegrid-ir`, `codegrid-model`, and `codegrid-syntax`.
- The `evaluate` command composes `codegrid-level-api` with CLI-owned file access, option parsing, display, and exit-code mapping.
- Delegate source acceptance to the compiler and execution to the VM; convert inputs/configuration/results at the process boundary.

## Prohibited

- Do not strip shared diagnostic/runtime codes. Native debug protocol 2 uses `{code,message}` errors; process failures include a bracketed stable code on stderr.

- Do not add a second parser, validator, instruction inventory, or interpreter.
- Do not put filesystem/process behavior into semantic crates or game scoring/policy into CLI output contracts.
- Do not report a normal halt when a requested execution budget is exhausted or a runtime error occurred.

## Validation

Test argument errors, file and UTF-8 failures, both input paths and explicit empty input, exit behavior, JSON stability, diagnostics, runtime failures, and end-to-end determinism.

`evaluate --api-version 2` selects only the shared v2 API/profile loader. The
default remains version 1. Do not detect scene kinds or interpret level rules
in the CLI; all author acceptance and results remain API-owned.
