# Status Flag and directionless READ

Status: implemented on 2026-10-08 after the user authorized F and READ execution. The shared Rust language core, native CLI, Runtime API projections, editor metadata, and language WASM hosts implement this contract.

## Source and instruction contract

The current Full language adds the conditional prefix `?!`, which succeeds exactly when the executing context's F is 1. It follows all existing prefix placement, suffix compatibility, evaluation order, movement, accounting, and permission rules. A false condition skips both Primary and suffix and preserves F. Testing F does not clear it. Only one prefix per cell is allowed. `?!^` conditionally turns up; `?!}` conditionally moves the register pointer right. `?!]` is a conditional Function RETURN and remains valid only on Function Boards.

Single-character `,` replaces `,^`, `,v`, `,<`, and `,>`. Reject each removed form as a complete atom, including guarded or suffixed forms; never split it into READ and a direction. READ preserves direction on both outcomes. On success, consume the next input byte, write the selected register, and set F=0. On exhaustion, consume nothing, preserve the register, and set F=1 without an error. Reading the last byte is successful; only the next attempted read observes exhaustion. No streaming, waiting, or dynamic-input protocol is introduced.

Custom READ follows the same rules using the invoking outer thread's data stack: success pops its top byte; an empty caller stack sets F=1 and preserves the Custom register. Existing caller-stack conflict and transactional routing rules remain in force.

READ has Instruction Code 44. Remove the old directional READ mappings; their codes are not reserved for compatibility. Bytes absent from the current instruction table follow the existing invalid-DECODE counted-no-op rule. Other current codes are unchanged by this design. `?!` has ASCII sum 96, which also belongs to `$<`; this is harmless because prefixes are not Primary Instruction Codes and are never encoded on the Instruction Stack. ReadCode/WriteCode still represent only Primary, preserving the fixed prefix and suffix.

Keep CMP `?=` and its unsigned comparison semantics, permissions and code 124. CMP preserves F. Keep `+`, `-`, NEG `$!`, and NAND `$&` spellings; NEG uses the eight-bit complement defined by the current source and VM specifications. Do not add a standalone POP, CMP alternative, EOF, or Carry instruction.

## F updates

F is one bit owned by the active execution context, not a shared register or an error indicator. Each actual execution overwrites F only for the following Primaries:

| Primary | F=1 | F=0 |
| --- | --- | --- |
| READ `,` | Input source is exhausted | Successful byte read |
| INC `+` | Old byte is 255 | Otherwise |
| DEC `-` | Old byte is 0 | Otherwise |
| POPADD `)` | Empty data stack, or nonempty addition has mathematical sum greater than 255 | Nonempty addition without carry |
| SHIFT LEFT `$<` | Old bit7 is 1 | Old bit7 is 0 |
| SHIFT RIGHT `$>` | Old bit0 is 1 | Old bit0 is 0 |
| Pointer left `{` | R0 wraps to R9 | Otherwise |
| Pointer right `}` | R9 wraps to R0 | Otherwise |

Empty POPADD sets F=1 while preserving the selected register and empty stack. It remains a counted normal operation with ordinary movement, not an error. This explicitly extends the existing empty-stack no-op behavior. Empty and nonempty NAND preserve F. Arithmetic still wraps modulo 256; INC/POPADD detect unsigned carry and DEC detects borrow.

Every other operation preserves F, including CMP, PUSH, NEG, NAND, CLEAR, OUTPUT, immediate output, direction, random direction, codec, memory, Page, ReadCode/WriteCode, HALT, Empty, Entry, and board movement or boundary wrapping. Calls and returns switch the active F scope as described below. Existing runtime errors retain their identities and return surfaces; F does not replace them.

## Scope, timing, and transactions

Top-level Main Entry threads start independently with F=0. Each new VM instance starts fresh; Debug and ordinary execution use identical semantics.

CALL copies the caller's active F along with its ten registers and pointer into a private Function invocation. Save the caller F with its frame. RETURN discards the callee F and restores the caller F unchanged. Recursive calls repeat this rule. Eligible self-tail reuse starts with the current callee F and preserves the original suspended caller F. Deferred CALL suffixes run against restored caller state; existing RETURN suffix behavior is unchanged.

Folded Blocks (the proposal's macros) share the caller's active F and retain their changes on exit. They introduce no private flag.

Each Custom Main Entry thread starts with its own copy of the invoking caller's F, overriding the top-level F=0 initialization rule. Internal threads then update independently. Custom Functions use the same private Function rules. Custom completion discards all internal F values and preserves the outer caller F.

Prefix evaluation uses active F in the relevant context transition-start snapshot, alongside the existing selected-register snapshot rules. Repeat reevaluates its prefix before every attempted execution. A false result cancels remaining repetitions under the existing movement rule and preserves F. Each successful repetition computes its own F; the final committed result survives. Folded and Custom work retain the existing context snapshot and execution ordering rules; do not introduce an independent flag scheduling model.

F changes are staged with all other instruction effects. Transition conflicts and runtime errors roll back active and suspended F with the entire outer transition. Host work yields must preserve resumability and the existing atomic visibility rules. Flags in different threads do not conflict, but a rejected shared effect rolls back their staged changes. Empty concurrent READs may independently set F=1 without an input/register conflict; successful concurrent reads retain existing conflicts.

Expose active `status_flag` (0 or 1) in thread snapshots and `saved_status_flag` in call frames, including Custom/internal and suspended Function state wherever those states are projected. Use existing ThreadChanged before/after snapshots for flag changes; do not invent a shared RegisterChanged flag event. Debug UI selects the active thread/frame flag. These required fields are exposed by the current hosts.

## Unpublished language contracts

The project has no user-published release. Apply the [release policy](../AGENTS.md#release-and-compatibility-policy): update current source, IR and language consumers directly, with no old-generation support, migration layer or mandatory IR version bump. Verified IR must match the current model and reject invalid representations; validation is not a promise to support historical IR. The language-level meanings of directionless READ, F, and `?!` are defined by the source and VM specifications; game-specific permissions and scoring belong to the Rust Game Runtime.

## Implementation acceptance gate

Implementation covers the shared Rust syntax/model/IR/compiler/VM, runtime projections, CLI/LSP/VS Code metadata and diagnostics. Hosts must not implement another parser or interpreter. Add focused source rejection, code round-trip, byte arithmetic, empty/nonempty POPADD, shift, pointer, prefix, Repeat cancellation, mutable-code, frame isolation, Custom initialization, rollback, work-yield, run/step and reset cases. Verify the current instruction inventory and IR validation without adding historical compatibility tests. Check register conflicts separately from thread-local F changes.

Compare complete state and debug events in actual native, browser-WASM and server-WASM language hosts before claiming parity. Rebuild language artifacts and record truthful provenance; update language authoring, examples, teaching text, reference programs and F displays. Game Runtime consumer verification is recorded in the downstream repository.

Current language acceptance: Rust workspace tests, focused F VM tests including
all byte values for arithmetic/shifts, nested calls, mutable READ, work-yield
and concurrent rollback, plus model/IR/source inventory tests. The Full fixture
suite compares complete native CLI, Chromium, Node WASM and Wasmtime state and
events. The native CLI/LSP and VS Code language-debug tests cover active and
suspended F watches. Game-specific scene fixtures, evaluator identities,
downstream app tests and artifact hashes are maintained in the Rust Game Runtime
repository and its Lang history archive.
