# Full Language Decisions and Open Questions

## Sources of truth

- [`spec/codegrid-source-spec.md`](../spec/codegrid-source-spec.md) is the normative Full authority for source syntax and static acceptance. Source-language choices are resolved below, and the source/compiler plus IR implementation gates have focused conformance coverage.
- [`spec/codegrid-vm-spec.md`](../spec/codegrid-vm-spec.md) is the Full execution authority. The choices in the table below resolve its former implementation gates; direct conformance coverage remains required.
- [`spec/codegrid-runtime-api-spec.md`](../spec/codegrid-runtime-api-spec.md) defines the normative host-neutral Full contract in v3. Browser binding and server ABI use separately versioned transport contracts.
- Historical compiler, IR, and VM tests plus retained Full examples are evidence to review, not automatic replacements for normative rules.

## Stable error identifiers (2026-09-30)

Explicit user decision: complete stable error codes and publish their full specification under `spec`. Validators assign source/IR codes at detection; adapters forward the codes. Diagnostic `code` is additive under CLI run-result schema 1, Runtime API v3 and Server ABI v4. Existing VM codes and WASM aliases remain compatible. Native debug errors change from a string to `{code,message}`, advancing the protocol to 2 and the editor to 0.3.0. No source acceptance, VM transition, error ordering or rollback rule changes. See the [error specification](../spec/codegrid-error-codes.md) and its JSON registry.

## VS Code interface languages (2026-09-30)

Explicit user decision: localize the VS Code extension for the game's ten
languages. The supplied nine translations are German, French, Spanish (Spain),
Traditional Chinese, Japanese, Simplified Chinese, Korean, Portuguese (Brazil),
and Russian; English is the default tenth language. Editor 0.4.0 follows the
VS Code display language using manifest and runtime catalogs. Translation is a
presentation concern: language acceptance, VM transitions, protocol fields,
watch paths, and stable error identities remain unchanged. Native compiler/LSP
messages retain their original English text. See the
[editor localization contract](../editors/vscode/l10n/README.md).

## Editor error presentation and formatting (2026-09-30)

Explicit user requests: make the extension introduction multilingual, show a
stable identity and a localized message for exceptions, and align the reported
5x5 board during document formatting. Editor 0.4.2 adds ten-language README
introductions and translations for all 119 source, IR, VM, fault, CLI, native
debug and editor identities used at native/editor boundaries. Native messages
and serialized VM state remain unchanged. The editor translates by identity,
preserves original diagnostics as related information, and exposes VM exception
codes in DAP stop events. Unknown identities receive a localized fallback.

The standalone formatter now recognizes named closing directives such as
`@end main`, which previously caused a safe decline of the entire document.
It preserves directive comments and aligns columns by their longest cell token,
including attached spellings such as `+x3`. Source acceptance and VM semantics
are unchanged.

## Established Full scope

| Topic | Decision | Evidence and follow-up |
| --- | --- | --- |
| Language surface | Full includes Main, Function, Custom, and Folded Block definitions; all Primary instructions; ReadCode, WriteCode, and Repeat Attachments; and the associated state and execution semantics. | Canonical inventory in [`codegrid-model`](../crates/codegrid-model/src/lib.rs) and the Full source contract. |
| Board structure | Support scoped Function/Folded Block paths, including definitions nested in Custom CodeGrids. Main and Custom Main contexts may have multiple Entries; each Function Board has exactly one Entry. | Source syntax and Entry-count rules are normative in the Full source specification. Board execution and resume behavior belong to the Full VM specification. |
| Folded Blocks | A Folded Block is one row high and matches its owner's width; entry starts at internal position (0,0) facing Right, independent of the caller direction. Execution wraps horizontally and uses the specified vertical exit/Resume behavior. | The historical `concurrent_outer_folded_threads_keep_control_and_register_state_isolated` case confirms independent internal direction and saved caller direction; the Full VM specification now states this explicitly. |
| Concurrent tick | Threads evaluate from the same tick-start state. Their private state and shared effects are staged, conflicts are resolved deterministically, and the outer tick commits atomically or rolls back. Thread order is not a semantic write priority. | The Full VM specification defines the complete conflict, rollback, and ordering rules; direct integration coverage remains required. |
| Calls and Repeat | Function CALL/RETURN and caller Resume behavior consume their specified ticks. Qualifying tail recursion reuses a frame. Repeat spreads Primary executions across ticks and moves after its final execution. | Execution timing, tail-call eligibility, and Repeat behavior are normative in the Full VM specification; direct conformance cases remain required. |
| Custom | Each Custom invocation starts with isolated context/state, runs synchronous internal ticks within the outer transaction, participates in caller-stack transactions, and is bounded by a host Custom execution limit. | Error priority, seed derivation, nested-call, rollback, and internal tick behavior are normative in the Full VM specification; direct conformance cases remain required. |
| Determinism | Seeded random direction and derived thread/invocation streams follow one specified algorithm and stable derivation order. Metrics and conflict handling must not depend on host scheduling. | The Full VM specification defines the algorithm and golden vectors; native and host execution comparisons remain required. |
| IR trust boundary | The VM accepts only verifier-produced Full IR. Deserialized or externally supplied IR is revalidated before execution. | Keep the existing architectural invariant while extending validation to every Full structure and instruction. |
| Local versus game policy | Language, compilation, VM, CLI, editor, Runtime API, and WASM adapters are local repository scope. Levels, scoring, UI, Steam, telemetry, and game policy remain outside the language core. | Root architecture rules. |

## Resolved source-language decisions

| Topic | Decision | Evidence and implementation requirement |
| --- | --- | --- |
| Attachment compatibility | ReadCode and WriteCode are valid on every Primary with a normative Instruction Code. Repeat is valid on those Primary instructions except CALL and RETURN. Folded Block calls, Custom calls, Custom returns, and HALT are not encodable and accept no Attachment. | `compiles_every_resolved_attachment_pair_for_each_encodable_primary` covers compiler acceptance; `verifies_the_complete_full_primary_attachment_compatibility_matrix` covers the IR verifier; `rejects_invalid_attachment_placement_and_repeat_counts` covers detached, Entry, non-encodable, and invalid Repeat cases. The compiler and IR tests use the same `is_encodable()` rule and CALL/RETURN exclusion. |
| Portable board geometry | Width and height are each in `1..=u32::MAX`; checked width × height is at most `u32::MAX` cells. The same source-level bound applies on native and `wasm32`; embedding hosts may impose lower resource ceilings. | `rejects_dimensions_and_total_cells_above_the_portable_source_limit` and `rejects_board_dimensions_and_cell_counts_above_portable_full_bounds` cover source compilation and decoded-IR verification with the shared portable limit. |
| Size placement | A lexical-scope `@size` may occur between that scope's grid rows. It applies to the whole scope and does not terminate or split the row sequence. Grid rows still must remain contiguous apart from size directives/comments and must precede nested structural definitions. | `size_between_rows_applies_to_the_complete_lexical_scope` verifies rows on both sides of `@size` form one board and that the declared size applies to the whole scope. |
| Implicit Main with Folded Blocks | An implicit Main may define local `@M<n>` or its equivalent `@main.M<n>` after its grid begins. It may not define Functions or Customs. The first non-size structural definition ends the top-level Main row sequence; later top-level rows are invalid. | `compiles_implicit_main_with_only_board_local_folded_blocks`, `implicit_main_accepts_qualified_folded_blocks_after_its_grid`, and `implicit_main_requires_an_explicit_block_when_function_or_custom_is_defined` cover the accepted relative/qualified forms and rejection boundary. |
| Named `@end` aliases | A close name omits `@` and is case-insensitive. Main accepts `main`; Custom accepts its local `C<n>`; Function/Folded Block accepts its local final segment or complete qualified path. Bare `@end` remains valid. No other aliases are accepted. | `accepts_named_end_aliases_for_custom_and_function_folded_blocks` covers local and qualified closes for Custom Main, outer Function, and Custom Function Folded Blocks; `rejects_mismatched_custom_and_function_folded_block_end_aliases` checks mismatches for those ownership contexts. `accepts_case_insensitive_local_and_qualified_end_names` covers Main, Custom, and Function aliases. |

## Resolved VM semantic decisions

| Topic | Full rule | Evidence and implementation requirement |
| --- | --- | --- |
| CustomReturn completion | `#]` terminates only the executing Custom internal thread. A Custom invocation returns normally after every internal Entry thread has terminated through `#]`; other threads continue on later aligned internal ticks. A same-tick Custom Halt takes precedence over normal return, and any runtime error takes precedence over Halt. | Direct Full tests `custom_invocation_waits_for_returns_from_every_internal_thread` and `custom_threads_returning_on_the_same_tick_complete_together` cover staggered and simultaneous returns. `custom_halt_takes_priority_over_same_tick_custom_return`, `custom_internal_error_takes_priority_over_a_same_tick_halt_request`, and `custom_execution_limit_rolls_back_the_caller_stack_transaction` cover priority and the limit. Covered by active VM tests. |
| CALL attachments | A CALL's ReadCode or WriteCode Attachment is deferred until the caller's AfterCall tick, after the callee returns successfully. It executes once against the CALL site's tick-start mutable Primary, then the caller advances. It does not execute if the call halts, errors, or otherwise fails before returning. | `calls_consume_a_tick_and_return_uses_a_separate_resume_tick` and `call_write_code_runs_after_return_using_the_callees_instruction_stack` cover the two attachment types; `recursive_callee_aftercall_uses_the_current_mutable_call_site` observes consecutive recursive AfterCall writes against the updated CALL cell; `deferred_call_attachments_do_not_run_when_the_callee_halts_or_errors` checks both attachment types against halt and error exits. Covered by active VM tests. |
| RETURN attachments | ReadCode and WriteCode attached to RETURN are accepted by the source/IR compatibility matrix but have no runtime effect and contribute no attachment operation metric. RETURN itself still executes and is counted normally. | The CALL timing test observes inert RETURN ReadCode; `write_code_attached_to_return_is_inert` asserts the Primary, Instruction Stack, operation count, and Instruction Variety for RETURN WriteCode. Covered by active VM tests. |
| ReadCode of Empty | Preserve the defensive result that an Empty tick-start Primary would push the EMPTY Instruction Stack item (Instruction Code 32). This state is unreachable in verified IR: each cell has one fixed Attachment, attachments require an initial Primary, and WriteCode changes only the Primary while retaining its Attachment. | `validate_cell` enforces the initial structure, and the VM writes only the Primary field. Active tests cover ReadCode of an executable Primary, WriteCode clearing while retaining its Attachment, and DECODE/ENCODE of Code 32. The impossible post-mutation ReadCode case remains intentionally excluded; VM spec §17 records the reachable acceptance gates. |
| Empty POPADD and NAND | Each is a counted no-op when the executing thread's Data Stack is empty: preserve the selected register and stack, then perform ordinary movement. | `empty_popadd_and_nand_are_counted_noops` directly asserts preserved register/stack, each instruction's movement, operation count, and variety; nonempty behavior is covered by `nonempty_popadd_updates_register_and_consumes_one_stack_value` and `nand_complements_the_bitwise_and_of_register_and_stack_values`. Covered by active VM tests. |
| Empty Custom READ | Preserve the selected Custom register and set the instruction's encoded direction, as outer READ does when its input source is exhausted. The empty caller stack causes no successful caller-stack read, register write, or read conflict. | `empty_custom_read_preserves_register_and_turns_before_later_output` checks preserved value, direction, and movement; `exhausted_custom_read_can_coexist_with_one_caller_stack_output` and `custom-empty-concurrent-reads-coexist-with-output` cover coexistence. Nonempty reads and conflict cases have separate direct tests. Covered by active VM tests. |
| Conflict matrix and error order | Adopt the exact conflict rules in VM Spec §§10.1–10.2, including rejected reads not producing secondary register conflicts, same-value writes still conflicting, and the specified cross-effect cases. Normalize participant IDs and sort errors by scope, error-code name, resource-key variant/fields, and participant IDs. | `effects.rs` covers rejected input/caller reads, empty reads, equal writes, memory/code identity, and caller-stack combinations; `error.rs` covers canonical scope/code/resource/participant ordering. Active VM integration tests cover rollback and several resource combinations, including new successful input-read/register-write and Custom caller-read/register-write cases. **Partial:** there is not yet one direct Full integration matrix covering every cross-effect combination; keep this gate open. |
| Event ordering | Emit committed events only after a successful outer-tick commit. Order all CellReached events first by outer thread ID, with that thread's outer visit before its Custom visits; order Custom visits by internal tick then internal thread ID. Then order Custom state events by internal tick, caller thread ID, and Custom ID, followed by Outer state events. Within each context tick, order event categories as InputConsumed, RegisterChanged, MemoryChanged, CodeChanged, ThreadChanged; order values within each category by thread ID, register index, resource key, cell key, or thread ID as applicable. | `step_events_report_input_and_memory_commits_with_scope` checks successful event order; `simultaneous_custom_state_events_are_ordered_by_aligned_internal_tick` checks the Custom visit prefix, aligned register event order, and Custom-before-Outer state ordering. Failed conflict tests assert empty committed events and canonical error order. **Partial:** the complete cross-category multi-thread/Custom event sequence does not yet have one full golden assertion; keep this gate open. |
| Stack high-water metrics | Sample aggregate depths at outer-tick boundaries and at each aligned Custom internal-tick boundary. Sum the depths of all resident Outer and Custom threads at that instant; count each Outer caller stack once. Before commit, include staged pushes and call-frame growth without allowing staged pops to reduce the attempted high-water mark. On successful commit, also sample the committed post-state. Preserve attempted peaks through runtime-error rollback; a host work-limit interruption adds no metric. | `failed_aligned_custom_tick_preserves_integrated_stack_high_water_formula` directly covers all three stack classes across Outer and two aligned Custom threads, caller-stack replacement, staged growth/pops, and full failed-tick rollback with attempted peaks retained. `failed_aligned_custom_tick_preserves_the_aggregate_stack_high_water_formula` independently checks helper arithmetic and metric merging. Covered by active VM tests. |

## Resolved native CLI contract

The local native CLI contract is specified in [`docs/cli.md`](cli.md). Its command spellings, input formats, JSON schema, exact-integer encoding, output streams, and process exit codes are decisions for the Full local implementation, not unresolved adapter choices.

| Topic | Decision | Follow-up |
| --- | --- | --- |
| Commands and bounds | Keep `check <program.cg>` and `run <program.cg>`. `run` requires `--boundary`, `--seed`, `--custom-limit`, `--max-ticks`, and `--max-work-units`; tick, Custom, and work limits are positive `u64` values. Input is optional and comes from exactly one of `--input` or `--input-file`; initial Outer memory is optional via `--initial-memory-file`. | Implemented in `codegrid-cli`; focused argument and all shared Full fixture tests pass. |
| Input representation | Inline input is a comma-separated byte list; file input is a JSON byte array. Initial memory is a JSON array of unique `{address, value}` entries; addresses are canonical signed decimal strings and values are bytes. Zero-valued initial entries are accepted and normalized away. | Implemented and covered for both input forms, malformed values, duplicate addresses, empty input, wide addresses, and memory normalization. |
| Run result | Use `schema: "codegrid.cli.run-result"`, `schema_version: 1`, with distinct `source_error`, `halted`, `runtime_error`, `yielded`, and `vm_fault` outcomes, complete Full snapshots, committed events/output, and the effective configuration. | Implemented with full snapshots, events, metrics, errors, yields, and deterministic JSON checks; this schema remains independent from Runtime API v3 and adapter wire schemas. |
| Wide integers and ordering | Encode every `u64`, arbitrary-precision signed integer, and UTF-8 source byte offset as a canonical decimal string. Keep bytes and bounded values as exact JSON numbers. Serialize identity collections in canonical VM order. | Implemented and covered by wide seed/address and shared Full conformance checks. |
| Process behavior | `check` uses human-readable output. `run` emits one complete JSON result on stdout whenever source reaches the compiler; human diagnostics go to stderr. Exit codes are fixed: 0 success/halt/help, 2 invalid arguments or host data, 3 I/O or source decoding, 4 source rejection, 5 VM runtime error, 6 tick-slice yield, 7 work-unit yield, and 8 VM fault. | Implemented and exercised by focused CLI tests; yields and VM faults remain distinct from normal halt and runtime errors. |

## Resolved server Runtime API adapter contract

The no-import server adapter uses its own Server ABI v4 over host-neutral
Runtime API v3. ABI v3's reduced request and snapshot fields are incompatible
with Full v3, so the server adapter version is bumped independently; Runtime
API versioning does not version the byte ABI. Requests and responses carry both
`abi_version: 4` and `api_version: 3`.

The v4 request layout uses flat operation fields. `initialize` takes immutable
trusted `host_limits`, including `max_initial_memory_entries`. `create_instance`
takes a program handle, byte-array input, `boundary_mode`, canonical decimal
string `seed`, positive decimal string `custom_execution_limit`, and optional
`initial_memory` entries with canonical signed decimal string addresses and
byte values. `program_view` exposes canonical compiler output by existing
program handle. `step`, `run`, `snapshot`, release, and shutdown operations map
to the equivalent Runtime API v3 operations. The adapter never accepts IR or
duplicates compiler or VM semantics.

The full `step`, `run`, and `snapshot` projections include complete thread
state, Page, stacks, call frames, phase, RNG state, mutable Outer code, sparse
memory, raw metric identities/counters, committed events, errors, and optional
faults. `run` maps Runtime API v3 yield reasons to the stable top-level strings
`tick_limit_reached` and `work_limit_reached`; both include a full snapshot.
API failures, source diagnostics, VM errors, faults, and yields stay distinct.
Every `u64`, handle, tick, counter, PRNG state, and arbitrary-precision signed
integer uses a canonical decimal string; bytes and bounded geometry remain
exact JSON numbers. Errors use a versioned envelope with stable codes and
structured details. Requests/responses and retained state are bounded, and
oversized responses are rejected as complete errors rather than partial JSON.

| Topic | Decision | Follow-up |
| --- | --- | --- |
| Server ABI lifecycle | Keep the existing no-import buffer exports and per-module session, but version its Full JSON contract as ABI v4, independent of Runtime API v3. A module session has immutable host-selected limits and explicit `initialize`/`shutdown` lifecycle. | Implemented; native tests and the Node WebAssembly smoke pass export shape, buffer lifecycle, and session isolation checks. |
| Server Full projection | Serialize the complete v3 snapshot, event, error, fault, metric, and program-view data through bounded projections. Keep large integers exact and distinguish work-limit yield from tick-slice yield. | Implemented; all 63 shared Full runtime cases pass in native adapter tests and the Node WebAssembly smoke. |

## Completed local verification and out-of-scope policy

- The same Full conformance fixtures were compared in the native CLI, actual browser-WASM hosts (Node and headless Chrome), and actual server-WASM host (Wasmtime 49.0.1 using Server ABI v4 / Runtime API v3). All 63 cases match the complete Native CLI result projection; the host lifecycle, memory, stack, fuel, work-limit, and buffer probes also pass.
- Production authentication/authorization, deployment quotas, process/isolate resource controls, cancellation, and release/distribution policy are outside the local language-development goal.

Adapter schemas, local resource ceilings, and wide-integer representations are specified in the respective versioned host contracts. Do not infer production suitability from local limits.

Production authentication/authorization, multi-user quota policy, and deployment operations are outside the local development scope and are not gates for the Full local CLI or language core.
