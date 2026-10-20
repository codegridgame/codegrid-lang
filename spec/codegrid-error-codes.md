# CodeGrid Error Code Specification

Registry version: 3. Language entries are normative for compiler, VM, CLI, debug, editor, and host-adapter error identities. Game error identities are owned by the Rust Game Runtime in the sibling game repository and are not part of this language registry. No user-published release has established an error compatibility baseline. The source and VM specifications still define language acceptance, execution, ordering and rollback. This document does not introduce new language failures.

## 1. Identity and compatibility

- Consumers MUST use a code together with its layer/transport version, never parse message wording. Codes are case-sensitive. Existing VM PascalCase codes, WASM snake_case codes, JSON-RPC numeric codes, and ABI sentinels retain their spelling. New source, IR, CLI, debug and editor identifiers use a dotted namespace.
- Codes identify stable categories, not necessarily one message. A category may cover several concrete validation failures; source location, resource fields and configuration fields distinguish the instances.
- Before a user-published release, identifiers may be added, removed, or renamed with their consumers; do not retain unpublished aliases, number reservations, or migration layers. After publication, record the exact compatibility scope here.
- Source diagnostics expose codes under Runtime API v4, Server ABI v5 and CLI run-result schema 1. Debug protocol 2 carries structured errors. Older debug protocol 1 is rejected with editor.unsupported_debug_protocol.
- This registry covers errors deliberately emitted by the language toolchain. Operating-system termination, browser engine traps, Wasmtime fuel/stack traps, allocation aborts, and third-party VS Code failures outside a callable adapter are host failures, not newly invented language errors. Hosts retain their own platform codes; the editor catches exposed exceptions as a coded host category.
- The machine-readable registry is [codegrid-error-codes.json](codegrid-error-codes.json). Every entry identifies layer, code, trigger, return surface and state effect. The called ABI export is part of the identity for numeric sentinel errors.

## 2. Response shapes

### Four-digit presentation numbers

The complete [four-digit error table](codegrid-error-numbers.md) is generated
from the JSON registry. Each scoped identity has a unique, stable `error_number`
string of exactly four decimal digits. Current language ranges are source 1000,
IR 2000, VM 3000, faults 3100, Runtime API 4000, CLI 5000, debug 5100, editor
6000, browser 7000, server 7500, LSP 8000, and ABI sentinels 8100. The table
contains only language-toolchain and host-adapter identities.

Process errors render `[number] [original_code]`; JSON error, diagnostic and
fault objects add `error_number` without replacing original `code` or `kind`.
LSP keeps standard JSON-RPC error codes and carries the number in `error.data`;
diagnostics use `data.error_number`. DAP uses the four-digit number as its
presentation `id` and includes `error_number` in format variables. Binary ABI zero sentinels cannot carry an extra field: their
export-specific numbers are documented in the table, and the zero ABI remains
compatible. Uncaught OS/runtime traps remain host failures as defined above.

Native Rust language callers can use the data-only `codegrid_model::error_number`
lookup and diagnostic/error helper methods. TypeScript language presentation
data is generated from the same registry; it introduces no language semantics.

### Source diagnostics

`{code, severity, message, span:{start,end}}`. Span offsets are half-open UTF-8 bytes in core/JSON, serialized as decimal strings by native/WASM hosts. LSP exposes the same code with a UTF-16 range. CLI check renders `path:line:column: error: [code] message`. No invalid source yields executable IR. Core helper parser errors expose code even when no source span is supplied.

### Runtime errors and faults

`{transition_id, scope, code, details}`; scope distinguishes Outer and a Custom invocation/internal step. Wide IDs, steps and addresses use lossless decimal strings at JSON boundaries. Equal writes still conflict; rejected reads do not manufacture secondary conflicts. Canonical ordering, participant normalization, and rollback follow VM §§10 and 14; prepared candidates expose their footprint through `StepReport`. Faults use `{kind,...}` rather than being mixed into the language-runtime error list.

### API/WASM errors

API Rust errors expose `ApiError::code()`. Browser/server JSON uses `error:{code,details? ,message?}` with its current version fields. Message is optional on some API errors; code is required. API and server identities are scoped separately where their names overlap.

### Debug and editor errors

`{debug_protocol_version:2,error:{code,error_number,message}}` or `{debug_protocol_version:2,body:...}`. Compilation diagnostics remain a successful transport body containing diagnostic records; they are not malformed-request errors. DAP failure response.message is the original code; `body.error` uses the four-digit number as `id`, `format:"[{error_number}] [{code}] {message}"`, `variables:{error_number,code,message}`, and `showUser:true`. Runtime error stops expose the original snapshot error codes and their numbers. Host/limit stops include the host identity in the stopped event.

### Process and ABI failures

CLI/debug/LSP process messages render a bracketed code on stderr. If a pipe cannot be written, no valid JSON response can be promised. Server buffer exports return the documented zero sentinel without a JSON envelope; its coarse category is intentional under ABI v5. No host may infer a more precise cause from zero alone.

## 3. Complete identifier registry

### source

Return surface: Diagnostic.code in core, CLI JSON, LSP diagnostics, browser/server diagnostics and debug diagnostics. CLI check renders [code].

State effect: Compilation rejection: no executable IR or VM instance.

| Code | Trigger / meaning |
| --- | --- |
| `source.unsupported_line_ending` | A bare carriage return occurs outside or inside a block comment. |
| `source.nested_comment` | A block comment starts inside an open block comment. |
| `source.unterminated_comment` | A block comment reaches end of source without its closing delimiter. |
| `source.invalid_entry` | An Entry token is malformed or carries an Attachment. |
| `source.invalid_cell` | A complete cell token is unknown, malformed, detached, combined, or has an invalid Primary/Attachment pairing. |
| `source.missing_directive_prefix` | The public directive-head parser receives a head without @. |
| `source.invalid_directive_path` | A structural path has malformed segments or an out-of-range ID. |
| `source.unknown_directive` | The directive head is not a recognized structural or attribute directive. |
| `source.size_argument_count` | A size declaration does not have exactly one argument. |
| `source.invalid_size` | Dimensions do not use the required positive decimal WIDTHxHEIGHT spelling. |
| `source.zero_size` | A board dimension is zero. |
| `source.geometry_limit` | Dimensions, their product, or total cells exceed portable source bounds or overflow. |
| `source.end_argument_count` | An end directive has more than one closing name. |
| `source.invalid_end_name` | A closing name is not a valid structural path. |
| `source.definition_arguments` | A non-Folded-Block definition contains cells on its directive line. |
| `source.folded_structure` | A directive other than the closing end occurs inside a Folded Block. |
| `source.definition_scope` | A relative or qualified Function/Folded Block definition appears outside its permitted owner scope. |
| `source.nested_codegrid` | A Main or Custom CodeGrid definition is nested inside another definition. |
| `source.duplicate_definition` | Main, Custom, Function, or Folded Block is declared more than once at the same identity. |
| `source.mixed_main` | Implicit Main grid rows are combined with an explicit Main block. |
| `source.nested_fold` | A Folded Block is nested inside another Folded Block. |
| `source.fold_row_count` | A Folded Block contains more or fewer than one row. |
| `source.grid_scope` | A grid row appears outside the declared explicit Main block. |
| `source.grid_order` | Board rows are noncontiguous or appear after nested definitions, or a nested definition precedes its owner grid. |
| `source.size_scope` | A size declaration occurs inside a Folded Block. |
| `source.invalid_span` | An internal source location cannot be read from the supplied source; compilation is rejected. |
| `source.duplicate_size` | A lexical program, CodeGrid, or Function scope repeats its size declaration. |
| `source.unexpected_end` | An end directive has no open definition to close. |
| `source.end_mismatch` | The closing name does not identify the open definition. |
| `source.missing_end` | An open definition reaches end of source without its closing end. |
| `source.explicit_main_required` | Function or Custom definitions exist without an explicit Main block. |
| `source.undefined_custom` | A qualified definition belongs to an undeclared Custom CodeGrid. |
| `source.missing_grid` | A required Main, Custom Main, or Function grid is absent. |
| `source.height_mismatch` | The number of rows differs from the effective board height. |
| `source.width_mismatch` | A row width differs from the effective board width. |
| `source.function_entry_count` | A Function board has more than one explicit Entry. |
| `source.fold_width` | A Folded Block width differs from its owner board width. |
| `source.fold_entry` | An Entry appears in a Folded Block. |
| `source.fold_attachment` | A suffix Attachment appears in a Folded Block; conditional prefixes are allowed. |
| `source.fold_primary` | A Primary is forbidden in its Folded Block context, including direction changes and random directions. |
| `source.return_scope` | RETURN appears on a Main board. |
| `source.custom_return_scope` | CUSTOM_RETURN appears outside a Custom Main board. |
| `source.custom_call_scope` | A Custom definition calls a Custom instruction. |
| `source.missing_program` | Structural compilation did not produce a program. |

### ir

Return surface: IrError.code; compiler forwards the original IR code into Diagnostic.code with compiler source spans.

State effect: IR verification fails; unchecked IR cannot execute.

| Code | Trigger / meaning |
| --- | --- |
| `ir.unsupported_version` | Executable IR format version is unsupported. |
| `ir.geometry_overflow` | Board dimensions overflow the host address space. |
| `ir.geometry_limit` | Board geometry exceeds portable Full bounds. |
| `ir.invalid_layout` | Dimensions are zero or do not match cell count. |
| `ir.function_entry_count` | A Function has more than one explicit Entry. |
| `ir.fold_width` | Folded Block length differs from board width. |
| `ir.fold_primary` | A Folded Block contains a forbidden instruction, including direction changes and random directions. |
| `ir.entry_instruction` | An Entry shares its cell with a Primary or Attachment. |
| `ir.detached_attachment` | A prefix or suffix Attachment has no initial Primary (including an invalid folded prefix coordinate). |
| `ir.repeat_count` | Repeat count is outside 2 through 9. |
| `ir.attachment_primary` | A suffix Attachment is attached to a nonencodable Primary; conditional prefixes have independent eligibility. |
| `ir.repeat_call_return` | Repeat is attached to CALL or RETURN. |
| `ir.return_scope` | RETURN is outside a Function. |
| `ir.custom_return_scope` | CUSTOM_RETURN is outside a Custom Main. |
| `ir.undefined_function` | CALL references an undeclared Function in its CodeGrid. |
| `ir.undefined_fold` | A Folded Block reference is absent from its owner board. |
| `ir.custom_reference` | A Custom reference is absent or the call context forbids Custom calls. |

### vm

Return surface: RuntimeError.code, CLI/WASM snapshot.errors and step results. DAP displays the same codes.

State effect: Entire failing outer transition rolls back; committed events/output are empty; committed_steps does not advance.

| Code | Trigger / meaning |
| --- | --- |
| `ConcurrentCallerStackReadConflict` | Multiple nonempty caller-stack reads conflict within a Custom invocation. |
| `ConcurrentCallerStackReadWriteConflict` | A successful caller-stack read conflicts with a caller-stack output. |
| `ConcurrentCallerStackWriteConflict` | Multiple Custom outputs write the caller stack in one aligned internal transition. |
| `ConcurrentCodeWriteConflict` | Multiple writes target the same static code cell. |
| `ConcurrentInputConflict` | Multiple live threads consume the same available Outer input. |
| `ConcurrentMemoryWriteConflict` | Multiple writes target the same scoped memory address. |
| `ConcurrentOutputConflict` | Multiple Outer outputs conflict in one transition. |
| `ConcurrentWriteConflict` | Multiple writes target the same scoped register, including equal values. |
| `ReturnWithoutCall` | RETURN executes without a caller frame. |

Details fields:

| Code | Required details |
| --- | --- |
| `ConcurrentCallerStackReadConflict` | `internal_thread_ids` |
| `ConcurrentCallerStackReadWriteConflict` | `internal_thread_ids` |
| `ConcurrentCallerStackWriteConflict` | `internal_thread_ids` |
| `ConcurrentCodeWriteConflict` | `cell, thread_ids` |
| `ConcurrentInputConflict` | `thread_ids` |
| `ConcurrentMemoryWriteConflict` | `address, thread_ids` |
| `ConcurrentOutputConflict` | `thread_ids` |
| `ConcurrentWriteConflict` | `register, thread_ids` |
| `ReturnWithoutCall` | `thread_id, board, position` |

### fault

Return surface: snapshot.fault.kind; metric_counter_overflow also includes counter.

State effect: VM fault; retain the VM-defined terminal state and report a distinct fault, never a normal halt.

| Code | Trigger / meaning |
| --- | --- |
| `metric_counter_overflow` | A normative metric counter cannot represent the attempted increment. |
| `committed_steps_overflow` | The committed-step counter cannot represent another step. |
| `internal_invariant_violation` | A VM internal invariant fails. |

### api

Return surface: ApiError.code(); browser API error.code. Server adapter identities are listed separately.

State effect: Rejected requests do not advance the VM; a work-limited step rolls back its interrupted transition.

| Code | Trigger / meaning |
| --- | --- |
| `unsupported_version` | Request Runtime API version differs from 4. |
| `source_limit_exceeded` | Source UTF-8 bytes exceed the configured maximum. |
| `program_limit_reached` | The runtime compiled-program registry is full. |
| `instance_limit_reached` | The runtime instance registry is full. |
| `input_limit_exceeded` | Input length exceeds the configured byte maximum. |
| `initial_memory_limit_exceeded` | Initial-memory entry count exceeds its maximum. |
| `duplicate_initial_memory_address` | Initial memory contains duplicate addresses. |
| `invalid_configuration` | An execution setting is invalid; details identify the field. |
| `unknown_program_handle` | Program handle is unknown, released, foreign or otherwise unavailable. |
| `unknown_instance_handle` | Instance handle is unknown, released, foreign or otherwise unavailable. |
| `zero_step_budget` | A bounded run requests zero steps. |
| `run_step_budget_exceeded` | The requested max_steps exceeds the host maximum for one call. |
| `instance_step_budget_exceeded` | The requested run exceeds the instance remaining total step budget. |
| `work_unit_budget_exceeded` | A step needs more work units than the per-call deterministic work ceiling. |
| `handle_space_exhausted` | No new opaque handle can be allocated. |
| `vm_initialization_failed` | VM initialization fails; reason identifies InitialThreadIdOverflow. |

### cli

Return surface: Native stderr: error: [code] message. Source diagnostic messages have their own source/IR code.

State effect: Exit categories remain stable; host failures do not produce a run-result JSON.

| Code | Trigger / meaning |
| --- | --- |
| `cli.invalid_arguments` | Subcommand, option, option multiplicity, required argument, canonical numeric value or direct byte list is invalid. |
| `cli.source_io` | Source cannot be read as UTF-8, including unavailable files and invalid UTF-8. |
| `cli.input_io` | Input JSON file cannot be read. |
| `cli.input_json` | Input file does not decode as one valid JSON byte array. |
| `cli.input_data` | Decoded input fails host byte validation. |
| `cli.memory_io` | Initial-memory file cannot be read. |
| `cli.memory_json` | Initial-memory file does not decode as a valid JSON memory array. |
| `cli.memory_data` | Initial-memory addresses or entries fail validation, including duplicates. |
| `cli.serialization_failed` | A native run-result JSON cannot be serialized. |
| `cli.vm_initialization_failed` | Verified program cannot initialize its VM. |


### debug

Return surface: Debug protocol 2: error.code and error.message; transport I/O failures use coded stderr when stdout is unavailable.

State effect: State unchanged on invalid requests; work-limit rollback uses the same VM transition; request-size failure terminates the process.

| Code | Trigger / meaning |
| --- | --- |
| `debug.invalid_request` | JSON-lines request is malformed, has unknown fields, has wrong field types, or names an unsupported command. |
| `debug.invalid_configuration` | Seed/limit configuration is invalid or exceeds u64. |
| `debug.already_loaded` | A session receives another launch after a successful launch. |
| `debug.no_program` | Step or snapshot occurs before successful launch. |
| `debug.vm_initialization_failed` | The compiled program cannot initialize its VM. |
| `debug.step_budget_exceeded` | The session has committed its configured maximum number of VM steps before another step request. |
| `debug.work_unit_budget_exceeded` | One atomic VM step exceeds its deterministic work-unit ceiling and rolls back. |
| `debug.request_limit_exceeded` | A request line exceeds 4 MiB including its line ending. |
| `debug.transport_io` | The process cannot read a request or write a response. |

### editor

Return surface: DAP failed response.message is the code; body.error.variables.code is the same code (DAP id 1000 is a generic presentation ID). Stopped host failures include body.code. UI-only errors render [code].

State effect: Editor/transport rejection; never substitute these categories for a native source or VM error.

| Code | Trigger / meaning |
| --- | --- |
| `editor.operation_failed` | An otherwise unclassified editor operation fails, including document access or platform exceptions. |
| `editor.unsupported_debug_protocol` | Native debug protocol version is not 2. |
| `editor.invalid_debug_response` | Native response is malformed JSON or has an invalid structured error. |
| `editor.runtime_start_failed` | The executable cannot be spawned. |
| `editor.runtime_exited` | The runtime process exits while an operation is pending. |
| `editor.transport_io` | Native process pipe read/write fails. |
| `editor.session_closed` | A pending request is cancelled when the session is disposed. |
| `editor.invalid_expression` | Watch/evaluate expression is not a read-only state path. |
| `editor.unknown_state_path` | A requested read-only property/index does not exist. |
| `editor.no_caller_frame` | Step Out is requested without a selected caller frame. |
| `editor.unsupported_debug_request` | The DAP request is not supported. |
| `editor.workspace_untrusted` | Run/debug is attempted in an untrusted workspace. |
| `editor.invalid_program` | Launch does not provide a nonempty source-file path. |
| `editor.invalid_input` | Input is not an array of byte integers in 0 through 255. |
| `editor.source_errors` | Compilation rejected the source; individual diagnostics retain their source/IR codes. |
| `editor.no_session` | Stepping or continuing has no active native runtime. |
| `editor.session_running` | A step/continue request occurs while the session is already running. |
| `editor.vm_terminal` | Execution is requested after VM halt/error. |
| `editor.no_codegrid_file` | Run/Debug command has no current .cg document. |
| `editor.lsp_path_empty` | The configured language-server executable path is empty. |
| `editor.lsp_start_failed` | The optional language server fails to start. |
| `editor.invalid_template` | New-file command receives an unknown template identifier. |

### browser

Return surface: Browser adapter error.code (API v4). Runtime API errors also use the API table.

State effect: Pre-copy validation failures do not dispatch; oversized retained-state instances are released; response limits produce complete structured errors.

| Code | Trigger / meaning |
| --- | --- |
| `invalid_source` | Source payload is not a JavaScript string. |
| `invalid_input` | Input is not a Uint8Array. |
| `input_allocation_failed` | Input buffer cannot be reserved before copying. |
| `request_allocation_failed` | A request buffer cannot be reserved. |
| `source_payload_limit_exceeded` | Source UTF-8 bytes exceed the adapter payload ceiling. |
| `input_payload_limit_exceeded` | Input bytes exceed the adapter payload ceiling. |
| `request_payload_limit_exceeded` | Request JSON/strings or combined request bytes exceed the adapter ceiling. |
| `invalid_request_field` | A required request field has the wrong JavaScript type. |
| `invalid_program_handle` | Program handle spelling is not a canonical nonzero u64 decimal string. |
| `invalid_initial_memory` | Initial-memory JSON has an invalid shape or invalid entry. |
| `invalid_configuration` | Configuration is invalid under the runtime contract. |
| `invalid_request` | A request value cannot be accepted by the binding. |
| `invalid_instance_configuration` | Configuration JSON has an invalid shape or required fields. |
| `invalid_instance_handle` | Instance handle spelling is not a canonical nonzero u64 decimal string. |
| `invalid_step_limit` | Requested max_steps value is not a canonical u64 decimal string. |
| `instance_state_limit_exceeded` | Canonical retained snapshot bytes exceed the configured instance quota. |
| `invalid_memory_address` | An initial-memory address is not a canonical arbitrary-precision signed decimal string. |
| `invalid_configuration_integer` | Seed is not a valid canonical integer string. |
| `response_payload_limit_exceeded` | The full response cannot fit the configured response ceiling. |
| `browser.invalid_host_limit` | BrowserRuntime constructor receives invalid numeric/string ceilings or host limits. |

### server

Return surface: Server ABI v5 / Runtime API v4 JSON error.code.

State effect: Invalid/over-limit requests do not execute; work limit rolls back; oversized retained instances are released; shutdown permanently closes the session.

| Code | Trigger / meaning |
| --- | --- |
| `invalid_input` | Input is not a JSON array. |
| `invalid_input_byte` | An input element is not an exact byte integer. |
| `invalid_host_limit` | A required host ceiling is invalid or nonpositive where required. |
| `host_limit_exceeds_adapter_ceiling` | A configured host limit exceeds the adapter hard ceiling. |
| `invalid_decimal_integer` | A required u64 string is noncanonical or out of range. |
| `invalid_memory_address` | Initial-memory address spelling is invalid. |
| `invalid_request_field` | Required request field is missing or has the wrong type. |
| `unsupported_api_version` | Request Runtime API version differs from 4. |
| `source_payload_limit_exceeded` | Source bytes exceed the runtime source ceiling. |
| `input_payload_limit_exceeded` | Input bytes exceed the configured ceiling. |
| `vm_initialization_error` | VM initialization fails (server alias of vm_initialization_failed). |
| `invalid_initial_memory` | Initial-memory payload is not an array. |
| `initial_memory_payload_limit_exceeded` | Initial-memory payload/entry collection exceeds the adapter ceiling. |
| `invalid_initial_memory_entry` | An initial-memory entry has invalid fields, address, or value. |
| `request_payload_limit_exceeded` | Request bytes exceed the adapter request ceiling. |
| `invalid_request_json` | Request bytes are not one valid UTF-8 JSON value. |
| `invalid_request` | Top-level request is not a valid object. |
| `unsupported_abi_version` | Request ABI version differs from 5. |
| `runtime_closed` | A request targets a permanently shut-down module session. |
| `runtime_not_initialized` | A runtime operation occurs before initialization. |
| `unsupported_operation` | The requested operation is not recognized. |
| `runtime_already_initialized` | Initialize is called twice. |
| `invalid_program_handle` | Program handle spelling is invalid. |
| `invalid_instance_handle` | Instance handle spelling is invalid. |
| `invalid_host_limits` | Initialization host_limits is not an object. |
| `response_payload_limit_exceeded` | The full response exceeds the configured response ceiling. |
| `instance_state_limit_exceeded` | Canonical retained instance state exceeds its configured quota. |
| `program_limit_reached` | The runtime compiled-program registry is full. |
| `instance_limit_reached` | The runtime instance registry is full. |
| `initial_memory_limit_exceeded` | Initial-memory entry count exceeds its maximum. |
| `duplicate_initial_memory_address` | Initial memory contains duplicate addresses. |
| `invalid_configuration` | An execution setting is invalid; details identify the field. |
| `unknown_program_handle` | Program handle is unknown, released, foreign or otherwise unavailable. |
| `unknown_instance_handle` | Instance handle is unknown, released, foreign or otherwise unavailable. |
| `zero_step_budget` | A bounded run requests zero steps. |
| `run_step_budget_exceeded` | The requested max_steps exceeds the host maximum for one call. |
| `instance_step_budget_exceeded` | The requested run exceeds the instance remaining total step budget. |
| `work_unit_budget_exceeded` | A step needs more work units than the per-call deterministic work ceiling. |
| `handle_space_exhausted` | No new opaque handle can be allocated. |

### lsp

Return surface: JSON-RPC error.code is numeric for protocol errors; native process failures use coded stderr. Source diagnostics use string source/IR codes.

State effect: Rejected request or terminal transport failure; no VM execution occurs in LSP.

| Code | Trigger / meaning |
| --- | --- |
| `-32600` | Invalid JSON-RPC request or lifecycle operation. |
| `-32601` | Unknown/unsupported method. |
| `-32602` | Method parameters are invalid. |
| `-32002` | Server is not initialized for the requested operation. |
| `lsp.transport_io` | LSP framing, parsing or process I/O fails. |
| `lsp.abnormal_exit` | Exit or EOF occurs without the required shutdown lifecycle. |

### server-abi

Return surface: Numeric sentinel 0, interpreted with the called ABI export; no JSON response exists at this boundary.

State effect: Pre-dispatch reservation failure does not execute the request. A late response-write failure has no rollback guarantee; do not blindly retry.

| Code | Trigger / meaning |
| --- | --- |
| `0` (alloc_buffer) | Allocation length/pool/resource reservation fails; returned pointer is zero. |
| `0` (free_buffer) | The pointer is unknown; release returns zero. |
| `0` (process_request) | Pointer/length validation or response reservation/write fails; packed response is zero. |

## 4. Process exit and bounded-run status

| Native CLI exit | Meaning |
| --- | --- |
| 0 | Normal halt or successful check/help/debug disconnect/EOF. |
| 2 | Invalid command/host data; debug request-size failure. |
| 3 | File/source/transport I/O failure. |
| 4 | Source compilation rejected; use each diagnostic code. |
| 5 | Runtime error or VM initialization failure. |
| 6 | Step slice exhausted. |
| 7 | Work-unit budget exhausted. |
| 8 | VM fault. |

CLI run results use `step_limit_reached` or `work_limit_reached` when their configured step or work-unit boundary is reached; `yield_reason` is null. Runtime API/WASM bounded runs use `{ "kind": "yielded", "reason": "step_slice_exhausted" }` or `{ "kind": "yielded", "reason": "work_unit_budget_exhausted" }`. These are resumable host statuses, not language error codes. Debug sessions use `debug.step_budget_exceeded` for their cumulative session step ceiling and `debug.work_unit_budget_exceeded` when one transition exceeds its work-unit ceiling. An editor no-debug run uses DAP exit 0 for halt, 5 for VM failure, and 1 for host/session failure; detailed identity is in the coded output. LSP process exit 1 reports abnormal lifecycle or transport failure with its stderr code.

## 5. Fault and host details

Metric overflow counter values are operation_count, committed_steps, data_stack_usage, instruction_stack_usage and call_stack_usage. InitialThreadIdOverflow is the currently defined VM initialization reason. API details are the fields of the corresponding ApiError variant: version received/supported; sizes received_bytes/maximum; limits maximum; initial-memory received_entries/maximum or address; invalid configuration field; handles handle (server uses program/instance); run budgets requested/maximum or requested/remaining/maximum. Browser state-quota details use maximum_bytes where present. A missing optional message/details field must not cause a consumer to discard its code.

## 6. Implementation and verification rules

Assign source/IR codes where validation detects the failure. Never classify an error by matching English text. Adapters forward the compiler/VM identifier rather than defining a competing source validator or interpreter. Tests must check both code and location/resource fields, include malformed transport errors, and compare native/LSP/browser/server diagnostic codes. Registry consistency checks reject an emitted code missing from this registry. Updating a test message must not automatically update its expected code.

## Conditional prefix migration diagnostics

Malformed, detached, repeated, out-of-range, or obsolete conditional source atoms use source.invalid_cell. Prefixes do not suppress contextual source/reference diagnostics. IR Entry sharing a cell with a prefix uses ir.entry_instruction; detached normal or folded prefixes use ir.detached_attachment. Unsupported executable format 1 uses ir.unsupported_version. No new error identity or number is introduced; unpublished identifiers follow the current registry directly. Invalid DECODE bytes, including retired instruction numbers, remain counted no-ops and are not errors.

### Invalid-cell message context

The parser includes the rejected source atom in invalid-cell messages and describes the detected token, prefix, suffix, count, or Primary/Attachment restriction. The UTF-8 span selects the same complete atom. Prefixing a malformed body does not replace its specific cause with a generic prefix error. This refines message context without adding or changing error identities; consumers must use the code or number, not message text, for programmatic decisions.
