# Full Language Decisions and Open Questions

This log records language and VM decisions only. The current normative
contracts are the [source specification](../spec/codegrid-source-spec.md),
[VM specification](../spec/codegrid-vm-spec.md), and [Runtime API v4](runtime-api-spec.md).
Historical host versions and behavior descriptions remain dated evidence; they
do not replace current specifications. Game-only decisions and game-specific
verification excerpts are archived in the sibling
[Game Runtime history index](../../bf-steam-wt/doc/codegrid-lang-history/README.md).

## Sources of truth

- [`spec/codegrid-source-spec.md`](../spec/codegrid-source-spec.md) is the normative Full authority for source syntax and static acceptance. Source-language choices are resolved below, and the source/compiler plus IR implementation gates have focused conformance coverage.
- [`spec/codegrid-vm-spec.md`](../spec/codegrid-vm-spec.md) is the Full execution authority. The choices in the table below resolve its former implementation gates; direct conformance coverage remains required.
- [`runtime-api-spec.md`](runtime-api-spec.md) defines the normative host-neutral Full Runtime API v4 contract. Browser bindings and server ABI use separately versioned transport contracts.
- Historical compiler, IR, and VM tests plus retained Full examples are evidence to review, not automatic replacements for normative rules.

## Stable error identifiers (2026-09-30)

Explicit user decision: complete stable identifiers for language and toolchain errors and publish their specification under `spec`. Validators assign source/IR codes at detection; adapters forward the codes. Diagnostic `code` is additive under CLI run-result schema 1, Runtime API v3 and Server ABI v4. Existing VM codes and WASM aliases remain compatible. Native debug errors change from a string to `{code,message}`, advancing the protocol to 2 and the editor to 0.3.0. No source acceptance, VM transition, error ordering or rollback rule changes. See the [error specification](../spec/codegrid-error-codes.md) and its JSON registry. Game error identities are preserved in the Game Runtime history archive.

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
| Board structure | Support scoped Function/Folded Block paths, including definitions nested in Custom CodeGrids. Main and Custom Main contexts may have multiple Entries; each Function Board has at most one explicit Entry. A board without Entries defaults to (0, 0), facing Right. | Source syntax and Entry-count rules are normative in the Full source specification. Board execution and resume behavior belong to the Full VM specification. |
| Folded Blocks | A Folded Block is one row high and matches its owner's width; entry starts at internal position (0,0) facing Right, independent of the caller direction. The block never wraps: movement leaving the block exits through Fold Resume, and direction changes and random directions are forbidden inside it (see the Linear Folded Block sweep decision, 2026-10-20). | The historical `concurrent_outer_folded_threads_keep_control_and_register_state_isolated` case confirms independent internal direction and saved caller direction; the Full VM specification now states this explicitly. |
| Concurrent outer transition | Threads evaluate from the same transition-start state. Their private state and shared effects are staged, conflicts are resolved deterministically, and the outer transition commits atomically or rolls back. Thread order is not a semantic write priority. | The Full VM specification defines the complete conflict, rollback, and ordering rules; direct integration coverage remains required. |
| Calls and Repeat | Function CALL/RETURN and caller Resume behavior consume their specified VM transitions. Qualifying tail recursion reuses a frame. Repeat spreads Primary executions across transitions and moves after its final execution. | Execution timing, tail-call eligibility, and Repeat behavior are normative in the Full VM specification; direct conformance cases remain required. |
| Custom | Each Custom invocation starts with isolated context/state, runs synchronous internal transitions within the outer transaction, and participates in caller-stack transactions. Every outer and Custom thread dispatch uses the same generic host work-unit budget; there is no language-level Custom cap. | Error priority, seed derivation, nested-call, rollback, and internal transition behavior are normative in the Full VM specification; direct conformance cases remain required. |
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
| CustomReturn completion | `#]` terminates only the executing Custom internal thread. A Custom invocation returns normally after every internal Entry thread has terminated through `#]`; other threads continue on later aligned internal steps. A same-step Custom Halt takes precedence over normal return, and any runtime error takes precedence over Halt. | Direct Full tests `custom_invocation_waits_for_returns_from_every_internal_thread` and `custom_threads_returning_on_the_same_step_complete_together` cover staggered and simultaneous returns. `custom_halt_takes_priority_over_same_step_custom_return`, `custom_internal_error_takes_priority_over_a_same_step_halt_request`, and `work_limit_covers_nested_custom_dispatch_and_rolls_back_the_committed_steps` cover priority and the limit. Covered by active VM tests. |
| CALL attachments | A CALL's ReadCode or WriteCode Attachment is deferred until the caller's AfterCall transition, after the callee returns successfully. It executes once against the CALL site's transition-start mutable Primary, then the caller advances. It does not execute if the call halts, errors, or otherwise fails before returning. | `calls_consume_a_step_and_return_uses_a_separate_resume_step` and `call_write_code_runs_after_return_using_the_callees_instruction_stack` cover the two attachment types; `recursive_callee_aftercall_uses_the_current_mutable_call_site` observes consecutive recursive AfterCall writes against the updated CALL cell; `deferred_call_attachments_do_not_run_when_the_callee_halts_or_errors` checks both attachment types against halt and error exits. Covered by active VM tests. |
| RETURN attachments | ReadCode and WriteCode attached to RETURN are accepted by the source/IR compatibility matrix but have no runtime effect and contribute no attachment operation metric. RETURN itself still executes and is counted normally. | The CALL timing test observes inert RETURN ReadCode; `write_code_attached_to_return_is_inert` asserts the Primary, Instruction Stack, operation count, and Instruction Variety for RETURN WriteCode. Covered by active VM tests. |
| ReadCode of Empty | Preserve the defensive result that an Empty transition-start Primary would push the EMPTY Instruction Stack item (Instruction Code 32). This state is unreachable in verified IR: each cell has one fixed Attachment, attachments require an initial Primary, and WriteCode changes only the Primary while retaining its Attachment. | `validate_cell` enforces the initial structure, and the VM writes only the Primary field. Active tests cover ReadCode of an executable Primary, WriteCode clearing while retaining its Attachment, and DECODE/ENCODE of Code 32. The impossible post-mutation ReadCode case remains intentionally excluded; VM spec §17 records the reachable acceptance gates. |
| Empty POPADD and NAND | Each is a counted no-op when the executing thread's Data Stack is empty: preserve the selected register and stack, then perform ordinary movement. | `empty_popadd_and_nand_are_counted_noops` directly asserts preserved register/stack, each instruction's movement, operation count, and variety; nonempty behavior is covered by `nonempty_popadd_updates_register_and_consumes_one_stack_value` and `nand_complements_the_bitwise_and_of_register_and_stack_values`. Covered by active VM tests. |
| Empty Custom READ | Preserve the selected Custom register and set the instruction's encoded direction, as outer READ does when its input source is exhausted. The empty caller stack causes no successful caller-stack read, register write, or read conflict. | `empty_custom_read_preserves_register_and_turns_before_later_output` checks preserved value, direction, and movement; `exhausted_custom_read_can_coexist_with_one_caller_stack_output` and `custom-empty-concurrent-reads-coexist-with-output` cover coexistence. Nonempty reads and conflict cases have separate direct tests. Covered by active VM tests. |
| Conflict matrix and error order | Adopt the exact conflict rules in VM Spec §§10.1–10.2, including rejected reads not producing secondary register conflicts, same-value writes still conflicting, and the specified cross-effect cases. Normalize participant IDs and sort errors by scope, error-code name, resource-key variant/fields, and participant IDs. | `effects.rs` covers rejected input/caller reads, empty reads, equal writes, memory/code identity, and caller-stack combinations; `error.rs` covers canonical scope/code/resource/participant ordering. Active VM integration tests cover rollback and several resource combinations, including new successful input-read/register-write and Custom caller-read/register-write cases. **Partial:** there is not yet one direct Full integration matrix covering every cross-effect combination; keep this gate open. |
| Event ordering | Emit committed events only after a successful outer-step commit. Order all CellReached events first by outer thread ID, with that thread's outer visit before its Custom visits; order Custom visits by internal step then internal thread ID. Then order Custom state events by internal step, caller thread ID, and Custom ID, followed by Outer state events. Within each context step, order event categories as InputConsumed, RegisterChanged, MemoryChanged, CodeChanged, ThreadChanged; order values within each category by thread ID, register index, resource key, cell key, or thread ID as applicable. | `step_events_report_input_and_memory_commits_with_scope` checks successful event order; `simultaneous_custom_state_events_are_ordered_by_aligned_internal_tick` checks the Custom visit prefix, aligned register event order, and Custom-before-Outer state ordering. Failed conflict tests assert empty committed events and canonical error order. **Partial:** the complete cross-category multi-thread/Custom event sequence does not yet have one full golden assertion; keep this gate open. |
| Stack high-water metrics | Sample aggregate depths at outer-step boundaries and at each aligned Custom internal-step boundary. Sum the depths of all resident Outer and Custom threads at that instant; count each Outer caller stack once. Before commit, include staged pushes and call-frame growth without allowing staged pops to reduce the attempted high-water mark. On successful commit, also sample the committed post-state. Preserve attempted peaks through runtime-error rollback; a host work-limit interruption adds no metric. | `failed_aligned_custom_step_reports_footprint_and_rolls_back_state` directly covers all three stack classes across Outer and two aligned Custom threads, caller-stack replacement, staged growth/pops, and rollback of a failed candidate step while retaining attempted peaks. `aligned_custom_internal_stacks_contribute_to_the_global_peak` checks the aggregate stack metric. Covered by active VM tests. |

## Resolved native CLI contract

The local native CLI contract is specified in [`docs/cli.md`](cli.md). Its command spellings, input formats, JSON schema, exact-integer encoding, output streams, and process exit codes are decisions for the Full local implementation, not unresolved adapter choices.

| Topic | Decision | Follow-up |
| --- | --- | --- |
| Commands and bounds | Keep `check <program.cg>` and `run <program.cg>`. `run` requires `--seed`, `--max-steps`, and `--max-work-units`; both limits are positive `u64` values. Input is optional and comes from exactly one of `--input` or `--input-file`; initial Outer memory is optional via `--initial-memory-file`. | Current command and bound names are specified in [`docs/cli.md`](cli.md); focused argument and shared Full fixture checks cover the language CLI. Former game execution quotas are in the Game Runtime history archive. |
| Input representation | Inline input is a comma-separated byte list; file input is a JSON byte array. Initial memory is a JSON array of unique `{address, value}` entries; addresses are canonical signed decimal strings and values are bytes. Zero-valued initial entries are accepted and normalized away. | Implemented and covered for both input forms, malformed values, duplicate addresses, empty input, wide addresses, and memory normalization. |
| Run result | Use `schema: "codegrid.cli.run-result"`, `schema_version: 1`, with distinct `source_error`, `halted`, `runtime_error`, `yielded`, and `vm_fault` outcomes, complete Full snapshots, committed events/output, and the effective configuration. | Implemented with full snapshots, events, metrics, errors, yields, and deterministic JSON checks; this schema remains independent from the current Runtime API v4 and adapter wire schemas. |
| Wide integers and ordering | Encode every `u64`, arbitrary-precision signed integer, and UTF-8 source byte offset as a canonical decimal string. Keep bytes and bounded values as exact JSON numbers. Serialize identity collections in canonical VM order. | Implemented and covered by wide seed/address and shared Full conformance checks. |
| Process behavior | `check` uses human-readable output. `run` emits one complete JSON result on stdout whenever source reaches the compiler; human diagnostics go to stderr. Exit codes are fixed: 0 success/halt/help, 2 invalid arguments or host data, 3 I/O or source decoding, 4 source rejection, 5 VM runtime error, 6 step-limit yield, 7 work-unit yield, and 8 VM fault. | Implemented and exercised by focused CLI tests; step/work yields and VM faults remain distinct from normal halt and runtime errors. |

## Historical server Runtime API adapter contract (2026-09-30; superseded by Runtime API v4 / Server ABI v5)

The following records the earlier v3/v4 adapter decision and its verification.
The current host-neutral contract is [Runtime API v4](runtime-api-spec.md);
the current server transport uses ABI v5.

The no-import server adapter uses its own Server ABI v4 over host-neutral
Runtime API v3. ABI v3's reduced request and snapshot fields are incompatible
with Full v3, so the server adapter version is bumped independently; Runtime
API versioning does not version the byte ABI. Requests and responses carry both
`abi_version: 4` and `api_version: 3`.

The v4 request layout uses flat operation fields. `initialize` takes immutable
trusted `host_limits`, including `max_initial_memory_entries`.
`create_instance` takes a program handle, byte-array input, canonical decimal
string `seed`, and optional `initial_memory` entries with canonical signed
decimal string addresses and byte values. `program_view` exposes canonical
compiler output by existing program handle. `step`, `run`, `snapshot`, release,
and shutdown operations map to the equivalent Runtime API v3 operations. The
adapter never accepts IR or duplicates compiler or VM semantics. Former
game-specific Custom and execution ceilings are preserved in the Game Runtime
history archive.

The full `step`, `run`, and `snapshot` projections include complete thread
state, Page, stacks, call frames, phase, RNG state, mutable Outer code, sparse
memory, committed events, errors, and optional faults. `run` returns a complete
snapshot for each terminal or yielded outcome. API failures, source diagnostics,
VM errors, faults, and yields stay distinct. Every `u64`, handle, step counter,
PRNG state, and arbitrary-precision signed integer uses a canonical decimal
string; bytes and bounded geometry remain exact JSON numbers. Errors use a
versioned envelope with stable codes and structured details. Requests/responses
and retained state are bounded, and oversized responses are rejected as
complete errors rather than partial JSON.

| Topic | Decision | Follow-up |
| --- | --- | --- |
| Server ABI lifecycle | Keep the existing no-import buffer exports and per-module session, but version its Full JSON contract as ABI v4, independent of Runtime API v3. A module session has immutable host-selected limits and explicit `initialize`/`shutdown` lifecycle. | Implemented; native tests and the Node WebAssembly smoke pass export shape, buffer lifecycle, and session isolation checks. |
| Server Full projection | Serialize the complete v3 snapshot, event, error, fault, and program-view data through bounded projections. Keep large integers exact and distinguish work-limit yield from step-slice yield. | Implemented; all 63 shared Full runtime cases pass in native adapter tests and the Node WebAssembly smoke. |

## Historical local verification and out-of-scope policy

- The same Full conformance fixtures were compared in the native CLI, actual browser-WASM hosts (Node and headless Chrome), and actual server-WASM host (Wasmtime 49.0.1 using Server ABI v4 / Runtime API v3). All 63 cases match the complete Native CLI result projection; the host lifecycle, memory, stack, fuel, work-limit, and buffer probes also pass.
- Production authentication/authorization, deployment quotas, process/isolate resource controls, cancellation, and release/distribution policy are outside the local language-development goal.

Adapter schemas, local resource ceilings, and wide-integer representations are specified in the respective versioned host contracts. Do not infer production suitability from local limits.

Production authentication/authorization, multi-user quota policy, and deployment operations are outside the local development scope and are not gates for the Full local CLI or language core.

## Four-digit error identities (2026-10-01)

An explicit user request assigns stable four-digit decimal strings to all
registered toolchain errors. Registry v2 adds error_number to each scoped
identity. Published textual identifiers, VM spellings, transport aliases,
standard JSON-RPC codes and ABI zero sentinels remain compatible. Structured
outputs carry the number; process/editor messages show it alongside the
original identifier. LSP stores it in data and DAP uses it as the presentation
error ID. Numbers are never inferred from message wording, reassigned, or
recycled. Generated Rust/TypeScript lookup data and the Markdown table use
the JSON registry.

## Immediate output Primaries (2026-10-03)

The user explicitly approves .0 through .9 as non-encodable Primaries that
output raw byte values 0 through 9 without changing registers or the register
pointer. They have no Instruction Codes and accept no Attachments, including
ReadCode, WriteCode, and Repeat. Ordinary . retains its existing behavior.
Immediate output uses ordinary output movement, step/work accounting,
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
condition skips both Primary and suffix, consumes one committed step and moves one cell
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

The [execution manual](conditional-prefix-migration.md) preserves the language-side
design history. The source and VM specifications define the current language
contract. Former Level permission and game-host acceptance details are preserved
in the [game-host history record](../../bf-steam-wt/doc/codegrid-lang-history/conditional-prefix-game-host-2026-10-03.md).

### Non-consuming CMP Primary

The user adds CMP with the source token `?=`. It reads A from the executing
thread's Data Stack top without popping and B from the register selected by
that thread's current pointer. CMP writes 0 when A equals B, 1 when A is greater
than B, and 2 when A is less than B to the same selected register. Comparison
uses the existing unsigned-byte value domain. The Data Stack is unchanged.
As a Primary, CMP is eligible for the approved conditional prefix category.
In `?1?=`, the prefix tests the register before CMP executes.

At the time this design record was written, empty-stack behavior, CMP Instruction
Code assignment, suffix compatibility, and operation accounting remained open.
The current source and VM specifications now define CMP. The former game
capability question is preserved in the [game-host history record](../../bf-steam-wt/doc/codegrid-lang-history/conditional-prefix-game-host-2026-10-03.md).

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
  internal Custom threads. It supports ReadCode, WriteCode, and Repeat and counts
  one CMP runtime kind.
- A prefix reads the selected register from the context step-start snapshot.
  CALL checks only before invocation; AfterCall and FoldResume do not recheck.
- WriteCode changes only Primary and preserves both prefix and suffix. ReadCode
  reads only Primary. Cleared cells still evaluate their fixed prefix before
  deciding whether to execute the retained suffix.
- Every evaluated prefix costs one operation, whether true or false. The three
  values share one runtime kind.
- New source rejects old IfZero tokens and standalone `?`. Executable IR gets
  a new format version. RandomDirection moves from code 63 to 126 and CMP uses
  124; 63 and removed IfZero codes 95, 97, 129, and 153 no longer decode. Other
  retained encodings remain unchanged, and existing non-encodable forms remain
  excluded. Existing invalid-byte DECODE behavior remains a counted no-op.

This approval resolved the preceding design entries. At that time, implementation,
normative specification migration, and host execution parity remained future
work. The current source and VM specifications are authoritative for language
behavior; former game capability names and game-host acceptance are archived in
the [Game Runtime history](../../bf-steam-wt/doc/codegrid-lang-history/conditional-prefix-game-host-2026-10-03.md).

### Implementation projection and capability choices

The former Level permission identifiers `CONDITION_0`, `CONDITION_1`,
`CONDITION_2`, and `CMP` are preserved in the [game-host history record](../../bf-steam-wt/doc/codegrid-lang-history/conditional-prefix-game-host-2026-10-03.md). On the
language side, runtime operation kinds are Condition and Compare. Prefix
metadata is stored separately from Primary and the existing suffix, including
on Folded Block cells.

Normal code-view cells gain a nullable prefix field; folded code-view strings
include the canonical prefix. These additive read-only fields do not accept
executable IR from hosts. At that time, Runtime API v3, browser envelope,
server ABI v4, and CLI JSON schema 1 remained unchanged; executable IR alone
advanced to format 2. This records
implementation choices within the approved migration, not compatibility
acceptance of the removed language.

## 2026-10-07: Repeat suffixes on immediate output

The user explicitly permits `.0` through `.9` with Repeat suffixes `x2` through
`x5`, including conditional prefixes. This supersedes the immediate-output
suffix prohibition for Repeat only. Each repetition executes on its own committed step,
uses ordinary output costs, conflicts and rollback, and routes Custom output
to the caller stack. Registers and their pointer are preserved; movement occurs
after the last repetition. ReadCode and WriteCode remain invalid because
immediate outputs have no Instruction Code. Encoding, IR format and host API
versions are unchanged. Rust compiler/IR/VM and actual-host fixtures establish
acceptance and execution; downstream consumers use rebuilt shared WASM.

## 2026-10-07: Specific invalid-cell diagnostics

Invalid Full cells retain source.invalid_cell and numeric code 1004. Diagnostics quote the complete source atom, preserve its UTF-8 byte span, and identify the violated token or Attachment rule. Conditional prefixes preserve an invalid body's specific reason. Message wording is descriptive; consumers must continue using stable error identities rather than parsing messages.

## 2026-10-08: Compatibility begins only with a user-published release

The user subsequently authorizes execution of the language compatibility cleanup.
The user states that no project release has been published. Only an explicit
user publication establishes a compatibility baseline. Before that event,
update language contracts and consumers directly without historical readers,
aliases, reserved codes, migration layers, retained binaries, or mandatory
version increments for compatibility. Internal version labels and acceptance
reports are not published releases. This supersedes earlier unpublished
language compatibility decisions. See the [repository policy](../AGENTS.md#release-and-compatibility-policy).
Preserve unrelated user work and truthful provenance throughout.

## 2026-10-08: Status Flag and directionless READ design

The user approves the [next-generation amendment](status-flag-and-read.md),
implemented following explicit user authorization. Add thread/frame-local F and
`?!`; replace directional READ with `,` (code 44). Empty READ and empty
POPADD set F=1; nonempty POPADD reports unsigned carry. NAND and existing CMP
preserve F; no standalone POP is added. Fix the pointer example to `?!}`.
Top-level threads start at zero; each Custom Entry copies the invoking F.
Functions copy and discard private F; Folded Blocks share active F.
Exhaustion means attempting a read after the provided input has no bytes left.

Apply the unpublished-release policy to language contracts: remove directional
READ syntax and mappings directly, without reserving its former code or adding a
compatibility reader or migration layer. No compatibility-driven IR or language
host version bump is required. The language-side implementation and its 94 Full
fixture comparisons are described in the [amendment](status-flag-and-read.md).
The former `CONDITION_FLAG` game permission and 57-scene host comparison are
preserved in the Game Runtime history archive.

## 2026-10-08: Function-private registers and NEG

The user requests specification changes first after review of the referenced
Function and NEG design documents. Approve ordinary Function CALL copying all
ten caller active register values and the pointer into invocation-private
state. RETURN discards that state and resumes the caller bank and saved
pointer. Main banks remain shared across Main threads; returning must not
restore an old Main snapshot over sibling writes. Nested Functions, recursion,
eligible self-tail reuse, Functions in Custom, step-start reads and rollback
follow the clarified VM contract. Stack/Page/PRNG ownership, memory/code/I/O,
CALL/RETURN/AfterCall timing and tail eligibility are unchanged.

Approve `$!` as NEG with Instruction Code 69, capability `NEG` and metric kind
`Neg`. The explicit current table has no encoding collision. NEG writes the
selected byte's additive inverse modulo 256 and performs ordinary movement.
It accepts existing prefixes and encodable-Primary suffixes using canonical
`x2` through `x5` syntax. Existing NAND, decrement and every other code remain
unchanged. DECODE(69) will change from an invalid counted no-op to a valid push.

These decisions supersede the shared-Function-register contract and complete
the Primary/code inventory only as target specifications. Rust and downstream
WASM implementation remains pending. The
[implementation plan](function-registers-and-neg.md) identifies required
coverage and downstream consumers. Snapshot/event wire representation,
resource sizing and compatibility/version review are release gates, not
claims that current host fields already expose private state.

## Function and NEG implementation compatibility (2026-10-08)

The approved value-copy Function register/pointer scope and `$!` NEG code 69 are implemented. RETURN discards the callee bank and resumes its caller bank/pointer; same-thread Data Stack returns remain available. Main sibling writes are never overwritten by an old snapshot. Executable IR is bumped to 3 because the Function execution semantics changed; reject formats 1 and 2 and recompile source. At the time, Host Runtime API v3/browser v3/server ABI v4 added nullable private/saved banks and saved pointers without a new event kind. ThreadChanged carries private state; RegisterChanged retains shared Main-bank semantics. Actual host comparisons covered all 81 Full fixtures.

## Toroidal boards and implicit entries (2026-10-11)

The user approved removal of normal-board boundary selection. Every normal
Main and Function board, including boards inside Customs, wraps horizontally
and vertically. Crossing an edge preserves the other coordinate and direction.
All ordinary boards use this fixed topology; hosts cannot configure a different one.
Invalid geometry remains rejected by verification; invalid internal movement
state is a VM fault. Folded Blocks retain horizontal wrapping and vertical
Fold Resume transitions.

A board without an explicit Entry starts at (0, 0), facing Right. Outer and
Custom Main create one default thread only when no explicit Entries exist.
Functions allow zero or one explicit Entry. Explicit Main Entries still create
threads in row-major order, without an additional default thread. The default
entry does not replace the cell, occupy a cell, or count as a placed instruction.
The first dispatch executes the actual cell at (0, 0); an empty cell consumes
one committed step and moves Right. CALL retains its own transfer step, and the
target cell executes on the next step. Custom execution retains synchronous
internal steps.

Current language request, response, CLI, debug, and editor contracts removed
boundary selection directly under the unpublished-release policy. Existing work
and player content must be preserved. Application-specific adoption is recorded
in the Game Runtime history archive.

## Fixed toroidal topology across every consumer (2026-10-11)

The user extended the approved scope to every application execution path and
requested removal of alternative board topology configuration and obsolete
authoring documentation. Ordinary boards always wrap horizontally and
vertically; entering (0, 0) does not reset direction. Initial execution faces
Right and executes the actual first cell. This extends the application contract
without adding game rules to the Rust language core.

Current language schemas, callers, and editors do not expose topology
selection. Source formatters and parsers do not accept topology directives. No
aliases or compatibility-only migration layers are retained under the
unpublished-release policy. Game completion and content updates are preserved in
the Game Runtime history archive.

## Repeat suffix range extension (2026-10-12)

The user explicitly extends Repeat from `x2`–`x5` to `x2`–`x9`.
This decision updates the current source and VM contracts, canonical attachment
inventory, diagnostics, and editor assistance. Repeat still executes its
Primary once per VM transition, for the stated total count;
prefix reevaluation, movement, cancellation, conflicts, and rollback are unchanged.
CALL and RETURN still reject Repeat. Immediate output accepts the same range.
`x0`, `x1`, multi-digit counts, and non-ASCII digits remain invalid. No API or
IR format version bump or compatibility reader is required before publication.
Game-side authoring and acceptance history is preserved in the Game Runtime
archive.

## Linear Folded Block sweep (2026-10-20)

The user approved two Folded Block changes. First, Folded Blocks no longer
wrap: movement that would leave the block — vertical in either direction,
Left past the left edge, or Right past the right edge — exits through a Fold
Resume transition that retains the saved outer caller direction. Second,
direction changes and random directions are forbidden inside Folded Blocks;
the compiler rejects them with `source.fold_primary` and the IR verifier with
`ir.fold_primary`. Folded Block execution is therefore a deterministic
left-to-right sweep that exits at the right edge or through a prefix-gated
Halt. This supersedes the retained horizontal wrapping in the 2026-10-11
toroidal decision; ordinary boards remain toroidal.

No error-number additions are required: both forbidden-direction diagnostics
reuse the existing `source.fold_primary` (1040) and `ir.fold_primary` (2007)
identities, whose trigger descriptions now include directions. Conformance
fixtures that exercised in-fold wrapping or in-fold directions were rewritten
for the linear sweep. Under the unpublished-release policy, current contracts
are updated directly without aliases or compatibility layers.
