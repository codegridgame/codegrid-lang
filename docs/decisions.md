# Full Language Decisions and Open Questions

## Robot author-map revision (2026-10-05)

The user selects a terrain/object/test separation and resets the scene author
format to v1 without a legacy compatibility requirement. Terrain is 16 rows of
16 ASCII characters: `.` VOID, `0` low ground, and `1` high ground. Initially
all terrain is VOID; authors must draw usable ground. Sparse colors preserve
the existing NONE/BLUE/RED protocol, with omitted ground colors defaulting to 0.
Sparse objects use index = y * 16 + x. start, patrol, trigger, and door are
mutually exclusive and object indices are globally unique within objects.
All colors, objects, and starts must address drawn ground. device is excluded.
One trigger and one door with the same integer ID form a mechanism; no explicit
reference is stored. Patrol IDs are a separate namespace. Static starts are
map objects; Robot tests carry visibility only. No single-test restriction was
approved. This decision supersedes earlier dense maps, combined patrol/trigger
cells, explicit door references, and default-walkable proposals. The v1
contract, loader, examples, and fixtures now use these rules; direct protocol
and complete host conformance remain tracked separately.

## Robot sparse-map validation identities (2026-10-05)

The Scene Host Contract v2 reason table now follows the selected sparse-map
shape: dimensions/terrain, colors, and objects are validated separately;
VOID placement errors point at the sparse record index; repeated sparse indices
and same-type IDs point at the later record; and unmatched trigger/door IDs
point at the unmatched `.id`. `InvalidDirection` applies to an unsupported
direction string, while numeric range errors keep `IntegerOutOfRange`. These
are typed reason strings under the existing LevelInvalid identity and allocate
no new numeric error codes. The previous dense-cell-only reasons are superseded.

## Sources of truth

- [`spec/codegrid-source-spec.md`](../spec/codegrid-source-spec.md) is the normative Full authority for source syntax and static acceptance. Source-language choices are resolved below, and the source/compiler plus IR implementation gates have focused conformance coverage.
- [`spec/codegrid-vm-spec.md`](../spec/codegrid-vm-spec.md) is the Full execution authority. The choices in the table below resolve its former implementation gates; direct conformance coverage remains required.
- [`spec/codegrid-runtime-api-spec.md`](../spec/codegrid-runtime-api-spec.md) defines the normative host-neutral Full contract in v3. Browser binding and server ABI use separately versioned transport contracts.
- Historical compiler, IR, and VM tests plus retained Full examples are evidence to review, not automatic replacements for normative rules.

## Level Core architecture (2026-09-30)

Explicit user decision: use the [Level Core v1 specification](../spec/codegrid-level-core-spec-v1.md)
to design Rust level validation and evaluation, invoked through WASM by Web,
Steam, and backend hosts. The [architecture](level-core-architecture.md) places
`codegrid-level-core` above the unchanged language crates, with a separate
level API and browser/portable WASM adapters. These components are planned,
not implemented, and do not change the current Cargo dependency graph or
Runtime API v3 / Server ABI v4 contracts. Logical evaluation and scoring belong
to this upper layer; assets, Steam integration, and publication remain host concerns.

Resolve omitted seeds at the host boundary and pass explicit seed data to Rust;
this reconciles the specification's automatic-seed convenience with the
deterministic core rule. Backend Official evaluation uses trusted full levels
and freshly compiled submitted source. Client results cannot certify
server-authoritative leaderboard records; Steam achievements and offline
progress remain game-host policy. Hidden data shipped to clients is not secret.

Follow-up explicit user decision: align the architecture with the normative VM.
Only successfully committed output can complete tests/actions. Concurrent
outer output writes conflict; failed ticks roll back; runtime errors outrank
same-tick HALT. Tick/work exhaustion is a nonterminal yield, never a synthesized
VM `TickLimitExceeded` error. Interrupted work retries the entire tick, so
insufficient repeated budgets do not guarantee progress. Cumulative evaluation
safety termination requires its own host/evaluation contract. Level Core
sections 15 and 34 require normative clarification against these VM rules
before implementation; this change does not edit either specification.

### ExactIO first-phase contract (2026-09-30)

The user explicitly selected cost as VM Operation Count, explicit per-evaluation
Exit/Wrap configuration, first-phase WriteCode support, and enforcement of the
whitelist on generated code. The user then authorized the remaining recommended
choices. The [ExactIO implementation contract](../spec/codegrid-level-exactio-contract-v1.md)
records the schema, independent Attachment whitelist, capability mapping,
metric/constraint registry, shuffle recurrence, seed derivation, safety profile,
and terminal priorities. Level Core sections 15 and 34 are now clarified to
follow the VM's runtime-error and nonterminal yield rules.

Generated forbidden code is detected after successful commit, including
committed Custom code-change events, and ends the evaluation as ProgramRejected
before output acceptance. This supersedes the earlier conversational suggestion
to roll back a tick for a level policy violation; VM semantics remain unchanged.
Debug still runs all visible tests after ordinary test/constraint failures, but
program rejection or terminal host resource/fault outcomes end evaluation.

No implementation, host ABI, scene protocol, or host parity is claimed. Host
safety profile values are explicit versioned inputs; production defaults are
not selected by this decision.

### Level delivery task scope (2026-09-30)

Explicit user decision: prepare a task book for Rust level judgment, Web/Steam/
backend WASM invocation with matching results, and a CLI command reading level
JSON and `.cg` files and reporting evaluation/scoring. Design the Environment
extension fully but defer concrete scenes. The [task book](../tasks/level-core-exactio-v1.md)
records staged contracts, implementation, examples, safety/error preparation,
CLI and WASM delivery, and actual-host parity gates. Its CLI command spelling
is a proposal to publish before implementation; no existing CLI/ABI version or
dependency graph changes in this documentation step.

Schema details, instruction capability mapping, metric definitions, scene
protocols, deterministic shuffle/seed derivation, atomic completion priorities,
and level API/ABI/error contracts remain implementation gates listed in the
architecture. No source/VM semantics or level specification text is changed by
this architectural decision.

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

## Level delivery contracts (2026-09-30)

Level Host API, browser binding, and portable byte ABI start at independent
version 1; existing language Runtime API v3 and Server ABI v4 remain unchanged.
The [API contract](level-api-v1.md) and [transport contract](level-wasm-v1.md)
define shared operations and exact wire data. The named
[local profile](../fixtures/levels/profiles/local-v1.json) is verification input,
not a guessed production profile.

VM dispatch accounting is exposed as a neutral observation alongside the
existing atomic work-limited step. It counts actual attempts without changing
rollback, metrics, errors, or execution semantics. Retained-state accounting
uses explicit logical storage units and input-representation bytes, as defined
in [safety accounting](level-safety-accounting.md); physical heap ceilings remain
embedding-runtime responsibility. This resolves the implementation gap that a
borrowed VM snapshot cannot measure Rust allocator capacity reliably.

Environment extension interfaces are described in
[the design](level-environment-extension.md). No production scene is registered;
unsupported scene requests retain their dedicated error identity. Scene
transition priority remains a required normative scene decision before delivery.

## Level feedback and response safety refinement (2026-10-01)

Completion review found that a wire response ceiling alone did not bound retained
visible feedback and terminal results. Level API v1 now reserves an explicit
conservative encoded feedback/result allowance together with each evaluation's
input clones, charged against the existing trusted state-byte ceiling. The Rust
kernel receives the effective allowance as resolved configuration; no player
level field can change it. Before retaining each visible feedback record, the
kernel checks its bound. Terminal exhaustion remains ResourceLimitExceeded,
without official metrics or rating. Fixed-schema and per-record bounds are
published in [level safety accounting](level-safety-accounting.md).

Response projection checks these bounds before building JSON collections, and a
bounded writer prevents oversized serialized-buffer allocation. Trusted level
loading ceilings are host resource errors rather than malformed-level errors.
Committed generated-code rejection diagnostics retain trusted scope/cell/Primary
internally; only permitted privacy-safe reasons are projected to clients. These
changes refine host safety and diagnostics without changing source acceptance,
VM transitions, work units, correctness, metrics, or scoring.

## Level grouped permissions and defaults (2026-10-02)

An explicit user request adds Level whitelist groups IF_ZERO, READ,
REGISTER_POINTER, STACK, CODEC, MEMORY, PAGE, and SHIFT. STACK permits PUSH and
POP_ADD only; the user explicitly keeps NAND independent. The four direction
Primaries, OUTPUT, and HALT are always permitted. CALL includes RETURN and
CUSTOM includes CUSTOM_RETURN. RANDOM_DIRECTION, CLEAR, ADD, SUB, NAND, and
FOLDED_BLOCK remain independent. Existing individual identifiers remain accepted
for compatibility; groups and individuals form a union. Memory gating,
Attachment permissions, structural limits, source acceptance, and VM semantics
are unchanged. Initial and committed generated code share these permissions.
This recorded permission change retains format version 1 and does not change
static instruction_kinds scoring or VM dynamic Instruction Variety.

## Four-digit error identities (2026-10-01)

An explicit user request assigns stable four-digit decimal strings to all
registered toolchain errors. Registry v2 adds error_number to each scoped
identity. Published textual identifiers, VM spellings, transport aliases,
standard JSON-RPC codes and ABI zero sentinels remain compatible. Structured
outputs carry the number; process/editor messages show it alongside the
original identifier. LSP stores it in data and DAP uses it as the presentation
error ID. Numbers are never inferred from message wording, reassigned, or
recycled. Generated Rust/TypeScript lookup data and the Markdown table use
the JSON registry. Existing ExactIO failure outcomes also gain registered
presentation identities without changing execution, priority, or privacy.

## Five selected product scenes (2026-10-03)

The user selects ExactIO, Baudot, QualityControl, Elevator, and Robot from the
referenced design discussion; Terminal is excluded. ExactIO, Baudot, and
QualityControl share ExactIO correctness; Elevator and Robot require Environment
world-state evaluation. The Rust product catalog records selection separately
from executable registration. This does not add loader acceptance, change API
v1 capabilities, register MaintenanceRobot as an alias, or approve scene wire
protocols. See [five-scene architecture](level-scenes-architecture.md).

The conversation's proposed memory across Robot actions conflicts with the
normative fresh-VM-per-decision Environment contract. That lifecycle question
remains open; the current reset contract applies until explicitly revised.

## Immediate output Primaries (2026-10-03)

The user explicitly approves .0 through .9 as non-encodable Primaries that
output raw byte values 0 through 9 without changing registers or the register
pointer. They have no Instruction Codes and accept no Attachments, including
ReadCode, WriteCode, and Repeat. Ordinary . retains its existing behavior.
Immediate output uses ordinary output movement, tick/work accounting,
transactional effects, concurrent conflicts, and Custom caller-stack routing.
All forms share OUTPUT permissions and the Output metric kind; immediate
values do not create distinct instruction kinds. Existing Instruction Codes,
IR format version, and host envelope versions are unchanged. Malformed forms
and illegal Attachments retain existing structured validation identities.

## Conditional prefix migration design (2026-10-03)

The user explicitly selects removal of the four IfZero Primaries `#^`, `#v`,
`#<`, and `#>`. Their replacement is a prefix attachment spelled `?0`, `?1`,
or `?2`, requiring equality of the executing thread's selected register to
the corresponding byte. RandomDirection changes its source spelling from `?`
to `??`; Custom calls `#0` through `#9` and CustomReturn `#]` remain unchanged.

A cell may combine one prefix with one existing suffix Attachment. A false
condition skips both Primary and suffix, consumes a tick, and moves one cell
in the current direction. Every Primary may receive a prefix, including
Function and Custom calls/returns, Folded Block calls and otherwise valid
Folded Block contents, immediate output, and Halt. Initial Empty and Entry
cells cannot receive prefixes. Existing contextual restrictions and suffix
compatibility remain in force. This supersedes the preceding immediate-output
decision's Attachment prohibition only for the new prefix category; immediate
output still cannot carry ReadCode, WriteCode, or Repeat.

Repeat evaluates the prefix before each Primary execution. A false initial or
subsequent result ends the cell's repetition and advances in the current
direction. Prefixes are not new Primary instructions.

The [execution manual](conditional-prefix-migration.md) separates
confirmed requirements from pending register snapshot, resume, mutable-code,
metric, level permission, and compatibility decisions. This is a design record;
the existing normative specifications and implementation have not yet been
migrated. No new source acceptance, execution behavior, or host parity is claimed.

### Non-consuming CMP Primary

The user adds CMP with the source token `?=`. It reads A from the executing
thread's Data Stack top without popping and B from the register selected by
that thread's current pointer. CMP writes 0 when A equals B, 1 when A is greater
than B, and 2 when A is less than B to the same selected register. Comparison
uses the existing unsigned-byte value domain. The Data Stack is unchanged.
As a Primary, CMP is eligible for the approved conditional prefix category.
In `?1?=`, the prefix tests the register before CMP executes.

Empty-stack behavior, CMP Instruction Code assignment, suffix compatibility,
operation accounting, and level capability mapping remain pending in the
[execution manual](conditional-prefix-migration.md). This extends the design
scope only; CMP has not been implemented or added to normative specifications.

### Fixed ASCII-sum instruction numbering

The user clarifies that instruction numbers are fixed by the ASCII value of
a single symbol or the sum of the two symbols' ASCII values. Consequently,
RandomDirection `??` has number 126 and CMP `?=` has number 124. The former
recommendation to retain random code 63 after the token rename is superseded.
The prefix token sums for `?0`, `?1`, and `?2` are 111, 112, and 113; this does
not make prefix attachments independently executable Primaries.

The [manual's numbering audit](conditional-prefix-migration.md#ascii-sum-numbering-audit)
records duplicate sums among structural and immediate-output Primary forms.
These forms are currently excluded from byte Instruction Codes. Whether those
exclusions should change remains unresolved; the unique existing encodable
subset must not be presented as proof that all token sums are unique. This is
a design clarification, not an implementation or normative encoding change.

Follow-up user decision: retain the existing non-encodable forms. Duplicate
ASCII sums involving FoldedBlock, Custom, or immediate output therefore do not
create executable Instruction Code collisions. The proposed encoded subset,
including CMP 124 and RandomDirection 126, is unique. No collision-driven token
changes or expansion of encoding eligibility are required.

### Final approval of execution and migration rules

The user approves all remaining design choices in the execution manual:

- CMP on an empty Data Stack preserves the register, counts one CMP operation,
  and moves normally. CMP uses each executing thread's own Data Stack, including
  internal Custom threads. It supports ReadCode, WriteCode, and Repeat, counts
  one CMP runtime kind, and requires independent CMP level permission.
- A prefix reads the selected register from the context tick-start snapshot.
  CALL checks only before invocation; AfterCall and FoldResume do not recheck.
- WriteCode changes only Primary and preserves both prefix and suffix. ReadCode
  reads only Primary. Cleared cells still evaluate their fixed prefix before
  deciding whether to execute the retained suffix.
- Every evaluated prefix costs one operation, whether true or false. The three
  values share one runtime kind but have independent level permissions and
  count separately as static instruction kinds.
- New source rejects old IfZero tokens and standalone `?`. Executable IR gets
  a new format version. RandomDirection moves from code 63 to 126 and CMP uses
  124; 63 and removed IfZero codes 95, 97, 129, and 153 no longer decode. Other
  retained encodings remain unchanged, and existing non-encodable forms remain
  excluded. Existing invalid-byte DECODE behavior remains a counted no-op.

This approval resolves the preceding pending entries. The manual is finalized;
implementation, normative specification migration, and host execution parity
verification remain future work. Concrete capability names and required host
schema/version changes are implementation details to document consistently.

### Implementation projection and capability choices

Conditional permissions use CONDITION_0, CONDITION_1, and CONDITION_2 in
allowed_attachments; CMP uses independent CMP in allowed_instructions.
Runtime operation kinds are Condition and Compare. Prefix metadata is stored
separately from Primary and the existing suffix, including on Folded Block cells.

Normal code-view cells gain a nullable prefix field; folded code-view strings
include the canonical prefix. These additive read-only fields do not accept
executable IR from hosts. Runtime API v3, browser envelope, server ABI v4, CLI
JSON schema 1, debug protocol 2, and level format 1 remain unchanged; executable
IR alone advances to format 2. This records implementation choices within the
approved migration, not compatibility acceptance of the removed language.

## Scene protocol specification (2026-10-05)

The user requests a concrete repository specification based on the selected
conversation "梳理确定场景" (conversation ID
6ac2518f-10cc-83ec-98ae-a8e59bd6b639). The resulting
[Scene Specification v1](../spec/codegrid-scene-spec-v1.md) records six scenes:
ExactIO, Baudot, Elevator, Robot, QualityControl, and MechanicalArm.

This replaces the earlier five-scene product selection for planned work and
Robot's repair-device objective with required-patrol completion. New dynamic
protocols preserve one VM for the whole case and append observations to its
unread queue; reset all VM/scene state only between cases. This supersedes the
earlier fresh-VM-per-decision conceptual Environment lifecycle for these scenes.
MechanicalArm uses hidden true inspection, fixed state packing, delayed table
and buffer readiness, and may pack an inspected normal robot without Processing.

Only documentation changes are authorized in this task. Current strict Level
JSON, executable registrations, API/ABI versions, the five-entry Rust catalog,
and ExactIO execution are unchanged. The spec explicitly lists remaining
schema/session/error/projection decisions and acceptance cases; those must be
resolved before implementation relies on them. Existing VM no-op/error rules,
transaction semantics, resource categories, and hidden-test privacy remain
normative and are not replaced by informal examples in the conversation.

## Scene execution boundaries accepted (2026-10-05)

The user explicitly accepts all six recommendations from the Scene Spec review.
They are now normative in Scene Spec Sections 2 and 6: append observations after
each committed tick before the next tick; validate actors when dispatched in
A/B order and ignore B after terminal A; retain completed A effects on B failure;
use the explicit dynamic-scene terminal priority; use screen-style Robot axes,
modular turns, and forbid wall/door co-location; preserve the same VM and its
ordinary state for an entire case. Reset fully only between cases.

The session design records candidate round publication with all-or-none input
append, trusted accounting, replay and privacy boundaries. The conformance plan
provides concrete vectors and delivery gates. Typed author JSON, actual append
APIs, stable scene failure/metric transport, and concrete trusted-profile fields
remain implementation integration work. No source, VM implementation, loader,
API/ABI, or executable scene capability changes are delivered in this task.

## Scene author JSON v2 contract (2026-10-05)

The user requests completion of supporting documents after agreeing that the
scene author JSON must be specified before implementation. The selected
[Level JSON v2 contract](../spec/codegrid-scene-level-json-v2.md) adds explicit
scene_type/scene_config to the familiar envelope, uses ExactIO for static
profiles and Environment for dynamic scenes, and preserves v1 ExactIO loading.
All fields are explicit with no implicit defaults, strict unknown/duplicate-key
rejection, and domain/cross-field validation.

The contract selects dense 256-cell Robot maps with per-cell trigger/door
records, four fixed optional MechanicalArm worktable slots, ordered passenger
records, level-wide QC representation/mode, domain-limited Baudot bytes, and
complete per-scene test records. Elevator distance/stops aggregate with SUM
across visible cases; existing VM metrics retain their aggregation and scoring
rules. Case VMs use the existing root-seed VM derivation and never reseed per
action. Twelve complete author examples plus a manifest are documentation
fixtures, not current loader/execution evidence. Typed failure transport and
trusted scene resource-profile fields still need implementation contracts;
no code, runtime capability, or host version is changed by these documents.

## Scene host integration contract v2 (2026-10-05)

The user explicitly authorizes completing failure identity, trusted resource
fields, and result/event contracts before implementation. The selected
[Scene Host Contract v2](../spec/codegrid-scene-host-contract-v2.md) uses separate
Level API 2/profile 2/browser binding 2/portable ABI 2 and leaves v1 untouched.
Language API/ABI and the author/gameplay versions are independently unchanged.

Existing level.test_failed/9027 carries typed InvalidOutput, IllegalOperation,
and IncompleteGoal reasons for new v2 scenes. Existing WrongOutput, VM errors,
fault, constraints, resource, and cancellation identities stay in their categories.
The registry clarifies the v2 scope of existing level correctness/runtime-error
categories without changing v1 emitted behavior or allocating new numbers.
Author cross-field typed reasons and exact offending paths are fixed.

Profile 2 adds mandatory scene_limits for unread/cumulative input, retained scene
state, complete frames, per-call/cumulative scene work, retained visible Debug
events, and reserved feedback bytes. All values are positive exact u64 strings.
Public results use visible_cases with explicit scene summaries; hidden failure
redaction remains mandatory. Only visible Debug cases produce scene feedback,
with monotonic public event sequences and acknowledgement cursors. World-derived
round transitions are allowlisted; no raw VM/world or hidden author snapshots
are exported. Local example ceilings are not production recommendations.

The contract is design-only. Actual Rust v2 profile/loader/session/projection/
feedback support, dual-version transport artifacts, and host comparisons must
be implemented before advertising it. Auto-generated numeric documentation is
synchronized; runtime instruction semantics and existing API implementations
are unchanged by this task.

## Scene feedback implementation clarifications (2026-10-05)

Implementation review found two incomplete payload domains in the accepted
Scene Host v2 design. Under the user's delegation to adopt recommended design
choices, record these explicit clarifications before depending on them:

- MechanicalArm feedback interaction identifies the slot faced before the
  action, including WAIT/TURN. An unpopulated table remains Worktable with its
  public index; the lower-arm orientation 0 is Unavailable. This names feedback
  locations without changing GRAB/DROP gameplay or permitting unavailable use.
- RuntimeError failure details contain runtime_errors, an array of forwarded
  VM code/error_number pairs. This resolves the contract's existing forwarding
  requirement while preserving the exactly-four-field visible failure envelope.

These are documented implementation clarifications, not quotations from the
referenced scene conversation. They allocate no error numbers and change no
source syntax, VM behavior, author acceptance, or hidden-case projection.

## Scene feedback retained-record accounting clarification (2026-10-05)

Under the user's authorization to adopt recommended integration details, the
Scene Host Contract v2 explicitly counts each retained event and round-change
record as one scene state unit. Owned observation byte vectors retained by an
event are counted separately from world observations and VM input; scalar fields
inside a record add no units. This closes a retention-accounting omission for
already specified feedback records without changing gameplay, language behavior,
or stable error identities. It does not establish complete API-instance or
cross-host resource conformance.

## Scene feedback publication encoding and page copies (2026-10-05)

Under the user's authorization to adopt recommended integration details, retain
one immutable JSON encoding per published visible event. Scene work charges
its actual UTF-8 encoding bytes during staging, including failed attempts.
Reading/replaying retained events copies that encoding into a transport envelope;
this byte copy does not perform a new scene-content encoding. Typed native pages
remain available, but the shared API must use the retained encoding rather than
serializing scene payloads again. Feedback byte retention includes the cached
encoding until acknowledged storage is released.

A prepared page's owned event/observation/round-change copies count independently
from retained storage against scene units. Reserve their complete peak before
cloning each record and reject insufficient capacity without changing delivery
or acknowledgement. Envelope capacity must be checked before copying cached
bodies or confirming delivery. This clarifies the previously accepted work and
retention contract without changing event fields, gameplay, or stable errors.

## Player-authored Scene architecture requirement (2026-10-05)

The user requires the project architecture to support player-authored Scenes in
the future. The current six-scene Rust catalog, Level API v2, author formats,
and WASM transports remain fixed and are not represented as custom-package
support.

The recommended project boundary is recorded in the
[Custom Scene architecture](custom-scenes-architecture.md): a contract-only
Scene API; shared Rust evaluation in `codegrid-level-core`; host-owned package
resolution and sandboxed WASM execution; and a separately versioned cooperative
call/reply Level API so browser and portable WASM hosts can run the same
evaluator without embedding a particular WASM engine in the core. Community
packages use a serialized protocol, never Rust dynamic-library ABI. This is an
architecture target, not an approved runtime wire schema. Exact package
manifest, guest ABI, resource accounting, error identities, editor schema, and
host conformance vectors must be specified before implementation begins.

## 2026-10-07: Remove paper-tape and elevator runtime scenes

The user explicitly removes Baudot and Elevator from the Rust scene catalog.
Only ExactIO, QualityControl, Robot, and MechanicalArm remain executable.
Paper tape is an authoring and presentation mode compiled into ExactIO input
and expected output bytes. Rust does not enforce a five-bit domain or interpret
character shifts. Old Baudot/Elevator wire scene names are unsupported; consumers
may explicitly migrate saved paper-tape data to ExactIO while preserving
presentation metadata. Elevator drafts remain available for recovery/backup but
cannot be exported or evaluated by the current host. Elevator-specific metrics,
passenger state, runtime actions, and feedback are removed. This decision
supersedes the earlier six-scene selection without changing Full VM semantics.

The same user decision also removes QualityControl as a Rust scene. Quality
authoring precompiles numeric input and expected decisions into ExactIO cases;
Color/PackedRobot and Item/Batch are presentation/authoring metadata only.
Rust imposes no quality-specific byte ranges or case lengths. The final runtime
catalog is ExactIO, Robot, MechanicalArm.

## 2026-10-07: Remove obsolete scene fixtures and examples

The user requests complete cleanup of removed scene implementation material.
Delete obsolete device fixtures and dedicated invalid-example files; explicit
removed-identifier rejection remains covered directly by schema and transport
tests. Rename generic ExactIO examples and execution vectors to
`exactio-byte-boundaries` and `exactio-classification`, including their manifest
IDs. Remove superseded device examples and validation reasons from current
specifications and session documentation. Earlier dated decisions remain an
audit record, not executable registrations. No Full language semantics or
published numeric error identities change.
