# Native CLI

The CLI provides language-level source checking, VM execution and a JSON-lines
debug transport. It delegates source acceptance to the shared compiler and
execution to the shared VM; game scenarios, game limits and level results are
owned by the game runtime.

## Commands

```text
codegrid check <program.cg>
codegrid run <program.cg> --seed <u64> --max-steps <positive-u64> --max-work-units <positive-u64>
  [--input <byte,byte,...> | --input-file <json-file>]
  [--initial-memory-file <json-file>]
codegrid debug --stdio
```

`run` requires `--seed`, `--max-steps` and `--max-work-units` exactly once.
Numbers use canonical unsigned decimal notation. Seed accepts the full `u64`
range; both limits must be positive. Unknown or repeated options, malformed
numbers and simultaneous `--input`/`--input-file` are rejected. Input bytes
are integers from 0 through 255. Initial memory is a JSON array of
`{"address":"<signed decimal>","value":<byte>}` entries; duplicate
addresses are rejected and zero-valued entries are normalized away.

`--max-steps` bounds committed VM transitions in this run. If the VM remains
active at the boundary, the result is `step_limit_reached`. `--max-work-units`
limits deterministic work in this run call. Earlier complete transitions
remain committed when the work boundary is reached; an interrupted transition
is rolled back and the result is `work_limit_reached`. Neither option changes
language semantics.

## JSON run result

For a source file that can be read and submitted to the compiler, `run` writes
one pretty-printed JSON object and one newline to stdout. The schema is
`codegrid.cli.run-result`, version 1. Its top-level fields are `status`,
`yield_reason`, `configuration`, `diagnostics`, `events`,
`newly_emitted_output` and `snapshot`. Normal bounded statuses are
`halted`, `runtime_error`, `vm_fault`, `step_limit_reached` and
`work_limit_reached`; `yield_reason` is `null`.

`configuration` records the effective seed, step and work limits, input bytes
and normalized initial memory. `snapshot` records VM status, `committed_steps`,
registers, sparse memory, remaining input, cumulative output, the mutable
runtime program, threads, a step-count summary, structured errors and an
optional fault. Each runtime error has `transition_id`, `scope`, `code`,
`error_number` and structured `details`; Custom scopes identify an
`internal_step`. Wide integers are canonical decimal strings.

Events describe committed VM effects such as cells reached, input consumed,
register or memory changes, code changes and thread changes. An interrupted
transition emits no events or output. Runtime errors and faults remain distinct
from bounded-run statuses.

## Debug transport

`codegrid debug --stdio` reads and writes one JSON object per line. Debug
protocol 2 accepts a launch with `source`, byte-array `input`, decimal-string
`seed`, positive `max_steps` and positive `max_work_units_per_step`. Unknown
fields are rejected. A `step` request advances one VM transition, `snapshot`
returns current state and `disconnect` ends the process. The session stops with
`debug.step_budget_exceeded` before committing a transition beyond its
cumulative step budget. If one transition exceeds its work-unit budget, it is
rolled back and the transport returns `debug.work_unit_budget_exceeded`.
Compiler diagnostics are returned in a successful launch body and prevent VM
creation.

## Exit codes

| Exit | Meaning |
| --- | --- |
| 0 | Normal halt or successful `check`, help, or debug disconnect/EOF. |
| 2 | Invalid command, option, input or initial-memory data. |
| 3 | Source, input, memory or transport I/O failure. |
| 4 | Source compilation rejected. |
| 5 | VM runtime error or initialization failure. |
| 6 | Step limit reached while the VM remains active. |
| 7 | Work-unit limit reached. |
| 8 | VM fault. |

CLI run-result schema 1 and debug protocol 2 are independent of the Runtime API
v6 and Server ABI v7 transport contracts.
