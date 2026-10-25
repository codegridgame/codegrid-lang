# Function register scope and NEG

Status: implemented and verified on 2026-10-08.

The [source contract](../spec/codegrid-source-spec.md), [VM contract](../spec/codegrid-vm-spec.md), and dated [decisions](decisions.md) define these rules.

## Function register scope

Only the ten register values and register pointer gain invocation-local
ownership. Ordinary CALL copies the caller's active transition-start bank and
pointer by value. All Function register operations use that private bank.
RETURN discards it and resumes the caller's bank and saved pointer.

| Caller | Callee initial state | State resumed on RETURN |
| --- | --- | --- |
| Main thread | Copy of the context's shared Main bank and this thread's pointer | Live shared Main bank and saved thread pointer |
| Function | Copy of this invocation's private bank and pointer | Suspended caller's unchanged private bank and saved pointer |
| Custom Main thread | Copy of the invocation's shared Main bank and this internal thread's pointer | Live Custom Main bank and saved internal-thread pointer |

Returning to Main does not restore an old ten-byte Main snapshot over sibling
thread writes. For example, if Main R0 is 10 at CALL, the callee initially
receives 10. Its subsequent R0=99 remains private. With no sibling write, Main
R0 remains 10 on return; if another Main thread committed R0=20 meanwhile,
Main R0 is 20 on return. The caller's own saved pointer is restored in either
case. This preserves existing Main concurrency while isolating Functions.

Each nested or ordinary recursive call has an independent bank. Eligible
self-tail recursion reuses a frame with the current Function values and
pointer as the next invocation's initial state, preserving the original
suspended caller's bank and pointer for eventual return. Keep the existing
verifier-derived self-tail eligibility rule; do not introduce general tail
calls or broaden navigation analysis.

Retain CALL, Function Entry, RETURN and separate AfterCall transition timing.
Deferred CALL suffixes use restored caller state. Page, direction rules,
thread data/instruction stacks, PRNG, memory, mutable code and I/O retain
their existing semantics. Data stacks are shared across calls of the same
thread, not across all threads. Functions in Custom retain Custom stack I/O
routing. Folded Blocks operate on their caller's active bank and introduce
no new scope. Custom already exists and retains its existing isolation.
Scene behavior belongs to the game runtime outside this language project.

All active and suspended private state participates in the enclosing transition
transaction. Equal-value writes to one shared Main register still conflict;
same-index writes in distinct private banks do not. Shared input, output,
memory and code conflicts still reject the whole transition. Failed transitions and
host work yields restore private state as well as shared state. Halt does not
return a callee or publish its registers to Main.

No explicit parameters, return values, local memory, or local stacks are added.
A Function may communicate results through its existing thread stack or
context memory, with their existing rules.

## NEG

`$!` is the canonical token for `Neg`, an ordinary encodable Primary:

```text
selected_register = (0 - selected_register) mod 256
Instruction Code = 69
Metric kind = Neg
```

Code 69 was previously unassigned. The ASCII sum 36+33 motivates this
assignment; the explicit normative table remains authoritative, not a
generic ASCII-sum decoder. Preserve every existing code and token, including
`!` for PopClear and `$&` for NAND. No negative integer type or new error is
introduced. Existing decrement `-` remains unchanged.

| Input | NEG result |
| ---: | ---: |
| 0 | 0 |
| 1 | 255 |
| 5 | 251 |
| 127 | 129 |
| 128 | 128 |
| 255 | 1 |

For all byte values, `x + NEG(x) == 0 mod 256` and `NEG(NEG(x)) == x`.
NEG changes only its selected register as a Primary effect. It then performs
ordinary movement and any allowed suffix effect. It does not freeze position.
Unchanged-value writes still participate in conflict detection; change events
are emitted only if the committed value actually changes.

NEG accepts `?0`, `?1`, `?2`, ReadCode `*`, WriteCode `=`, and Repeat `x2` to
`x9` using existing rules. Canonical examples are `?0$!`, `$!*`, `$!=`,
`$!x2`, and `?1$!x3`. Conditions are rechecked on each repeat transition;
therefore an even count restores the original value only if all repetitions
execute. Without a prefix, `$!x2` restores 37 after two transitions and
`$!x3` produces 219 after three. Neither Unicode `×` nor detached suffixes
are source syntax.

NEG works in outer and Custom Main/Function Boards and in Folded Blocks
without a suffix, according to existing placement rules. DECODE(69) becomes
a valid instruction-stack push, ENCODE(Neg) returns 69, and WriteCode can
install NEG. The previous invalid-byte counted no-op for 69 is intentionally replaced by NEG.

Each NEG execution contributes one `Neg` operation to its candidate
`StepFootprint` and uses the existing one-work-unit-per-dispatch rule. A false
prefix contributes only its existing Condition operation. Repeat, candidate
cell observations, generic work limits and rollback remain governed by the
shared VM contract.

## Historical implementation evidence (2026-10-08)

The Rust workspace suite passes. Direct VM tests cover all 256 NEG inputs, double negation, selected pointers, Repeat/conditions, codec round trips, subtraction, nested/private banks, concurrent shared writes, self-tail reuse, conflicts and work yields. The historical game capability-test statement is preserved in the [game-side acceptance archive](../../bf-steam-wt/doc/codegrid-lang-history/function-register-neg-game-acceptance-2026-10-08.md).

All 81 Full fixtures match complete native CLI, browser, Node and Wasmtime results. VS Code tests pass with the native CLI, including separate active/suspended register scopes and stack-based Function returns. The historical Level acceptance claim for 41 legacy and 15 scene responses is preserved in the [game-side acceptance archive](../../bf-steam-wt/doc/codegrid-lang-history/function-register-neg-game-acceptance-2026-10-08.md), with its provenance limits.

At that time executable IR format 3 rejected formats 1 and 2. Runtime API v3,
browser v3, and server ABI v4 carried additive thread `private_registers` and
call-frame `saved_registers`/`saved_register_pointer`. Existing ThreadChanged
events carried private bank changes; shared RegisterChanged events kept their
Main-bank meaning. Snapshot quotas included serialized fields. These version
and evaluator statements are historical; see the current Runtime API and WASM
contracts for current versions and ownership.

The current downstream language-artifact synchronization workflow and consumer migration status are recorded in [mandatory downstream synchronization](downstream-synchronization.md). The dated editor/Web/API and generated-WASM acceptance statement formerly in this section is preserved in the [game-side acceptance archive](../../bf-steam-wt/doc/codegrid-lang-history/function-register-neg-game-acceptance-2026-10-08.md).
