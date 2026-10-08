# Native CLI Contract

Every emitted toolchain error includes a stable four-digit presentation number
from the [complete error table](../spec/codegrid-error-numbers.md). Human messages
show `[number] [original_code]`; JSON diagnostics/errors/faults add `error_number`.
Existing textual codes and process exit codes retain their meanings.

The native CLI provides a local process boundary for the Full language and level evaluation. It owns command-line parsing, file access, input conversion, JSON serialization, terminal diagnostics, presentation, and process exit codes. The shared compiler owns source acceptance and verified-program construction under the [source specification](../spec/codegrid-source-spec.md); the shared VM owns execution and snapshots under the [VM specification](../spec/codegrid-vm-spec.md). The `check`, `run`, and `debug` commands compose the compiler and VM directly. The `evaluate` command delegates all level semantics to the shared [Scene Level Host API](../spec/codegrid-scene-host-contract-v2.md). No CLI command implements a second parser, validator, instruction table, interpreter, metric calculator, or scoring policy.

This contract describes the local Full target. It does not select deployment authentication, multi-user quotas, or production service policy.

## Commands

```text
codegrid check <program.cg>
codegrid debug --stdio
codegrid run <program.cg> --boundary <exit|wrap> --seed <u64>
    --custom-limit <positive-u64> --max-ticks <positive-u64>
    --max-work-units <positive-u64>
    [--input <byte-list> | --input-file <json-file>]
    [--initial-memory-file <json-file>]
codegrid evaluate <level.json> <program.cg>
    --mode <debug|official> --boundary <exit|wrap> --seed <u64>
    --custom-limit <positive-u64> --limits-file <trusted-profile.json>
    [--format <json|human>]
```

The `run` options `--boundary`, `--seed`, `--custom-limit`, `--max-ticks`, and `--max-work-units` are required exactly once. Numeric option values use canonical unsigned ASCII decimal notation: `0` or a nonzero digit followed by zero or more digits, with no sign, separators, whitespace, or leading zero. `--seed` accepts the inclusive range `0..=u64::MAX`. The other numeric options must be in `1..=u64::MAX`. `--boundary` accepts exactly `exit` or `wrap`, case-sensitively. Unknown options, repeated options, missing values, and incompatible input sources are rejected.

`--input` accepts a comma-separated list of canonical unsigned decimal byte values in `0..=255`; ASCII whitespace around a value is ignored. Empty list elements, including consecutive or trailing commas, are invalid. An explicitly empty value (`--input ""`) supplies an empty sequence. If neither input option is present, the input sequence is empty. `--input` and `--input-file` are mutually exclusive.

`--input-file` contains one UTF-8 JSON array of byte integers, for example `[65, 0, 255]`. The array may be empty. Elements must be nonnegative JSON integer tokens in 0 through 255, without exponent notation; strings, negative values (including `-0`), fractions, booleans, and `null` are invalid. No extra JSON value may follow the array.

`--initial-memory-file` contains one UTF-8 JSON array of objects with exactly the `address` and `value` fields, for example:

```json
[
  {"address":"-1","value":12},
  {"address":"0","value":0}
]
```

An address is a canonical decimal string for an arbitrary-precision signed integer: `0` or an optional `-` followed by a nonzero digit and zero or more decimal digits. A leading `+`, leading zero, `-0`, whitespace within the string, or JSON numeric address is invalid. `value` must be a nonnegative JSON integer token in 0 through 255, without exponent notation. Each object must have exactly one `address` and one `value` field; unknown or duplicate object fields are invalid. Addresses must be unique. Zero-valued entries are valid and semantically equivalent to absent memory; snapshots and the effective configuration omit them. Initial memory applies to the Outer context only. Omitting this option starts with empty memory.

`--max-ticks` bounds this `run` call's outer Global Ticks. `--max-work-units` is a per-call dispatch ceiling: one unit is one scheduled live Outer or Custom thread dispatch, as defined by the VM specification. The VM yields only when another dispatch is required after the ceiling has been consumed; reaching the ceiling exactly while completing or halting does not itself yield.

The CLI does not prescribe host-specific maximum source, file, input, initial-memory, or JSON sizes. Such local safeguards may reject oversized requests with the corresponding host-input or I/O exit code, but must not alter accepted language semantics or return truncated JSON.

## Editor debug transport

`codegrid debug --stdio` serves protocol version 2 as one UTF-8 JSON request
and response per line. Stdout contains only protocol responses. The process
compiles source and advances the shared VM; it contains no separate interpreter.

A `launch` request contains `source` (string), `input` (byte array), `boundary`
(`exit` or `wrap`), and canonical decimal strings `seed`, `custom_limit`,
`max_work_units`, and `max_ticks`. A session accepts one successful launch.
It returns compiler source locations with one-based UTF-16 line/column ranges
and the normal Full VM snapshot. Compilation failures return `diagnostics`.
Locations use compiler IR paths such as `@main[0]`, `@main.F0[1]`, or
`@main.M0[2]`; no host parses `.cg` source to infer these locations.

`{"command":"step"}` advances one atomic Global Tick using
`step_with_work_limit`, returning the snapshot, committed events and newly
emitted output. Custom calls are part of that atomic tick. The tick limit
prevents further dispatch after the configured committed tick count; the work
limit rolls back an interrupted tick under the normative VM rules.
`{"command":"snapshot"}` reads state without execution.
`{"command":"disconnect"}` releases the session and exits normally. EOF
also exits normally. Request lines are limited to 4 MiB, including the newline.

Every response has `debug_protocol_version: 2` and either `body` or an `error`
object containing stable `code` and readable `message`. Invalid requests return a structured error and leave the session
available. DAP breakpoints and presentation are owned by the editor adapter,
not the VM or this transport. This local transport does not implement a
production memory sandbox.

## `check` behavior

`codegrid check <program.cg>` reads UTF-8 source and invokes the shared compiler's complete Full static acceptance. It never executes the VM. A valid source writes `<path>: valid` and a newline to stdout. Source diagnostics are written to stderr in stable source order as `<path>:<line>:<column>: <severity>: [<code>] <message>`; line and column are one-based, and columns count Unicode scalar values. The command does not emit JSON.

## Versioned `run` result

For a source file that can be read and submitted to the compiler, `run` writes exactly one pretty-printed JSON object followed by one newline to stdout. It emits no banner, progress line, or other stdout text. Human-readable diagnostics may also be written to stderr. The result schema is independent from the source language, IR, Runtime API v3, and browser/server transport schemas.

The top-level object has `schema: "codegrid.cli.run-result"` and `schema_version: 1`. Its fields are:

| Field | Meaning |
| --- | --- |
| `schema`, `schema_version` | Fixed schema identifier and version. |
| `status` | `source_error`, `halted`, `runtime_error`, `yielded`, or `vm_fault`. |
| `yield_reason` | `null`, `tick_slice_exhausted`, or `work_unit_budget_exhausted`; non-null only when `status` is `yielded`. |
| `configuration` | Effective boundary mode, seed, Custom limit, tick and work limits, input bytes, and normalized initial Outer memory. |
| `diagnostics` | Compiler diagnostics; empty for successful compilation. Spans use half-open UTF-8 byte offsets. |
| `events` | Ordered VM events from committed ticks in this `run` call. Empty on compile failure. Rolled-back work emits no events. |
| `newly_emitted_output` | Output bytes committed during this `run` call. Empty on compile failure or an interrupted tick with no earlier output. |
| `snapshot` | `null` on source diagnostics; otherwise the complete Full post-call VM snapshot. |

The serialized shape is fixed as follows. Names and tagged enum values use lower snake case, except stable VM error codes and instruction-variety names, which retain their VM-defined spelling.

```json
{
  "schema": "codegrid.cli.run-result",
  "schema_version": 1,
  "status": "halted",
  "yield_reason": null,
  "configuration": {
    "boundary_mode": "exit",
    "seed": "18446744073709551615",
    "custom_execution_limit": "1000",
    "max_ticks": "10000",
    "max_work_units": "100000",
    "input": [65, 0],
    "initial_memory": [{"address":"-1","value":12}]
  },
  "diagnostics": [],
  "events": [],
  "newly_emitted_output": [],
  "snapshot": {
    "status": "halted",
    "committed_ticks": "2",
    "registers": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "memory": [{"address":"-1","value":12}],
    "remaining_input": [],
    "output": [],
    "runtime_program": {
      "main": {"width":1,"height":1,"cells":[],"folded_blocks":{}},
      "functions": {}
    },
    "threads": [],
    "metrics": {
      "global_tick":"2",
      "operation_count":"0",
      "used_cell_count":"0",
      "used_cells":[],
      "used_memory_address_count":"0",
      "used_memory_addresses":[],
      "peak_data_stack_usage":"0",
      "peak_instruction_stack_usage":"0",
      "peak_call_stack_usage":"0",
      "instruction_variety":[]
    },
    "errors": [],
    "fault": null
  }
}
```

The object above illustrates field types and names; its abbreviated arrays are not a valid execution fixture. The normative contents of each field are:

- `configuration` contains `boundary_mode` (`exit` or `wrap`), decimal-string `seed`, `custom_execution_limit`, `max_ticks`, and `max_work_units`, plus the ordered input byte array and effective initial Outer memory sorted by numeric address. The memory list omits zero-valued entries.
- `diagnostics` contains `{code, severity, message, span}` objects. Severity is `error` or `warning`; span is `{start, end}` with decimal-string half-open UTF-8 byte offsets. Code is assigned by the compiler/verifier at the validation origin and follows the error registry. For `source_error`, `events` and `newly_emitted_output` are empty and `snapshot` is `null`.
- `snapshot` contains the VM status (`running`, `halted`, or `error`), decimal-string `committed_ticks`, ten register bytes, normalized sparse Outer memory, remaining input, cumulative output, the mutable Outer `runtime_program`, every Outer thread, cumulative raw metrics, structured runtime errors, and an optional fault.
- `runtime_program` is a `CodeGridView`: `main` and `functions`. Each board contains exact numeric `width` and `height`, row-major `cells` (each with nullable canonical source-token strings `prefix`, `entry`, `primary`, and `attachment`), and `folded_blocks` keyed by numeric Folded Block ID with row token arrays that include any conditional prefix before the Primary. The mutable Outer program view contains no Custom definitions.
- An Outer snapshot thread object contains `code_grid`, `id`, `board`, `position`, `direction`, `register_pointer`, `page`, `data_stack`, `instruction_stack`, `call_stack`, `phase`, and `random_state`. `code_grid` is `{kind:"outer"}`; `board` is `{kind:"main"}` or `{kind:"function", "id":<slot>}`. Position is `{x,y}`. Directions are `up`, `down`, `left`, or `right`. `page` is a canonical signed decimal string. Stack bytes are JSON integers. Instruction-stack items are `{kind:"empty"}` or `{kind:"primary", "token":<canonical token>, "instruction_code":<byte>}`. Call frames contain `caller_board`, `call_position`, and `saved_direction`. Phases are `{kind:"normal"}`, `{kind:"after_call"}`, `{kind:"repeat", "total":<byte>, "completed":<byte>}`, `{kind:"fold", "fold_id":<slot>, "internal_position":{x,y}, "internal_direction":<direction>, "saved_outer_direction":<direction>}`, `{kind:"fold_resume", "saved_outer_direction":<direction>}`, or `{kind:"terminated"}`. Thread arrays are ordered by thread ID.
- `metrics` exposes the full raw VM metric set: `global_tick`, `operation_count`, sorted `used_cells` and its `used_cell_count`, sorted `used_memory_addresses` and its `used_memory_address_count`, all three stack high-water counters, and sorted `instruction_variety`. A used cell contains `code_grid` (`{kind:"outer"}` or `{kind:"custom", "id":<slot>}`), `board` (`{kind:"main"}` or `{kind:"function", "id":<slot>}`), nullable `folded_block`, and `position:{x,y}`. A used-memory identity contains `space` (`{kind:"outer"}` or `{kind:"custom_invocation", "global_tick":<decimal string>, "caller_thread_id":<decimal string>, "custom_id":<slot>}`) and arbitrary-precision `address`. Set arrays use canonical VM identity order; count fields equal the corresponding array lengths.
- Each event is an object with a `kind` tag and the corresponding VM data: `cell_reached` has `scope`, `thread_id`, and `cell`; `input_consumed` has `scope`, `thread_id`, and `value`; `register_changed` has `scope`, `register`, `old`, and `new`; `memory_changed` has `scope`, `location`, `old`, and `new`; `code_changed` has `scope`, `cell`, and nullable canonical-token `old`/`new`; `thread_changed` has `scope`, `before`, and `after` VM thread snapshots. Event thread snapshots have the thread fields above except `code_grid`; the event `scope` identifies Outer or the Custom invocation. A scope is `{kind:"outer"}` or `{kind:"custom", "caller_thread_id":<decimal string>, "custom_id":<slot>, "internal_tick":<decimal string>}`. Event order is exactly the VM event order.
- Each runtime error records `global_tick` as a decimal string, its `scope`, stable case-sensitive VM error `code`, and a `details` object containing exactly that error kind's VM-defined fields. Wide IDs, ticks, and addresses in details use decimal strings; byte-sized fields and bounded coordinates/slots use exact JSON integers. Errors retain VM canonical ordering. Fault is `null`, `{kind:"metric_counter_overflow", "counter":<counter name>}`, `{kind:"global_tick_overflow"}`, or `{kind:"internal_invariant_violation"}`.

## Exact integer and ordering rules

Every `u64` value, every arbitrary-precision signed value, and every source byte offset is serialized as a canonical base-10 JSON string, never as a JSON number. This includes seed, limits, tick and operation counters, thread IDs, Page values, memory addresses, and tick/thread fields nested in errors, events, metrics, or phases. Signed strings use `0` or an optional minus sign followed by a nonzero digit and zero or more digits. Unsigned strings use `0` or a nonzero digit followed by zero or more digits. No plus sign, whitespace, or leading zero is allowed.

Byte values, register indexes, slots, coordinates, dimensions, bounded Repeat counts, and Instruction Codes use JSON numbers because their specified ranges are exactly representable. Maps use numeric keys when JSON permits an object map and have ascending numeric key order; otherwise identities are arrays in canonical VM order. Threads use ascending ID order. No output may depend on hash iteration or filesystem enumeration order.

## Exit codes and streams

| Code | Meaning |
| ---: | --- |
| `0` | Valid `check`, successful VM halt, or help output. |
| `2` | Invalid command-line arguments or host data, including invalid numeric options, malformed JSON, invalid bytes, duplicate memory addresses, or incompatible input options. |
| `3` | File access or source-text decoding failure. |
| `4` | Shared compiler rejected the source. For `run`, a versioned `source_error` JSON result is still written to stdout. |
| `5` | VM runtime error. `run` writes the `runtime_error` result and complete terminal snapshot. |
| `6` | The positive `--max-ticks` slice ended while the VM remained running. `run` writes `yielded` with `tick_slice_exhausted`; this is not a halt or VM error. |
| `7` | The work-unit ceiling prevented the next required dispatch. `run` writes `yielded` with `work_unit_budget_exhausted`; earlier complete ticks remain committed and the interrupted tick is rolled back without state or normative metric changes. |
| `8` | VM fault. `run` writes `vm_fault` and the complete fault snapshot. |

`run` returns code `0` only for `halted`. A yield is deliberately distinguishable from normal termination. For successful compiler submission, runtime errors, yields, and VM faults, stdout contains the one complete JSON result described above; stderr is reserved for human-readable messages. Argument/host-data failures and I/O failures produce no JSON result and write their message to stderr. A source diagnostic result is also rendered on stderr with one-based line and Unicode-scalar column. Help goes to stdout. JSON output is never truncated or mixed with progress messages.

## `evaluate` level command

`codegrid evaluate <level.json> <program.cg>` runs the shared Rust level
evaluator through [Level Host API 2](../spec/codegrid-scene-host-contract-v2.md). It does not compile,
validate, execute, score, or rate levels independently. The command requires
`--mode <debug|official>`, `--boundary <exit|wrap>`, `--seed <u64>`,
`--custom-limit <positive-u64>`, and `--limits-file <trusted-profile.json>`.
Each option may appear exactly once. `--format <json|human>` is optional and
defaults to `json`. Evaluation always uses API/profile 2; optional
`--api-version 2` validates the current version and `--api-version 1` is rejected.

Level `allowed_instructions` accepts group names: `READ`,
`REGISTER_POINTER`, `STACK` (PUSH and POP_ADD), `CODEC` (DECODE and ENCODE),
`MEMORY`, `PAGE`, and `SHIFT`. The four direction Primaries, OUTPUT, and HALT
are always allowed. `CALL` includes RETURN; `CUSTOM` includes CUSTOM_RETURN.
CMP, NAND, RANDOM_DIRECTION, CLEAR, ADD, SUB, and FOLDED_BLOCK are independent.
Existing individual names remain valid; permissions form a union. MEMORY/PAGE
still require `memory_enabled: true`, and Attachments need their own whitelist.
Conditional prefixes require independent `CONDITION_0`, `CONDITION_1`, or
`CONDITION_2` entries in `allowed_attachments`. The removed `IF_ZERO` names
are rejected. These groups apply to generated code too and do not change metric counting.
For example, the echo level needs only `"allowed_instructions": ["READ"]`.

`--seed` is a canonical unsigned decimal value in `0..=u64::MAX` and is passed
as the API's explicit `shuffle_seed`, preserving reproducibility. The boundary
and positive Custom limit are passed as resolved evaluation configuration.
The CLI supplies the profile's `max_work_per_call` as each `advance_evaluation`
budget and continues until the shared API returns a terminal result or typed
resource outcome. Safety ceilings come only from the trusted profile; level
JSON and source cannot override them. The command does not accept initial
memory, register state, or inline input overrides.

The profile file is at most 65,536 bytes and must be UTF-8 without a byte-order
mark. The CLI applies its positive `max_level_bytes` and `max_source_bytes`
limits before submitting the level JSON and source text to the API. Both files
must be UTF-8 without a byte-order mark. A missing/unreadable file or invalid
UTF-8 returns code `3`; a file over the trusted size limit returns code `11`.
These pre-evaluation file failures write only to stderr and produce no JSON.
Malformed profiles, invalid profile values, and invalid command-line values
return code `2` and produce no JSON.

In JSON mode, stdout contains exactly one complete Level Host API 2 response,
pretty-printed with a trailing newline. This is the same semantic `result`
projection used by the WASM level adapters. A reached API rejection is still
returned as one response: compiler diagnostics use `source_rejected`, while
malformed or unsupported level data uses `level_rejected`. File failures that
occur before API submission are the exception described above. Stderr is
reserved for file/argument failures and concise compiler or VM fault summaries;
no progress output is emitted. A response is never truncated into apparent
success.

`--format human` prints correctness status, any typed failure or source
diagnostics, visible-test feedback, constraint results, available partial or
final metrics, scoring values/targets, and optional rating. It uses only fields
permitted by the shared API result and does not expose hidden test data.

| Exit code | `evaluate` meaning |
| ---: | --- |
| `0` | Evaluation passed. |
| `2` | Invalid arguments or trusted safety profile. |
| `3` | Level, source, or profile file access/UTF-8 failure before evaluation. |
| `4` | Shared compiler rejected the source. |
| `8` | Evaluator fault or invalid API result. |
| `9` | Player evaluation failed: program rejection, wrong/incomplete output, runtime error, or logical constraint breach. |
| `10` | Level is invalid, uses an unsupported format, or requests an unsupported scene/capability. |
| `11` | Trusted resource limit prevented evaluation or a level/source file exceeded its profile size limit. |

File reads stop after at most the configured limit plus one byte, before UTF-8
decoding. Oversize diagnostics report the observed minimum size. A shared API
`level_api.response_too_large` error also returns exit code `11`.

The `evaluate` exit-code mapping is specific to this command. Existing `check`,
`run`, and `debug` exit codes and result contracts remain unchanged.

For a complete repository fixture run, use:

```text
cargo run -p codegrid-cli -- evaluate fixtures/levels-scene/echo.json fixtures/levels-scene/echo.cg --mode official --boundary exit --seed 18446744073709551615 --custom-limit 1000 --limits-file examples/scene-host-v2/profile-local-v2.json
```

Acceptance for `evaluate` compares the parsed CLI JSON result with a direct
request sequence through the same Level Host API, including an Official ExactIO
run with the maximum `u64` seed. Focused subprocess cases cover source and level
rejections, unsupported scene types, player failures, hidden-test redaction,
constraints, resource ceilings, malformed files and profiles, UTF-8/BOM
handling, and JSON/human output. This checks CLI delegation and presentation;
it does not claim native/WASM parity.

## Acceptance

Local acceptance covers Full compiler diagnostics, each documented option and input representation, malformed and duplicate host data, initial-memory normalization, exact integer serialization, all run statuses, full snapshots and events, canonical ordering, stable exit codes and stream separation, deterministic repeated requests, and both tick-slice and work-unit yields. Every source acceptance decision is delegated to the shared compiler, and every execution transition and snapshot comes from the shared VM.

## Stable errors

See the [complete error specification](../spec/codegrid-error-codes.md). Host/argument/file errors render `error: [cli.code] message` on stderr while retaining the documented exit categories. Diagnostic `code` is an additive JSON field; stable runtime codes and schema version 1 remain unchanged. Debug protocol 2 replaces protocol 1 string errors with `{code,message}`. Oversized requests return `debug.request_limit_exceeded` when the response can be written, then exit 2; broken transports report `debug.transport_io` on stderr.

## Conditional prefix and CMP migration (2026-10-03)

The earlier conditional-prefix migration introduced executable IR format 2 (superseded by format 3 for Function-private registers and NEG), RandomDirection `??` (126), CMP `?=` (124), and fixed conditional prefixes `?0`–`?2`. Normal code-view cells expose an additive nullable `prefix` field containing the canonical prefix spelling. This field survives Primary mutation and clearing. Folded Block views keep their existing arrays of nullable strings; nonempty strings include any prefix followed by the Primary token. These are read-only projections, not executable interchange data.

The Runtime API v3, browser binding envelope, server ABI v4, CLI JSON schema 1, and debug protocol 2 remain unchanged; existing lifecycle and transport fields are preserved. Language source acceptance, IR version, capability vocabulary, and new code-view fields follow the recorded migration decision. Removed source forms and obsolete instruction bytes are not compatibility aliases. Consumers displaying cells should include the prefix.

## Function register snapshot migration (2026-10-08)

Executable IR uses format 3; recompile formats 1 and 2 from source. Thread JSON includes nullable ten-byte `private_registers`. Call frames include nullable `saved_registers` and `saved_register_pointer`. Null denotes the live shared Main bank. ThreadChanged before/after snapshots expose private mutations with thread/context identity; RegisterChanged retains its shared-bank meaning. Debugger register scopes select the active or suspended frame bank.

## Status flag projection (2026-10-08)

Current thread snapshots and ThreadChanged before/after projections include required
`status_flag` (integer 0 or 1). Call frames include `saved_status_flag` with the
same range. Custom/internal thread events use the same fields. The active flag
is independent of registers; suspended frames retain the caller flag.
