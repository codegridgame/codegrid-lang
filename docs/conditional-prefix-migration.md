# Conditional Prefix Migration Execution Manual

Status: Implemented and verified. The approved decisions are reflected in the normative specifications, shared Rust pipeline, VM, level evaluator, hosts, and editor.

Date: 2026-10-03

## 1. Objective and authority

The confirmed change removes `#^`, `#v`, `#<`, and `#>` and introduces three prefix attachments: `?0`, `?1`, and `?2`. A prefixed instruction executes only when the executing thread's selected register equals the specified byte. RandomDirection changes its source token from `?` to `??`. Custom tokens remain unchanged.

The user approves D1–D15 below, including CMP (`?=`), empty-stack behavior, execution boundaries, accounting, and compatibility. Accepted decisions are recorded in `docs/decisions.md`. This manual defines the implementation scope; the normative specifications have been migrated to the new semantics.

CMP reads A from the executing thread's Data Stack top without popping it and B from the selected register. It writes 0 for A == B, 1 for A > B, and 2 for A < B to that same register. Values are unsigned bytes; comparison is not subtraction with wrapping. The register pointer and every Data Stack element remain unchanged. On an empty stack, CMP preserves the register, counts one operation, and moves normally. In Custom code it uses the internal thread's own Data Stack, not the outer caller's stack.

Authorities reviewed:

- [Source contract](../spec/codegrid-source-spec.md), especially sections 5–8.
- [VM contract](../spec/codegrid-vm-spec.md), especially sections 4–10 and 14.
- [Decision record](../docs/decisions.md).
- [Error identities](../spec/codegrid-error-codes.md).
- [ExactIO contract](../spec/codegrid-level-exactio-contract-v1.md).

Implementation evidence reviewed includes the canonical model inventory, HIR cell representation, IR cell/verifier and tail-call analysis, VM Normal/Repeat/AfterCall dispatch, VM metrics, level capability validation, and VS Code instruction metadata. The Cargo workspace already includes language and level WASM adapters; older documents describing them as planned do not establish their current implementation status.

## 2. Baseline behavior and affected design

| Area | Current behavior | Consequence of the proposal |
| --- | --- | --- |
| Conditional Primary | `#^`, `#v`, `#<`, `#>` are `IfZero(Direction)` | Removing them affects parsing, inventory, execution, metrics, encoding, editor metadata, and level capabilities. |
| Custom calls | `#0`–`#9` call Custom CodeGrids; `#]` returns | Preserved unchanged; the accepted `?0`–`?2` spelling avoids the initial collision. |
| Random direction | `?` is RandomDirection | Rename the token to `??`; decide encoding compatibility separately. |
| Comparison | No CMP Primary exists in the current canonical inventory | Add `?=` as a Primary with a non-consuming Data Stack comparison; decide empty-stack behavior and encoding. |
| Cell shape | One Primary and at most one suffix Attachment | A prefix needs its own field and compatibility rules if it can coexist with a suffix. |
| Suffixes | `*`, `=`, `x2`–`x5` | They run after the Primary, with special CALL/RETURN timing. |
| Suffix restrictions | Encodable Primaries only; Repeat excludes CALL/RETURN | Prefix eligibility must be specified independently. |
| Folded Blocks | Attachments are forbidden; structural restrictions also apply | Allowing prefixes requires a specific exception without permitting forbidden Primaries. |
| Register reads | Context tick-start register snapshot; thread-local pointer | A condition should use the selected register, not hard-code R0 or see sibling writes. |
| Repeat | Primary executes once per tick; the thread stays until completion | Rechecking a prefix and cancelling a repeat need explicit phase transitions. |
| Function resume | Successful RETURN enters a separate AfterCall tick | Decide whether a previously successful condition is rechecked after the callee changes state. |
| WriteCode | Changes only Primary; fixed suffix survives, even after EMPTY | Decide whether a prefix survives replacement and clearing. |
| Encoding | IfZero codes are 95, 97, 129, 153; EMPTY is 32 | Removed codes must not silently acquire new meanings. |
| IR | `IR_FORMAT_VERSION` is currently 1 | Removing a Primary and extending cells requires an explicit compatibility decision. |
| Level rules | IF_ZERO capabilities and independent suffix permissions exist | Replace capabilities and define prefix permissions and static kind accounting. |

## 3. Decision checklist

Confirmed: all D1–D15. No language-semantic question remains open. Concrete internal identifiers and any host schema/version changes must be specified and documented during implementation without altering these decisions.

| ID | Question | Accepted rule |
| --- | --- | --- |
| D1 | Which instructions are removed? | Remove exactly the four IfZero directions. Preserve RandomDirection, input exhaustion directions, and other control flow. |
| D2 | How is the Custom token collision resolved? | Confirmed: use `?0`, `?1`, `?2`; rename RandomDirection to `??`; preserve `#0`–`#9` and `#]`. Thus `?0+` is guarded Add and `?1#0` is guarded Custom 0. |
| D3 | Can prefix and suffix coexist? | At most one prefix and one existing suffix, ordered `prefix + Primary + suffix`, without whitespace. |
| D4 | What can receive a prefix? | Every Primary, including immediate output, Halt, calls, returns, and allowed Folded Block contents; never initial Empty or Entry cells. Existing contextual Primary restrictions remain mandatory. |
| D5 | What happens on false? | Skip the Primary and suffix, consume one dispatch/tick visit, and perform ordinary movement in the current direction. Movement may still cause a boundary error or Fold exit. |
| D6 | When and which register is read? | Use the context tick-start snapshot at the executing thread's pointer before any Primary effect. |
| D7 | How does Repeat interact? | Check before every attempted Primary execution. A false result cancels remaining repetitions, returns to Normal, and moves once. |
| D8 | Does AfterCall recheck the condition? | No. Test when dispatching CALL; after a successful return, run the existing deferred suffix and resume behavior once. FoldResume likewise does not recheck. |
| D9 | What survives WriteCode? | Prefix and suffix remain fixed; ReadCode/WriteCode and DECODE/ENCODE continue to represent only Primary. A cleared cell can retain a prefix; true permits the existing suffix behavior, false suppresses it. This runtime state is distinct from invalid initial detached prefixes. |
| D10 | Does a condition cost an operation? | Count each evaluated prefix as one operation, including false results, and use one runtime instruction kind for all three values. Count skipped cells as visited; do not count skipped Primaries/suffixes as executed. |
| D11 | How are level permissions represented? | Independent per-value prefix capabilities, separate from Primary permissions and existing suffix permissions. Count each prefix capability in static instruction kinds, consistent with current per-Attachment static counting. Choose and document concrete identifiers/schema during implementation. |
| D12 | What compatibility is required? | Reject removed source forms in the new language; reserve removed Instruction Codes and old random code 63 without reuse or decoding; invalidate old executable IR via a version change. Preserve unrelated published error identities and transport aliases. Decide separately which observable host schemas require version changes. |
| D13 | What does CMP compare and write? | Confirmed: `?=` compares Data Stack top A with selected register B without popping; writes 0 if equal, 1 if A > B, and 2 if A < B. |
| D14 | What does CMP do on an empty Data Stack? | Preserve the register and stack, count the executed Primary, and move normally, consistent with existing empty-stack POPADD/NAND behavior. |
| D15 | How is CMP encoded and accounted for? | ASCII-sum numbering: CMP `?=` is 124 and RandomDirection `??` is 126. Existing non-encodable forms remain non-encodable. CMP supports `*`, `=`, and `x2`–`x5`; each execution counts one operation and one CMP runtime kind, with independent `CMP` Primary capability. |

### ASCII-sum numbering audit

The user explicitly states that instruction numbering is fixed by symbol ASCII sums. This replaces the earlier recommendation to preserve RandomDirection code 63 after renaming it; `??` must have number 126 under this rule. CMP `?=` has number 124. Prefix sums are 111, 112, and 113 for `?0`, `?1`, and `?2`, respectively; assigning a token a numeric identity does not itself make it a Primary or an Instruction Stack item.

A computed audit of all 70 proposed Primary spellings finds these duplicate sums:

| Sum | Tokens |
| --- | --- |
| 84–92 | `$0` / `#1` through `$8` / `#9`, respectively |
| 93 | `]` / `$9` |
| 94 | `^` / `.0` |
| 96 | `$<` / `.2` |
| 98 | `$>` / `.4` |

The current 73 Primary spellings additionally collide at 95 (`#<` / `.1`) and 97 (`#>` / `.3`); removing IfZero eliminates those two pairs. Adding existing suffix tokens and the new prefix tokens to the proposed audit produces no additional collisions. CMP and renamed RandomDirection have no collisions.

The user confirms retaining the current exclusions of FoldedBlock, Custom, CustomReturn, immediate output, and Halt from Instruction Codes. With these exclusions, the proposed encodable Primary subset has no collisions. Prefixes and suffixes remain outside the Primary Instruction Code namespace. No token renaming is needed to resolve the sums above.

### Additional semantic and compatibility review

- CMP replaces B with the comparison result while retaining A on the stack. Programs needing B later must preserve it explicitly; no implicit flag register is introduced.
- Prefixes are independent tests, not an `if`/`else` chain. In `?0+ ?1.`, a successful first cell changes 0 to 1, so the next cell can also execute. A comparison result is ordinary mutable register data.
- In Custom code, CMP and PUSH/POPADD/NAND use the internal thread's own Data Stack. READ/OUTPUT use the outer caller's stack.
- Prefix evaluation precedes Primary behavior and suffix effects. In `?1}=`, the test uses the old pointer. In `?0?=`, the test uses B before CMP overwrites it.
- A skipped instruction still moves and wraps across normal-board edges. Skipping a return does not terminate the thread; skipping a Halt does not request termination. Existing tick/work and Custom limits still bound execution.
- Control-flow analysis must retain the false fall-through path of guarded direction changes, Halt, and returns. Existing tail-call proofs must not assume those instructions always execute.
- Random bytes must migrate from 63 to 126 in source using DECODE/ENCODE or self-modification, not just rename `?` tokens. Bytes 63, 95, 97, 129, and 153 no longer decode; under the existing invalid-DECODE rule they are counted no-ops, not runtime errors.
- Prefixes are fixed cell metadata under D9. Snapshots and debug views must show them even after Primary replacement or clearing. Hosts must not infer conditions from instruction numbers.

These findings are implementation and migration requirements. All previously pending semantic questions are resolved by the user's approval.

## 4. Accepted grammar and semantic examples

Under D1–D4:

```text
instruction-cell := [condition-prefix] primary [suffix-attachment]
condition-prefix := "?0" | "?1" | "?2"
```

Parse the complete atom using the shared Rust language authority. Do not consume a valid prefix and ignore a malformed remainder. `??` and `?=` are complete Primaries; `?0??` is a guarded RandomDirection and `?1?=` is a guarded CMP. The `=` inside `?=` is part of the Primary token, not a WriteCode suffix. Under D15, `?==` is CMP followed by WriteCode, whereas `?=*` is CMP followed by ReadCode. Standalone `?0`, `?1`, and `?2` are detached prefixes and invalid. Do not accept multiple prefixes, whitespace-separated prefixes, or value forms beyond the three approved bytes.

| Atom | Proposed interpretation |
| --- | --- |
| `?0+` | Add only when the selected register is 0. |
| `?1.` | Output only when it is 1. |
| `?2>` | Set direction Right only when it is 2. |
| `#0` | Existing Custom 0 call. |
| `?1#0` | Call Custom 0 only when the selected register is 1. |
| `?0[0=` | Conditional Function call with the existing deferred WriteCode suffix. |
| `?1+x3` | Conditional Repeat; it adds once from 1 to 2, then skips and leaves on the next tick. |
| `?2;` | Halt only when the selected register is 2. |
| `?0.3` | Output literal byte 3 only when the selected register is 0. |
| `??` / `?0??` | RandomDirection / RandomDirection guarded by register equality to zero. |
| `?=` | Compare stack top A with register B and write 0, 1, or 2 without popping. |
| `?1?=` | Execute CMP only if the selected register is 1 before CMP. |

Examples for nonempty stacks: A = 7, B = 7 gives 0; A = 9, B = 4 gives 1; A = 4, B = 9 gives 2. A = 255, B = 0 gives 1. In every case the original top A remains on the stack.

CMP provides a result for a later conditional cell. `?1?=` first tests the old register value and only then performs CMP; its prefix does not test the result of the same CMP. Under D7 and D15, `?=x3` recomputes B after each committed repetition while retaining A. For A = 2 and initial B = 5, the three successive register results are 2, 0, and 1; repeats must not cache the first comparison result.

Examples describe the implemented new syntax. Under D12, migrate `#^`, `#v`, `#<`, `#>` to `?0^`, `?0v`, `?0<`, `?0>` in source and rename standalone random `?` to `??`. Do not claim that replacing a legacy conditional instruction carrying a suffix preserves behavior: the legacy suffix can execute after an unsuccessful direction test, whereas D5 suppresses it. Encoded IfZero bytes and self-modifying instruction-stack programs also need manual review.

## 5. Implementation sequence

### Gate A: Freeze the contract

1. Use the approved D1–D15 and examples as the contract; do not reopen resolved semantics for implementation convenience.
2. Record accepted decisions in `docs/decisions.md`.
3. Update source and VM specifications, the encoding table, error triggers, level rules, and affected host contracts together.
4. Select IR and any required API/ABI/schema version changes. Do not bump unrelated contracts automatically or silently reuse old semantic versions.

Exit criterion: no unresolved decision affects parsing, execution phases, static acceptance, metrics, or external interpretation.

### Gate B: Canonical model and compiler pipeline

Read each module's closest `AGENTS.md` before edits.

1. Add a bounded canonical prefix type for exactly 0, 1, 2; remove IfZero from new-language inventories; add CMP `?=` and rename RandomDirection to `??`.
2. Keep existing suffix representation separate from the prefix. Propagate the prefix through syntax, HIR, compiler lowering, and IR.
3. Preserve complete-atom parsing, comments, round-trip formatting, UTF-8 spans, and diagnostics.
4. Update IR constructors and verification: initial Primary required, Entry/Empty exclusions, prefix cardinality/value bounds, suffix matrix, and contextual restrictions.
5. Review derived tail-call analysis for both true and false control paths. Conservatively disable optimizations involving prefixes until equivalence is proven. A conditional RETURN does not guarantee a return path.
6. Remove obsolete decode mappings; encode CMP as 124 and RandomDirection as 126, leaving all other retained Primary codes unchanged. Test encode/decode round trips and rejected obsolete codes. Audit actual IR interchange paths before changing their codecs.

Exit criterion: accepted source lowers to verified IR; invalid source or host-provided IR cannot bypass the new rules.

### Gate C: Shared VM execution

1. Evaluate the condition before Primary dispatch and side effects in Normal, Repeat, and Fold execution.
2. On false, suppress Primary and suffix, apply the agreed phase/movement transition, and preserve transaction/rollback behavior.
3. Keep skipped READ, OUTPUT, writes, and Halt out of effect conflict resolution. Skip RandomDirection without consuming PRNG state.
4. Preserve the agreed AfterCall/FoldResume rules, Function frames, Custom isolation, and synchronous Custom execution.
5. Preserve fixed prefixes across Primary mutation and EMPTY clearing; test true and false revisits.
6. Update operation metrics, instruction variety, visited-cell accounting, work limits, snapshots, and trace/event contracts as required by recorded decisions.
7. Use the same deterministic transition for `step` and bounded `run`.
8. Implement CMP using a non-consuming Data Stack peek and a staged selected-register write; apply existing register conflicts and transaction rollback. Apply the approved empty-stack rule and recalculate comparisons on repeated executions.

Exit criterion: phase transitions and observable effects match the contract, including rollback and retry.

### Gate D: Level and host boundaries

1. Update level capability validation, grouped identifiers, whitelist checks, generated-code policy, static metrics, fixtures, and schema documentation. Conditions must not hide forbidden Primaries from static validation.
2. Keep language and level adapters thin; they convert explicit data and delegate to shared Rust behavior.
3. Update runtime request/response conversions, code snapshots/events, CLI debug protocol, LSP presentation, and WASM transports only where their actual exposed shape changes.
4. Update VS Code highlighting, completion, hover, formatter integration, snippets, help, and debug rendering. Do not introduce an editor-side semantic parser or interpreter.
5. Migrate examples, current tests, distribution documentation, and relevant task documents. Preserve historical evidence as clearly labelled history where appropriate.

Exit criterion: every supported caller interprets the same prefix and exposes compatible results and diagnostics.

### Gate E: Verification and delivery

- Run `cargo fmt --all -- --check`, `cargo check --workspace`, and focused module tests during implementation.
- Run `cargo test --workspace` after integrating the pipeline and VM changes.
- Run the repository's applicable VS Code build/test checks and actual native/browser-WASM/server-WASM comparison harnesses after examining their instructions.
- Report exact commands, results, missing tools, and untested hosts. Compilation alone is not execution parity.
- Review the diff for stale active IfZero references, broken links, accidental version changes, and unrelated user modifications.

Exit criterion: required acceptance cases pass and every verification limitation is explicit.

## 6. Required acceptance matrix

| Group | Required cases |
| --- | --- |
| Syntax | Every prefix value; every eligible Primary; permitted suffix combinations; standalone Custom and RandomDirection; CMP `?=` versus its WriteCode suffix (`?==`); guarded CMP/Custom/CustomReturn; malformed/truncated atoms; detached/duplicate prefixes; invalid values; comments; formatting round trips. |
| Static validation | Entry/Empty exclusions; undefined targets even under always-false-looking conditions; existing CALL/RETURN placement; Custom nesting prohibitions; Folded Block restrictions; invalid external IR. |
| Register selection | R0 and moved pointer; equality against 0/1/2; values 3 and 255; pointer-changing Primary tests the old pointer. |
| CMP | Equal/greater/less; unsigned extremes 0/255; selected pointer other than R0; stack length and complete contents preserved; empty-stack rule; prefix tests pre-CMP B; repeated comparisons use updated B; concurrent register-write conflicts and rollback; Custom thread-local stack and context registers; Instruction Code round trip and suffix behavior. |
| Skips | No register/stack/input/output/memory/code/PRNG/call effects; no conditional Halt; direction preserved; normal four-direction wrapping; Fold horizontal wrap and vertical exit. |
| Repeat | Initial false; true throughout; true then false; no incorrect waiting at the cell; rollback does not advance repetitions; sibling code mutation reviewed under existing mutable-Primary rules. |
| Calls | False CALL creates no frame; successful guarded CALL returns despite changed pointer/register; deferred suffix exactly once; false RETURN retains frame; false CustomReturn retains internal thread; guarded Custom shell and internal work. |
| Mutable code | Replacement retains prefix/suffix; ReadCode reads only Primary; EMPTY clearing; true/false cleared-cell visits; removed codes do not decode as new instructions. |
| Concurrency | Sibling register write does not affect same-tick condition; skipped effects do not conflict; executing effects still conflict; errors outrank Halt; failed ticks expose no committed events. |
| Accounting | Successful and false prefix evaluation costs; skipped Primary/suffix variety; cell visits; Custom internal work; nonterminal work/tick yields and retry. |
| Levels and hosts | Prefix permissions; forbidden guarded/generated Primary; static counts; matching normalized outputs, status, errors, metrics, and traces in actual native/browser/server execution. |

## 7. Current delivery state

Implementation is complete. IR format is version 2. Runtime API, debug protocol, and host ABI envelope versions remain unchanged; normal code views add a nullable `prefix` field and folded code views include the prefix in their token strings. Level permissions are `CMP` and independently authorized `CONDITION_0`, `CONDITION_1`, and `CONDITION_2`. Runtime metrics use `Compare` and a shared `Condition` kind. Functions containing a conditional prefix conservatively disable tail-call optimization, as recorded in the VM contract.

Verification completed on 2026-10-03:

- `cargo check --workspace`, `cargo fmt --all -- --check`, and `cargo test --workspace --no-fail-fast` passed.
- Focused runtime coverage includes 14 conditional/CMP tests for unsigned comparisons, empty stacks, Repeat, tick-start reads, deferred suffixes, mutable code, Custom isolation, conflict rollback, retired codes, and run/step equivalence.
- `node scripts/check_error_codes.js` passed: 249 scoped identities and 456 emitted references.
- Actual language execution passed all 75 shared fixtures in native CLI, browser WebAssembly, Node WebAssembly, and Wasmtime server WebAssembly, including complete-result comparison, lifecycle, limits, and yielding checks.
- Actual level execution passed 41 complete semantic comparisons in native CLI, browser module worker, Node WebAssembly, and Wasmtime.
- The VS Code TypeScript build and real editor suite using the release CLI and LSP passed 124 tests.
- `git diff --check` passed.

Host checks use a configured WebAssembly memory maximum of 67,108,864 bytes. Shared cases and focused tests provide concrete coverage; they do not constitute an exhaustive proof for every possible program. Source and executable IR compatibility intentionally break as specified in D12. Legacy programs with suffixes, encoded retired instruction bytes, and self-modification require the migration review described above.

### Follow-up implementation review

The follow-up review corrected Folded Block condition-permission rejection paths to identify the exact folded cell, removed stale CLI capability guidance, and updated current host fixture counts. The older Level JSON walkthrough is explicitly marked historical. Added coverage verifies false Function/Custom calls and returns, CMP ReadCode suffix encoding, and exact folded rejection paths. Focused tests, the full Rust workspace, error-registry checks, and actual 41-case level comparisons were rerun after the correction.

### Documentation audit (2026-10-03)

All specification documents were reviewed against the recorded decisions:

| Contract | Current documentation |
| --- | --- |
| Source | Complete Primary inventory, prefix/suffix grammar, Folded restrictions, and retired syntax. |
| VM | Prefix-first Normal/Repeat transitions, CMP behavior, calls/resumes, mutation, metrics, codes, and IR version. |
| Runtime API | Nullable prefix projection in the main snapshot contract and unchanged envelope versions. |
| ExactIO | CMP and per-value permissions, unconditional static validation, and static/dynamic metric rules. |
| Level Core | Explicit CMP/prefix whitelist rules; Level format 1 remains unchanged. |
| Level errors | Existing rejection identities and exact folded-cell paths; no new identity. |
| Error registry/catalogs | Detached prefix and folded suffix triggers, suffix-specific compatibility, and preserved stable numbers. |
| Specification index | Current syntax, IR version, and migration link. |

The root README, CLI field inventory, IR version guidance, editor feature list, current host fixture counts, and level traceability were synchronized. Architecture, safety accounting, distribution, and transport documents were reviewed; dependency direction, work-unit definitions, API versions, and host ABI layouts do not change. Historical decision entries, old walkthroughs, dated verification reports, and artifact hashes remain explicitly historical rather than being rewritten as new evidence.
