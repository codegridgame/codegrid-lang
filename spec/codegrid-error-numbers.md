# CodeGrid Four-Digit Error Numbers

Generated from [codegrid-error-codes.json](codegrid-error-codes.json). Numbers are stable, unique, append-only four-digit decimal strings. Original identifiers and protocol codes remain compatible.

| Number | Layer | Original code | Trigger / meaning |
| --- | --- | --- | --- |
| `1000` | source | `source.unsupported_line_ending` | A bare carriage return occurs outside or inside a block comment. |
| `1001` | source | `source.nested_comment` | A block comment starts inside an open block comment. |
| `1002` | source | `source.unterminated_comment` | A block comment reaches end of source without its closing delimiter. |
| `1003` | source | `source.invalid_entry` | An Entry token is malformed or carries an Attachment. |
| `1004` | source | `source.invalid_cell` | A complete cell token is unknown, malformed, detached, combined, or has an invalid Primary/Attachment pairing. |
| `1005` | source | `source.missing_directive_prefix` | The public directive-head parser receives a head without @. |
| `1006` | source | `source.invalid_directive_path` | A structural path has malformed segments or an out-of-range ID. |
| `1007` | source | `source.unknown_directive` | The directive head is not a recognized structural or attribute directive. |
| `1008` | source | `source.size_argument_count` | A size declaration does not have exactly one argument. |
| `1009` | source | `source.invalid_size` | Dimensions do not use the required positive decimal WIDTHxHEIGHT spelling. |
| `1010` | source | `source.zero_size` | A board dimension is zero. |
| `1011` | source | `source.geometry_limit` | Dimensions, their product, or total cells exceed portable source bounds or overflow. |
| `1012` | source | `source.end_argument_count` | An end directive has more than one closing name. |
| `1013` | source | `source.invalid_end_name` | A closing name is not a valid structural path. |
| `1014` | source | `source.definition_arguments` | A non-Folded-Block definition contains cells on its directive line. |
| `1015` | source | `source.folded_structure` | A directive other than the closing end occurs inside a Folded Block. |
| `1016` | source | `source.definition_scope` | A relative or qualified Function/Folded Block definition appears outside its permitted owner scope. |
| `1017` | source | `source.nested_codegrid` | A Main or Custom CodeGrid definition is nested inside another definition. |
| `1018` | source | `source.duplicate_definition` | Main, Custom, Function, or Folded Block is declared more than once at the same identity. |
| `1019` | source | `source.mixed_main` | Implicit Main grid rows are combined with an explicit Main block. |
| `1020` | source | `source.nested_fold` | A Folded Block is nested inside another Folded Block. |
| `1021` | source | `source.fold_row_count` | A Folded Block contains more or fewer than one row. |
| `1022` | source | `source.grid_scope` | A grid row appears outside the declared explicit Main block. |
| `1023` | source | `source.grid_order` | Board rows are noncontiguous or appear after nested definitions, or a nested definition precedes its owner grid. |
| `1024` | source | `source.size_scope` | A size declaration occurs inside a Folded Block. |
| `1025` | source | `source.invalid_span` | An internal source location cannot be read from the supplied source; compilation is rejected. |
| `1026` | source | `source.duplicate_size` | A lexical program, CodeGrid, or Function scope repeats its size declaration. |
| `1027` | source | `source.unexpected_end` | An end directive has no open definition to close. |
| `1028` | source | `source.end_mismatch` | The closing name does not identify the open definition. |
| `1029` | source | `source.missing_end` | An open definition reaches end of source without its closing end. |
| `1030` | source | `source.explicit_main_required` | Function or Custom definitions exist without an explicit Main block. |
| `1031` | source | `source.undefined_custom` | A qualified definition belongs to an undeclared Custom CodeGrid. |
| `1032` | source | `source.missing_grid` | A required Main, Custom Main, or Function grid is absent. |
| `1033` | source | `source.height_mismatch` | The number of rows differs from the effective board height. |
| `1034` | source | `source.width_mismatch` | A row width differs from the effective board width. |
| `1035` | source | `source.main_entry_count` | An Outer or Custom Main board has no Entry. |
| `1036` | source | `source.function_entry_count` | A Function board does not have exactly one Entry. |
| `1037` | source | `source.fold_width` | A Folded Block width differs from its owner board width. |
| `1038` | source | `source.fold_entry` | An Entry appears in a Folded Block. |
| `1039` | source | `source.fold_attachment` | An Attachment appears in a Folded Block. |
| `1040` | source | `source.fold_primary` | A Primary is forbidden in its Folded Block context. |
| `1041` | source | `source.return_scope` | RETURN appears on a Main board. |
| `1042` | source | `source.custom_return_scope` | CUSTOM_RETURN appears outside a Custom Main board. |
| `1043` | source | `source.custom_call_scope` | A Custom definition calls a Custom instruction. |
| `1044` | source | `source.missing_program` | Structural compilation did not produce a program. |
| `2000` | ir | `ir.unsupported_version` | Executable IR format version is unsupported. |
| `2001` | ir | `ir.geometry_overflow` | Board dimensions overflow the host address space. |
| `2002` | ir | `ir.geometry_limit` | Board geometry exceeds portable Full bounds. |
| `2003` | ir | `ir.invalid_layout` | Dimensions are zero or do not match cell count. |
| `2004` | ir | `ir.main_entry_count` | A Main board has no Entry. |
| `2005` | ir | `ir.function_entry_count` | A Function does not have exactly one Entry. |
| `2006` | ir | `ir.fold_width` | Folded Block length differs from board width. |
| `2007` | ir | `ir.fold_primary` | A Folded Block contains a forbidden instruction. |
| `2008` | ir | `ir.entry_instruction` | An Entry shares its cell with a Primary or Attachment. |
| `2009` | ir | `ir.detached_attachment` | An Attachment has no Primary. |
| `2010` | ir | `ir.repeat_count` | Repeat count is outside 2 through 5. |
| `2011` | ir | `ir.attachment_primary` | An Attachment is attached to a nonencodable Primary. |
| `2012` | ir | `ir.repeat_call_return` | Repeat is attached to CALL or RETURN. |
| `2013` | ir | `ir.return_scope` | RETURN is outside a Function. |
| `2014` | ir | `ir.custom_return_scope` | CUSTOM_RETURN is outside a Custom Main. |
| `2015` | ir | `ir.undefined_function` | CALL references an undeclared Function in its CodeGrid. |
| `2016` | ir | `ir.undefined_fold` | A Folded Block reference is absent from its owner board. |
| `2017` | ir | `ir.custom_reference` | A Custom reference is absent or the call context forbids Custom calls. |
| `3000` | vm | `ConcurrentCallerStackReadConflict` | Multiple nonempty caller-stack reads conflict within a Custom invocation. |
| `3001` | vm | `ConcurrentCallerStackReadWriteConflict` | A successful caller-stack read conflicts with a caller-stack output. |
| `3002` | vm | `ConcurrentCallerStackWriteConflict` | Multiple Custom outputs write the caller stack in one aligned internal tick. |
| `3003` | vm | `ConcurrentCodeWriteConflict` | Multiple writes target the same static code cell. |
| `3004` | vm | `ConcurrentInputConflict` | Multiple live threads consume the same available Outer input. |
| `3005` | vm | `ConcurrentMemoryWriteConflict` | Multiple writes target the same scoped memory address. |
| `3006` | vm | `ConcurrentOutputConflict` | Multiple Outer outputs conflict in one tick. |
| `3007` | vm | `ConcurrentWriteConflict` | Multiple writes target the same scoped register, including equal values. |
| `3008` | vm | `CustomExecutionLimitExceeded` | A Custom invocation exceeds its normative execution limit. |
| `3009` | vm | `OutOfBounds` | Execution violates the VM contract for a required board position. |
| `3010` | vm | `ReturnWithoutCall` | RETURN executes without a caller frame. |
| `3100` | fault | `metric_counter_overflow` | A normative metric counter cannot represent the attempted increment. |
| `3101` | fault | `global_tick_overflow` | The Global Tick counter cannot represent another tick. |
| `3102` | fault | `internal_invariant_violation` | A VM internal invariant fails. |
| `4000` | api | `unsupported_version` | Request API version differs from 3. |
| `4001` | api | `source_limit_exceeded` | Source UTF-8 bytes exceed the configured maximum. |
| `4002` | api | `program_limit_reached` | The runtime compiled-program registry is full. |
| `4003` | api | `instance_limit_reached` | The runtime instance registry is full. |
| `4004` | api | `input_limit_exceeded` | Input length exceeds the configured byte maximum. |
| `4005` | api | `initial_memory_limit_exceeded` | Initial-memory entry count exceeds its maximum. |
| `4006` | api | `duplicate_initial_memory_address` | Initial memory contains duplicate addresses. |
| `4007` | api | `invalid_configuration` | An execution setting is invalid; details identify the field. |
| `4008` | api | `unknown_program_handle` | Program handle is unknown, released, foreign or otherwise unavailable. |
| `4009` | api | `unknown_instance_handle` | Instance handle is unknown, released, foreign or otherwise unavailable. |
| `4010` | api | `zero_tick_budget` | A bounded run requests zero ticks. |
| `4011` | api | `run_tick_budget_exceeded` | The requested per-call tick budget exceeds the host maximum. |
| `4012` | api | `instance_tick_budget_exceeded` | The requested run exceeds the instance remaining total tick budget. |
| `4013` | api | `work_unit_budget_exceeded` | A step needs more dispatches than the per-call deterministic work ceiling. |
| `4014` | api | `handle_space_exhausted` | No new opaque handle can be allocated. |
| `4015` | api | `vm_initialization_failed` | VM initialization fails; reason identifies InitialThreadIdOverflow. |
| `5000` | cli | `cli.invalid_arguments` | Subcommand, option, option multiplicity, required argument, canonical numeric value or direct byte list is invalid. |
| `5001` | cli | `cli.source_io` | Source cannot be read as UTF-8, including unavailable files and invalid UTF-8. |
| `5002` | cli | `cli.input_io` | Input JSON file cannot be read. |
| `5003` | cli | `cli.input_json` | Input file does not decode as one valid JSON byte array. |
| `5004` | cli | `cli.input_data` | Decoded input fails host byte validation. |
| `5005` | cli | `cli.memory_io` | Initial-memory file cannot be read. |
| `5006` | cli | `cli.memory_json` | Initial-memory file does not decode as a valid JSON memory array. |
| `5007` | cli | `cli.memory_data` | Initial-memory addresses or entries fail validation, including duplicates. |
| `5008` | cli | `cli.serialization_failed` | A native run-result JSON cannot be serialized. |
| `5009` | cli | `cli.vm_initialization_failed` | Verified program cannot initialize its VM. |
| `5010` | cli | `cli.invalid_tick_limit` | The internal run boundary rejects a zero tick limit. |
| `5100` | debug | `debug.invalid_request` | JSON-lines request is malformed, has unknown fields, has wrong field types, or names an unsupported command. |
| `5101` | debug | `debug.invalid_configuration` | Seed/limit/boundary configuration is invalid or exceeds u64. |
| `5102` | debug | `debug.already_loaded` | A session receives another launch after a successful launch. |
| `5103` | debug | `debug.no_program` | Step or snapshot occurs before successful launch. |
| `5104` | debug | `debug.vm_initialization_failed` | The compiled program cannot initialize its VM. |
| `5105` | debug | `debug.tick_limit_exceeded` | The committed Global Tick count has reached the session maximum before another dispatch. |
| `5106` | debug | `debug.work_limit_exceeded` | An atomic Global Tick exceeds its dispatch ceiling and rolls back. |
| `5107` | debug | `debug.request_limit_exceeded` | A request line exceeds 4 MiB including its line ending. |
| `5108` | debug | `debug.transport_io` | The process cannot read a request or write a response. |
| `6000` | editor | `editor.operation_failed` | An otherwise unclassified editor operation fails, including document access or platform exceptions. |
| `6001` | editor | `editor.unsupported_debug_protocol` | Native debug protocol version is not 2. |
| `6002` | editor | `editor.invalid_debug_response` | Native response is malformed JSON or has an invalid structured error. |
| `6003` | editor | `editor.runtime_start_failed` | The executable cannot be spawned. |
| `6004` | editor | `editor.runtime_exited` | The runtime process exits while an operation is pending. |
| `6005` | editor | `editor.transport_io` | Native process pipe read/write fails. |
| `6006` | editor | `editor.session_closed` | A pending request is cancelled when the session is disposed. |
| `6007` | editor | `editor.invalid_expression` | Watch/evaluate expression is not a read-only state path. |
| `6008` | editor | `editor.unknown_state_path` | A requested read-only property/index does not exist. |
| `6009` | editor | `editor.no_caller_frame` | Step Out is requested without a selected caller frame. |
| `6010` | editor | `editor.unsupported_debug_request` | The DAP request is not supported. |
| `6011` | editor | `editor.workspace_untrusted` | Run/debug is attempted in an untrusted workspace. |
| `6012` | editor | `editor.invalid_program` | Launch does not provide a nonempty source-file path. |
| `6013` | editor | `editor.invalid_input` | Input is not an array of byte integers in 0 through 255. |
| `6014` | editor | `editor.source_errors` | Compilation rejected the source; individual diagnostics retain their source/IR codes. |
| `6015` | editor | `editor.no_session` | Stepping or continuing has no active native runtime. |
| `6016` | editor | `editor.session_running` | A step/continue request occurs while the session is already running. |
| `6017` | editor | `editor.vm_terminal` | Execution is requested after VM halt/error. |
| `6018` | editor | `editor.no_codegrid_file` | Run/Debug command has no current .cg document. |
| `6019` | editor | `editor.lsp_path_empty` | The configured language-server executable path is empty. |
| `6020` | editor | `editor.lsp_start_failed` | The optional language server fails to start. |
| `6021` | editor | `editor.invalid_template` | New-file command receives an unknown template identifier. |
| `7000` | browser | `invalid_source` | Source payload is not a JavaScript string. |
| `7001` | browser | `invalid_input` | Input is not a Uint8Array. |
| `7002` | browser | `input_allocation_failed` | Input buffer cannot be reserved before copying. |
| `7003` | browser | `request_allocation_failed` | A request buffer cannot be reserved. |
| `7004` | browser | `source_payload_limit_exceeded` | Source UTF-8 bytes exceed the adapter payload ceiling. |
| `7005` | browser | `input_payload_limit_exceeded` | Input bytes exceed the adapter payload ceiling. |
| `7006` | browser | `request_payload_limit_exceeded` | Request JSON/strings or combined request bytes exceed the adapter ceiling. |
| `7007` | browser | `invalid_request_field` | A required request field has the wrong JavaScript type. |
| `7008` | browser | `invalid_program_handle` | Program handle spelling is not a canonical nonzero u64 decimal string. |
| `7009` | browser | `invalid_initial_memory` | Initial-memory JSON has an invalid shape or invalid entry. |
| `7010` | browser | `invalid_configuration` | Configuration is invalid under the runtime contract. |
| `7011` | browser | `invalid_request` | A request value cannot be accepted by the binding. |
| `7012` | browser | `invalid_instance_configuration` | Configuration JSON has an invalid shape or required fields. |
| `7013` | browser | `invalid_instance_handle` | Instance handle spelling is not a canonical nonzero u64 decimal string. |
| `7014` | browser | `invalid_tick_limit` | Requested tick budget is not a positive canonical u64 decimal string. |
| `7015` | browser | `instance_state_limit_exceeded` | Canonical retained snapshot bytes exceed the configured instance quota. |
| `7016` | browser | `invalid_memory_address` | An initial-memory address is not a canonical arbitrary-precision signed decimal string. |
| `7017` | browser | `invalid_boundary_mode` | Boundary mode is neither exit nor wrap. |
| `7018` | browser | `invalid_configuration_integer` | Seed or Custom limit is not a valid canonical integer string. |
| `7019` | browser | `response_payload_limit_exceeded` | The full response cannot fit the configured response ceiling. |
| `7020` | browser | `browser.invalid_host_limit` | BrowserRuntime constructor receives invalid numeric/string ceilings or host limits. |
| `7500` | server | `invalid_input` | Input is not a JSON array. |
| `7501` | server | `invalid_input_byte` | An input element is not an exact byte integer. |
| `7502` | server | `invalid_host_limit` | A required host ceiling is invalid or nonpositive where required. |
| `7503` | server | `host_limit_exceeds_adapter_ceiling` | A configured host limit exceeds the adapter hard ceiling. |
| `7504` | server | `invalid_decimal_integer` | A required u64 string is noncanonical or out of range. |
| `7505` | server | `invalid_memory_address` | Initial-memory address spelling is invalid. |
| `7506` | server | `invalid_request_field` | Required request field is missing or has the wrong type. |
| `7507` | server | `unsupported_api_version` | Request API version differs from 3. |
| `7508` | server | `source_payload_limit_exceeded` | Source bytes exceed the runtime source ceiling. |
| `7509` | server | `input_payload_limit_exceeded` | Input bytes exceed the configured ceiling. |
| `7510` | server | `vm_initialization_error` | VM initialization fails (server alias of vm_initialization_failed). |
| `7511` | server | `invalid_initial_memory` | Initial-memory payload is not an array. |
| `7512` | server | `initial_memory_payload_limit_exceeded` | Initial-memory payload/entry collection exceeds the adapter ceiling. |
| `7513` | server | `invalid_initial_memory_entry` | An initial-memory entry has invalid fields, address, or value. |
| `7514` | server | `request_payload_limit_exceeded` | Request bytes exceed the adapter request ceiling. |
| `7515` | server | `invalid_request_json` | Request bytes are not one valid UTF-8 JSON value. |
| `7516` | server | `invalid_request` | Top-level request is not a valid object. |
| `7517` | server | `unsupported_abi_version` | Request ABI version differs from 4. |
| `7518` | server | `runtime_closed` | A request targets a permanently shut-down module session. |
| `7519` | server | `runtime_not_initialized` | A runtime operation occurs before initialization. |
| `7520` | server | `unsupported_operation` | The requested operation is not recognized. |
| `7521` | server | `runtime_already_initialized` | Initialize is called twice. |
| `7522` | server | `invalid_program_handle` | Program handle spelling is invalid. |
| `7523` | server | `invalid_boundary` | Boundary mode is neither exit nor wrap. |
| `7524` | server | `invalid_instance_handle` | Instance handle spelling is invalid. |
| `7525` | server | `invalid_host_limits` | Initialization host_limits is not an object. |
| `7526` | server | `response_payload_limit_exceeded` | The full response exceeds the configured response ceiling. |
| `7527` | server | `instance_state_limit_exceeded` | Canonical retained instance state exceeds its configured quota. |
| `7528` | server | `program_limit_reached` | The runtime compiled-program registry is full. |
| `7529` | server | `instance_limit_reached` | The runtime instance registry is full. |
| `7530` | server | `initial_memory_limit_exceeded` | Initial-memory entry count exceeds its maximum. |
| `7531` | server | `duplicate_initial_memory_address` | Initial memory contains duplicate addresses. |
| `7532` | server | `invalid_configuration` | An execution setting is invalid; details identify the field. |
| `7533` | server | `unknown_program_handle` | Program handle is unknown, released, foreign or otherwise unavailable. |
| `7534` | server | `unknown_instance_handle` | Instance handle is unknown, released, foreign or otherwise unavailable. |
| `7535` | server | `zero_tick_budget` | A bounded run requests zero ticks. |
| `7536` | server | `run_tick_budget_exceeded` | The requested per-call tick budget exceeds the host maximum. |
| `7537` | server | `instance_tick_budget_exceeded` | The requested run exceeds the instance remaining total tick budget. |
| `7538` | server | `work_unit_budget_exceeded` | A step needs more dispatches than the per-call deterministic work ceiling. |
| `7539` | server | `handle_space_exhausted` | No new opaque handle can be allocated. |
| `8000` | lsp | `-32600` | Invalid JSON-RPC request or lifecycle operation. |
| `8001` | lsp | `-32601` | Unknown/unsupported method. |
| `8002` | lsp | `-32602` | Method parameters are invalid. |
| `8003` | lsp | `-32002` | Server is not initialized for the requested operation. |
| `8004` | lsp | `lsp.transport_io` | LSP framing, parsing or process I/O fails. |
| `8005` | lsp | `lsp.abnormal_exit` | Exit or EOF occurs without the required shutdown lifecycle. |
| `8100` | server-abi.alloc_buffer | `0` | Allocation length/pool/resource reservation fails; returned pointer is zero. |
| `8101` | server-abi.free_buffer | `0` | The pointer is unknown; release returns zero. |
| `8102` | server-abi.process_request | `0` | Pointer/length validation or response reservation/write fails; packed response is zero. |
| `9000` | level | `level.invalid` | Invalid logical JSON/schema/capability/metric; no validated level is returned. Details are typed reason and field path. |
| `9001` | level | `level.unsupported_format_version` | Unsupported logical format; payload is not interpreted as v1. |
| `9002` | level | `level.unsupported_scene_type` | No registered production scene; execution never falls back to ExactIO. |
| `9003` | level | `level.program_rejected` | Initial structural or committed generated capability violation; evaluation ends without rating. |
| `9004` | level | `level.evaluator_fault` | Reserved umbrella for future or otherwise unclassified evaluator fault categories; known v1 fault kinds use more specific registered codes. |
| `9005` | level | `level.resource_limit` | Trusted evaluation tick/work/state/output ceiling prevents completion; no official success. |
| `9006` | level | `level.cancelled` | Evaluation cancelled; no official success. |
| `9007` | level | `level_api.invalid_profile` | Malformed, duplicate, unknown, zero, or unsupported host numeric/identity data; session not created. |
| `9008` | level | `level_api.unsupported_profile_version` | Unsupported trusted profile version; session not created. |
| `9009` | level | `level_api.unsupported_version` | Unsupported API version; no semantic dispatch. |
| `9010` | level | `level_api.invalid_request` | Malformed, duplicate, unknown, incorrect-type request or unsupported operation; no semantic dispatch. |
| `9011` | level | `level_api.invalid_configuration` | Unsupported mode/boundary or noncanonical/zero required integer; no evaluation starts. |
| `9012` | level | `level_api.seed_required` | Seed omitted without an explicit host seed source; no evaluation starts. |
| `9013` | level | `level_api.invalid_handle` | Released, stale, wrong-kind, or other-session handle; no referenced operation executes. |
| `9014` | level | `level_api.handle_exhausted` | Checked handle/namespace capacity exhausted; no new handle is created. |
| `9015` | level | `level_api.resource_limit` | Request/source/program/retained-state/handle/per-call ceiling exceeded; no requested operation starts. |
| `9016` | level | `level_api.response_too_large` | A complete result/diagnostic response exceeds the ceiling; return a complete error, never partial JSON. Evaluation may already have advanced. |
| `9017` | level | `level_api.shutdown` | Operation attempted after shutdown; state remains released. |
| `9018` | level | `level_api.fault` | Shared API produced an invalid transport response; infrastructure failure, never player success. |
| `9019` | level | `level_abi.unsupported_version` | Unsupported portable ABI version; no dispatch. |
| `9020` | level | `level_abi.already_initialized` | Attempt to replace an initialized immutable profile; current session unchanged. |
| `9021` | level | `level_abi.not_initialized` | Semantic request before trusted initialization; no dispatch. |
| `9022` | level | `level.metric_overflow` | Checked level metric or aggregate arithmetic overflows during evaluation. |
| `9023` | level | `level.vm_fault` | The shared VM reports a fault during level evaluation. |
| `9024` | level | `level.vm_initialization_fault` | The shared VM cannot initialize a standard state for an evaluation test. |
| `5011` | cli | `cli.limits_io` | The trusted safety profile file cannot be opened or read. |
| `5012` | cli | `cli.profile_too_large` | The trusted safety profile exceeds the fixed CLI profile-file ceiling. |
| `5013` | cli | `cli.profile_utf8` | The trusted safety profile file is not valid UTF-8. |
| `5014` | cli | `cli.profile_bom` | The trusted safety profile begins with a forbidden UTF-8 byte-order mark. |
| `5015` | cli | `cli.level_api_protocol` | The shared Level API response is invalid JSON or violates the response shape expected by the CLI. |
| `5016` | cli | `cli.unknown_evaluation_status` | The shared Level API returned an unrecognized terminal evaluation status. |
| `5017` | cli | `cli.malformed_evaluation_result` | The shared Level API terminal result omitted its status. |
| `5018` | cli | `cli.level_io` | The evaluate command cannot open or read the level file. |
| `5019` | cli | `cli.level_too_large` | The level JSON exceeds the trusted profile's level-byte ceiling. |
| `5020` | cli | `cli.source_too_large` | The source file exceeds the trusted profile's source-byte ceiling. |
| `5021` | cli | `cli.level_utf8` | The level file is not valid UTF-8. |
| `5022` | cli | `cli.source_utf8` | The evaluate command's source file is not valid UTF-8. |
| `5023` | cli | `cli.level_bom` | The level file begins with a forbidden UTF-8 byte-order mark. |
| `5024` | cli | `cli.source_bom` | The evaluate command's source file begins with a forbidden UTF-8 byte-order mark. |
| `9025` | level | `level.wrong_output` | Committed output differs from the expected byte stream. |
| `9026` | level | `level.incomplete_output` | Execution terminates before all expected bytes are produced. |
| `9027` | level | `level.test_failed` | An ExactIO correctness check fails. |
| `9028` | level | `level.runtime_error` | An ExactIO test encounters a VM runtime error; public details follow visibility rules. |
| `9029` | level | `level.constraint_exceeded` | A permitted metric exceeds a configured level constraint. |
