# CodeGrid Full Virtual Machine Specification

**Current amendment (2026-10-11):** [Toroidal boards and implicit entries](../docs/decisions.md#toroidal-boards-and-implicit-entries-2026-10-11) fixes all normal boards to four-direction wrapping and supplies a default entry at (0, 0), facing Right when no explicit Entry exists.

**Previous amendment (2026-10-08):** [Status Flag and directionless READ](../docs/status-flag-and-read.md) records the approved decision implemented by the current source and VM contract.

**Current VM boundary amendment (2026-10-18):** Custom execution is enabled as a language instruction. Game counters, resource prices, scoring, and level policy are outside the VM; hosts may apply their own policy to neutral VM observations.

**Status:** Normative Full execution contract. Direct acceptance cases listed in Section 17 remain implementation conformance gates.  
**Source contract:** [CodeGrid Source Language Specification](codegrid-source-spec.md)  
**Executable input:** verifier-produced Full IR only

**Implemented (2026-10-08):** Function-private register banks and NEG are covered by direct VM tests and 81 complete native/browser/Node/Wasmtime fixtures. See [implementation evidence](../docs/function-registers-and-neg.md).

This specification covers deterministic execution of a verified Full CodeGrid program: state transitions, concurrent effects, instructions, calls, Folded Blocks, Custom invocations, mutable code, diagnostics, events, metrics, and seeded randomness. Source parsing, host JSON schemas, UI behavior, game scoring, levels, and deployment policy are outside this document.

Historical Full VM tests and existing helper code informed the explicit decisions in [Full Language Decisions](../docs/decisions.md). They are evidence for those decisions, not independent normative sources.

## 1. Terms and state domains

- An **outer transition** is one atomic evaluation of all outer threads. Its committed-step counter starts at zero; the first candidate transition is 1.
- An **outer thread** is created for one explicit Entry in the outer Main Board, or its default entry when no explicit Entry exists. Outer threads are evaluated once per outer transition.
- A **Custom invocation** is a synchronous execution context entered by one outer thread. It may contain several internal threads. Its internal transitions run as part of the caller's current outer transition and do not advance the committed-step counter.
- A **thread dispatch** evaluates one thread's phase for one outer or Custom transition. A deterministic host work limit counts dispatches.
- A **static cell** is identified by code grid, board, optional Folded Block ID, and coordinate. A Custom invocation does not change the static identity of its definition's cells.
- A **committed state** is VM state after an accepted outer transition. Effects produced during a transition remain staged until the transaction commits.

Values are unsigned 8-bit bytes. Each execution context has a ten-byte Main register bank shared by its Main threads. Each Function invocation has a private ten-byte bank and register pointer copied by value from its caller. Each thread selects R0 through R9 in its active bank. Threads in one context share sparse memory and mutable code; direction, pointer, F, Page, stacks, call frames, phase and PRNG remain thread-owned. Function banks are not shared between threads or recursive invocations.

The outer context owns the host input queue, output byte sequence, outer memory, and mutable outer program. A Custom invocation owns a fresh register bank, memory, runtime copy of its Custom definition, internal threads, and local PRNG streams. Custom READ and OUTPUT interact with the invoking outer thread's data stack as specified in Sections 5 and 7. Custom register values, memory, code mutations, and PRNG state do not merge back into the outer context.

## 2. Verified input and initialization

The VM accepts only a verifier-produced Full IR value. Any decoded, deserialized, or externally supplied IR must pass the IR verifier before execution. The VM does not repeat source acceptance or IR validation.

The host supplies an ordered input byte sequence, a 64-bit seed, optional sparse initial outer memory, and any requested execution/work bounds.

All outer registers start at zero. Outer output starts empty. The input queue contains the supplied bytes in order. Zero-valued initial-memory entries are observably equivalent to absent entries.

The VM creates one outer thread for each Entry in the outer Main Board, in row-major board order. IDs start at zero and increase in that order. Each starts at its Entry cell and direction, with pointer R0, F=0, Page zero, empty data and instruction stacks, empty call stack, Normal phase, and deterministic outer PRNG state. Revisiting an Entry does not create another thread. If no explicit Entry exists, one thread with ID zero starts at (0, 0), facing Right, with the same initial state. It executes the actual cell contents on the first transition; no synthetic Entry is inserted.

Each Custom invocation creates one internal thread at (0, 0), facing Right when its Main has no explicit Entry, or internal threads for every explicit Entry in its Custom Main Board, also in row-major order with invocation-local IDs starting at zero. Registers, Page, pointers, stacks, call frames, memory, and runtime code start fresh. Each internal Entry copies the invoking caller's active F independently. Custom memory is zero-initialized; outer initial memory is not copied into it. One invocation's code changes do not persist into a later invocation.

## 3. Coordinates and movement

Coordinates are zero-based: (0,0) is upper-left, x increases rightward, and y increases downward. Normal boards have positive width and height.

A Normal-phase dispatch evaluates the current cell and then makes the movement or control transition defined by its Primary and Attachment. Empty and Entry cells execute no Primary but still consume a transition and move once.

Every normal board wraps in all four directions. Left and Right edge crossings preserve the row; Up and Down edge crossings preserve the column. Edge crossings preserve direction and use the ordinary single movement transition, without an extra transition or operation. This applies to Main and Function Boards, including Custom Main and Function Boards.

A Folded Block has its owner's width and one row. It never wraps: movement that would leave the block exits it through a Fold Resume transition.

## 4. Outer transition transaction

For attempted outer transition N, where N is the prior committed count plus one, the VM evaluates every outer thread present at transition start. Thread IDs give deterministic evaluation and reporting order only; they do not determine which thread wins a write.

Every outer thread reads its active register bank and shared memory, input and code from the same transition-start snapshot. Private thread and Function-bank changes and shared effects are staged. A synchronous Custom invocation runs its internal transitions within the caller's staged transition. All dispatches in one context transition observe the same transition-start snapshot for that context.

The VM evaluates all outer threads for the candidate transition, even if a dispatch requested Halt or another dispatch already produced an error. It collects local errors and shared-effect conflicts before deciding whether to commit.

If no runtime error, conflict, or VM fault occurred, the VM atomically commits staged thread transitions and register, memory, input, output, code, caller-stack, and Custom effects, then increments the committed outer transition. A successful Halt transition commits and increments the counter, then changes overall status to Halted. Halt ends the whole VM after all dispatches and conflict checks for that outer transition.

If any runtime error or conflict occurs, the entire attempted outer transition is rejected. Registers, active and suspended F, thread positions and phases, stacks, call frames, PRNG state, Page values, input consumption, output, memory, mutable code, and caller-stack effects revert to their transition-start state. The committed outer transition counter does not advance. A prepared candidate's attempted observations remain only in its `StepReport` as described in Section 14. Runtime errors take precedence over a same-transition Halt request.

A step request on a terminal Halted or Error VM performs no work. The candidate ID is the committed-step count plus one. A failed attempt does not increment the committed-step count.

Bounded run repeatedly applies the exact same transition as step. A zero bound is invalid. If a positive bound expires while status remains Running, the result is StepLimitReached; this is not a VM runtime error and does not change the VM state. Earlier successful transitions remain committed.

A deterministic host work limit is separate from language execution. It counts each outer or Custom thread dispatch and interrupts only at a dispatch boundary. Exceeding it rejects the whole current outer transition without changing VM state or committed-step counts. Earlier complete transitions in a bounded run remain committed. Work-limit exhaustion is not a VM runtime error and does not make the VM terminal.

## 5. Execution phases

Each thread has an explicit phase, and the phase is part of transactional thread state.

### 5.1 Normal

Evaluate the cell at the current board and coordinate. Evaluate its conditional prefix first, as specified in Section 8.0. If the condition succeeds or no prefix is present, apply its Primary, then its allowed suffix Attachment, then perform that instruction's movement or control transition. A false condition skips both and moves once in the current direction. An ordinary instruction moves once in the resulting direction. Empty and Entry cells move once. Halt does not move.

Primary-before-Attachment is the rule for ordinary instructions with WriteCode: the current Primary executes, then WriteCode changes the Primary for later visits. CALL attachments are deferred to AfterCall as specified in Section 8.4. RETURN attachments have no runtime effect.

### 5.2 Repeat

Repeat(N) executes its attached Primary N times over N transitions, for N in 2 through 9. The first execution occurs on the transition that reaches the cell. While repetitions remain, the thread stays at that cell and does not move; it reexecutes the Primary on each following transition. Before each attempted execution, reevaluate the fixed prefix against that context's transition-start selected register. A false condition cancels remaining repetitions, returns to Normal, and moves once in the current direction. After the final successful execution it moves once in the resulting direction and returns to Normal.

The phase records total repetitions and successful repetitions already completed. A failed transition does not advance this phase. Each Primary execution counts separately in Operation Count. Repeat itself is not an operation kind.

### 5.3 Fold

Executing a Folded Block instruction enters the referenced block at internal position (0,0) facing Right, regardless of the caller thread's current direction. The caller direction is saved separately for Fold Resume. That same outer transition evaluates the first folded cell. The Folded Block shell and each visited block cell have distinct static cell identities.

A Folded Block never wraps. Movement that would leave the block — vertical in either direction, Left past the left edge, or Right past the right edge — records a Fold Resume phase, retaining the direction the outer thread had when entering. The following outer transition consumes Fold Resume by moving once from the Folded Block call site in that saved outer direction, then returns to Normal. Direction Primaries are forbidden inside Folded Blocks, so the internal sweep direction stays Right for the whole visit.

Folded execution uses the caller thread's register pointer, registers, Page, data stack, instruction stack, input/output context, and PRNG stream. Changes are staged with the enclosing outer transition. A Custom call from an allowed outer Folded Block returns to the Folded Block execution state.

The IR verifier rejects forbidden Primaries in Folded Blocks. They cannot contain nested Folded Block, CALL, RETURN, CUSTOM_RETURN, direction changes, or random directions. Custom calls are permitted in outer Folded Blocks only.

### 5.4 Function call and Resume

CALL consumes a transition and transfers the thread to the target Function Board's explicit Entry, or (0, 0), facing Right when it has no Entry. It saves the caller board, CALL-cell coordinate, and caller direction in a call frame. The caller stays at the CALL cell while the function executes. The target cell itself executes on a later transition, including an instruction at a default entry.

CALL saves the caller's register-bank ownership and pointer and creates a private Function bank by copying all ten values and the pointer from the caller's active transition-start state. A sibling's same-transition shared-register writes do not affect that copy. Nested calls and ordinary recursion copy the current Function's private bank into a new independent invocation. Page, data stack, instruction stack and PRNG retain their existing thread ownership and are not copied or restored by CALL/RETURN. Memory and runtime code retain their context ownership. Outer input/output and Custom caller-stack I/O routing are unchanged; the language VM owns no scene state.

RETURN consumes a transition. It discards the callee's private bank and pointer without copying either back, restores caller bank ownership and saved pointer, restores the caller board, CALL-cell coordinate and direction, then enters AfterCall. A separate AfterCall transition advances from the call site in the saved direction and returns to Normal. Page, stacks, memory, code, PRNG and I/O changes retain their existing behavior. Return to Main reselects the live shared Main bank: it must not overwrite sibling writes with an old CALL snapshot. Return to a suspended Function resumes that caller's unchanged private bank. AfterCall uses the restored caller state.

Folded Blocks introduce no register scope and use their caller's active bank.
Functions inside Custom use the same copy/discard rule within that invocation;
Custom initialization and isolation remain unchanged. Self-tail CALL frame
reuse preserves the current active register values and pointer as the next
invocation's initial state, but retains the original suspended caller state
for the eventual RETURN. Tail eligibility and call-depth behavior are unchanged.
Errors, Halt and work yield do not synthesize a RETURN. Ordinary
whole-transition rollback includes private banks and suspended callers.

RETURN without an active call frame is a ReturnWithoutCall runtime error. Source and verified IR should reject RETURN in Main, but self-modifying code can place one there.

A statically proven self-tail CALL may reuse the current call frame. Only verifier-derived TailCallSite metadata may enable this optimization; hosts must not construct it independently. The current IR analyzer conservatively excludes any Function containing a conditional prefix from tail-call optimization. For remaining Functions it accepts a direct call from a Function to itself only if the CALL has no Attachment and every reachable post-call navigation path, under fixed four-direction wrapping, consists only of empty cells and direction changes and reaches RETURN without looping, entering an Entry, encountering an Attachment or another Primary. The analysis conservatively includes all directions that may reach the site, including the false fall-through of guarded control flow and both outcomes of reads, and every RandomDirection result. This conservative verifier rule is normative.

### 5.5 Custom invocation

Executing a Custom Primary starts a fresh isolated invocation of the referenced Custom CodeGrid. It executes synchronously inside the current outer thread dispatch. Its internal threads progress on aligned internal rounds until the invocation returns, halts, or encounters a runtime error.

Static Custom call placement permits only the outer execution context, including allowed outer Folded Blocks. A Custom CodeGrid cannot invoke another Custom. A Custom definition may contain Functions and Folded Blocks; its Function calls remain within that Custom CodeGrid.

Custom Main may have multiple internal Entry threads. Its Main registers and memory are shared within the invocation; Functions use private banks under Section 5.4. Direction, pointer, Page, stacks, call frames, phase, and PRNG stream are thread-local. Custom READ and OUTPUT use the invoking outer thread's data stack. Local memory and mutable code belong only to that invocation.

Custom READ consumes the top value of the caller's data stack when available and writes it to the selected Custom register. When the caller's stack is empty, it leaves the selected register unchanged and sets F=1; success sets F=0. Direction is unchanged in both cases. Custom OUTPUT stages a push of the selected Custom register byte onto the caller's data stack. These effects are transactional with the outer transition.

CustomReturn is allowed only in Custom Main. It consumes a Custom internal round and terminates only the executing internal thread. Other internal threads continue on later aligned rounds. The invocation returns normally when every internal Entry thread has terminated through CustomReturn. On successful return, the caller advances once from the Custom call cell in its pre-invocation direction, in the same outer transition. Custom direction changes do not alter caller direction. A Custom Halt requests whole-VM termination and does not advance the caller; a same-transition Halt takes precedence over normal Custom return, and any runtime error takes precedence over Halt.

The VM has no Custom-specific execution cap or runtime error. Every live Outer and Custom thread dispatch consumes one unit from the host work budget. A host work limit can interrupt a nonterminating Custom invocation at a dispatch boundary; the entire candidate outer transition, including effects staged by other Outer threads and earlier Custom rounds, is then rolled back. The low-level unbounded `step` and `prepare_step` APIs may not return for a nonterminating Custom program; hosts running untrusted programs must use work-limited APIs.

## 6. Registers, stacks, Page, and memory

### 6.1 Registers and pointer

Each context's Main bank has ten shared unsigned-byte registers, initially zero. Every Main thread's pointer starts at R0. A Function starts with its caller's active bank and pointer copied by value. Moving left from R0 selects R9; moving right from R9 selects R0. Every register access, including prefixes, CMP, READ, OUTPUT, memory addressing, DECODE/ENCODE and Attachments, uses the executing thread's active bank and pointer.

### 6.2 Data stack

Each thread owns a LIFO data stack. PUSH copies the selected register byte onto it. When a value is available, POPADD consumes the top byte and adds it to the selected register modulo 256. When a value is available, NAND consumes the top byte and replaces the selected register with the eight-bit complement of that byte AND the prior register value.

With an empty Data Stack, POPADD and NAND are counted no-ops that preserve the selected register and stack and perform ordinary movement. MEMORY_STORE with an empty stack is a no-op and does not access or count a memory address.

### 6.3 Instruction stack and Instruction Codes

Each thread owns a LIFO Instruction Stack. Its elements are EMPTY or an encodable Primary. EMPTY has byte code 32.

ENCODE with a nonempty Instruction Stack pops the top item and writes its code to the selected register. ENCODE with an empty stack leaves the register unchanged.

DECODE converts the selected register byte to an Instruction Stack item and pushes it when it is 32 or a defined encodable Primary code. An invalid byte is a counted no-op: register and stack stay unchanged and no runtime error is raised.

Structural instructions and control forms without an Instruction Code cannot be represented as stack items.

### 6.4 Page and sparse memory

Each thread's Page is an arbitrary-precision signed integer, initially zero. Page increment and decrement add or subtract one without machine-integer wrapping.

The effective memory address is Page × 256 + current register byte.

Memory is sparse and zero-initialized. Absent addresses read as zero. Writing zero removes an address from the sparse representation without changing its observable value. Outer threads share outer memory; threads in one Custom invocation share that invocation's separate memory.

MEMORY_LOAD reads the effective address and pushes the byte onto the current thread's data stack. MEMORY_STORE, when the data stack is nonempty, pops its top value and stages it at the effective address. MEMORY_STORE with an empty stack does nothing and does not count the address as used. Reads observe transition-start memory; a sibling write does not affect a same-transition read.

Initial outer-memory addresses and Pages are signed arbitrary-precision values. Hosts preserve them without narrowing to JavaScript Number or machine integers.

### 6.4 Status flag F

F is a thread-owned bit in the active Main or Function context. INC sets it
on carry from 255; DEC on borrow from 0; POPADD on an empty stack or unsigned
sum above 255. Nonempty POPADD without carry sets it to zero. SHIFT LEFT uses
old bit7 and SHIFT RIGHT uses old bit0. Pointer left sets it on R0-to-R9 wrap;
pointer right sets it on R9-to-R0 wrap. Each of these operations overwrites F
with zero when its stated condition is false. READ follows Section 7.

All other instructions preserve F, including empty/nonempty NAND, NEG, CMP,
CLEAR, codec, memory, Page, output, direction, Attachments and movement.
An empty POPADD preserves the register and stack while setting F=1.
False prefixes preserve F and skip both Primary and suffix. Repeat checks the
current flag before every attempted execution; a false test cancels the rest.

CALL copies active F into the callee and saves it as saved_status_flag in the
frame. RETURN discards callee F and restores the saved caller value. Self-tail
reuse retains current callee F and the original suspended caller flag. Folded
Blocks share their caller's F. Custom completion discards internal flags and
preserves the outer flag. Flags are staged and rolled back with the whole transition,
including suspended frames and work-limit interruption. ThreadChanged carries
status_flag before/after; no shared flag conflict or separate metric exists.

## 7. Primary instruction behavior

Every ordinary Primary except Halt is followed by its defined movement or control transition. Attachments and operation metrics are specified in Sections 8 and 14.

| Primary | Behavior |
| --- | --- |
| Direction(up/down/left/right) | Set the thread direction to the encoded direction. |
| RandomDirection | Draw once from that thread's deterministic PRNG and set its direction. |
| Compare | Peek thread Data Stack top A without popping, compare against selected register B as unsigned bytes, and stage 0 if equal, 1 if A > B, or 2 if A < B. With an empty stack, preserve the register and stack. Use ordinary movement and count one operation. Custom CMP uses the internal thread stack, not the caller stack. |
| Read | Consume the next outer input byte or Custom caller-stack top and write the selected register with F=0; on exhaustion preserve the register with F=1. Preserve direction in both cases. |
| Clear | Set the selected register to zero. |
| Add / Sub | Increment / decrement the selected register modulo 256. |
| Neg | Set the selected byte to `(0 - value) mod 256`, equivalently `0u8.wrapping_sub(value)`. Preserve the pointer and every other register; use ordinary movement. |
| MoveRegisterPointer(left/right) | Move the thread pointer one slot in the encoded direction, wrapping between R0 and R9. |
| Output | Outer context: stage the selected byte into the shared output sequence. Custom context: stage a push onto the caller's data stack. |
| OutputImmediate(digit 0 through 9) | Stage the literal raw byte into outer output, or onto the Custom caller's data stack. Preserve all registers and the register pointer. Use ordinary Output movement, conflicts, rollback, transition and work accounting. |
| Push | Push the selected register byte onto the current thread's data stack. |
| PopAdd | Pop a data byte and add modulo 256 when present; on empty, perform the counted no-op specified in Section 6.2. |
| Decode / Encode | Convert between selected register byte and Instruction Stack according to Section 6.3. |
| Call(slot) | Enter the referenced Function Board using Section 5.4. |
| Return | Return through the current frame using Section 5.4, or raise ReturnWithoutCall. |
| Nand | Pop a data byte and set register to the eight-bit complement of register AND byte when present; on empty, perform the counted no-op specified in Section 6.2. |
| MemoryLoad / MemoryStore | Access memory as specified in Section 6.4. |
| MovePage(increment/decrement) | Add or subtract one from the signed arbitrary-precision Page. |
| Shift(left/right) | Shift selected byte left with high-bit truncation / right logically with zero entering the high bit. |
| FoldedBlock(slot) | Enter the referenced Folded Block using Section 5.3. |
| Custom(slot) | Execute the isolated Custom invocation in Section 5.5. |
| CustomReturn | Return from Custom Main using Section 5.5. |
| Halt | Request successful whole-VM termination after the current outer transition dispatches and conflict checks. |

Outer READ uses one shared input queue. When the queue is nonempty and exactly one thread reads in a transition, the first byte is consumed and staged for that thread's selected register. Multiple reads while the queue is nonempty conflict rather than receiving the same byte or consuming successive bytes. When input is empty, READ does not write a register or consume a byte; it sets F=1 and preserves direction. Empty-queue reads do not conflict; a successful read sets F=0, including the last byte.

NEG is an encodable ordinary Primary with Instruction Code 69 and distinct
metric kind `Neg`. It accepts the existing conditional prefixes and suffix
matrix, including per-transition Repeat and prefix rechecks. Every execution counts
one operation; work remains one unit per scheduled dispatch. It introduces no
instruction-specific overflow error. Game resource pricing and capability policy are outside the VM.

## 8. Attachments

A cell has at most one conditional prefix and one suffix Attachment.

### 8.0 Conditional prefixes

Before executing a cell's Primary, evaluate its fixed `?0`, `?1`, or `?2` prefix against the executing thread's selected register in the context transition-start snapshot, or `?!` against its active transition-start F=1. Testing F preserves it. Every evaluation counts one operation and the shared Condition instruction kind, including false results. On false, skip both Primary and suffix and move once in the current direction; normal boundary and Fold movement rules still apply. The visited cell, dispatch, and transition are still counted. Skipped effects, calls, PRNG draws, Halt requests, and skipped instruction metrics do not occur.

Repeat rechecks before each Primary execution. A false result cancels remaining repetitions, restores Normal phase, and moves once. CALL checks only before invocation; AfterCall runs its existing deferred suffix and movement without rechecking. FoldResume also does not recheck. Prefixes are allowed on all otherwise valid Primaries, including Folded Block contents, but do not relax static restrictions.

WriteCode replaces only Primary, retaining prefix and suffix. ReadCode and DECODE/ENCODE represent only Primary. A runtime-cleared cell still evaluates the retained prefix before the existing suffix behavior. Initial detached prefixes remain invalid. Prefix failure and CMP effects participate in the existing transaction/rollback and work-budget rules.

### 8.1 ReadCode

ReadCode reads the Primary at the same static cell from the transition-start mutable code snapshot and pushes its Instruction Stack item onto the executing thread's Instruction Stack. If that Primary is EMPTY, it pushes the EMPTY stack item (Instruction Code 32). It is an operation distinct from the Primary.

### 8.2 WriteCode

When the executing thread's Instruction Stack is nonempty, WriteCode pops its top item and stages replacement of the attached cell's Primary. EMPTY clears the Primary; an encodable Primary replaces it. With an empty stack, the Attachment is a counted no-op and leaves the cell unchanged.

A WriteCode change affects later visits, not the already executed Primary. It changes only the Primary field; the prefix and suffix remain attached, including after clearing the Primary. All threads in one execution context observe transition-start code for that transition. Multiple writes to the same static cell in one context transition conflict even when replacement values are equal.

WriteCode modifies the current mutable code copy. In a Custom invocation it changes only that invocation's copy, which is discarded when the invocation ends.

### 8.3 Repeat

Repeat(N) reexecutes its attached Primary N times over separate transitions, for N in 2 through 9, as in Section 5.2. The verifier rejects unsupported counts and Repeat on CALL or RETURN. Since a cell has only one suffix Attachment, Repeat cannot combine with ReadCode or WriteCode.

### 8.4 CALL and RETURN timing

CALL's ReadCode or WriteCode Attachment is deferred until the caller's separate AfterCall transition, after the Function returns successfully. The deferred Attachment executes once against the CALL site's transition-start mutable Primary, then the caller advances from that site in its saved direction. It does not execute if the call halts or fails before returning. The CALL Primary itself executes and is counted on the original call transition; its deferred ReadCode or WriteCode is counted on AfterCall.

ReadCode and WriteCode attached to RETURN are accepted by the source and IR compatibility matrix but have no runtime effect and contribute no Attachment operation to a candidate footprint. RETURN itself still executes and contributes its operation category normally.

## 9. Instruction Codes

These encodable Primaries have canonical byte Instruction Codes and may be stored on the Instruction Stack. Call uses the slot-specific range shown.

| Primary token/form | Instruction Code |
| --- | ---: |
| ! | 33 |
| % | 37 |
| & | 38 |
| ( | 40 |
| ) | 41 |
| + | 43 |
| , | 44 |
| - | 45 |
| . | 46 |
| < | 60 |
| > | 62 |
| $! | 69 |
| ?? | 126 |
| ?= | 124 |
| $& | 74 |
| $( | 76 |
| $) | 77 |
| $+ | 79 |
| $- | 81 |
| ] | 93 |
| ^ | 94 |
| $< | 96 |
| $> | 98 |
| v | 118 |
| { | 123 |
| } | 125 |
| Call(slot 0 through 9) | 139 through 148, respectively |

FoldedBlock, Custom, CustomReturn, and Halt have no Instruction Code. EMPTY has code 32 but is not a Primary. Attachment tokens are not Instruction Codes.

## 10. Concurrent effects and conflicts

Function register accesses target the executing invocation's private transition-start
bank. Register conflict identity includes bank ownership: writes to R0 in
different Function invocations, or to Function R0 and Main R0, do not conflict.
Input, output, memory, mutable-code and Custom caller-stack conflicts are
unchanged. Private writes roll back when any shared effect rejects the transition.
NEG is a write even for fixed points 0 and 128; it obeys existing equal-value
write conflict rules. Main threads still share the Main bank.

OutputImmediate has no Instruction Code and accepts only Repeat suffixes x2 through x9; conditional prefixes are permitted. Repeat uses the ordinary per-transition Primary execution, movement, conflicts, rollback, work accounting, and Custom caller-stack routing. ReadCode and WriteCode remain invalid. It cannot
be represented on the Instruction Stack. Existing Instruction Codes remain
unchanged. All immediate forms count as the existing Output metric kind, not
as separate kinds for each digit.

Threads read their active register bank and their context's shared memory, input and code from the beginning of the context transition. Reads do not see sibling writes from that transition. An overlapping memory or code read and write is not itself a conflict; the read sees the old value.

### 10.1 Normative conflict matrix

The Full VM uses this exact conflict matrix, implemented by the shared effect resolver.

- If input was nonempty and more than one thread staged an input read, report ConcurrentInputConflict and reject those reads as register-write candidates. Empty-queue reads do not conflict.
- A successful nonempty input read conflicts with another staged write to the same register.
- If the caller stack was nonempty, more than one staged caller-stack read reports ConcurrentCallerStackReadConflict. Rejected reads do not create register conflicts.
- More than one staged caller-stack push reports ConcurrentCallerStackWriteConflict.
- One successful caller-stack read plus any caller-stack push reports ConcurrentCallerStackReadWriteConflict. If there are multiple pushes, report both read/write and write/write errors.
- Successful nonempty caller-stack reads and input reads participate as register writes for register conflict detection.
- More than one write to one register, one MemoryLocationId, or one static code cell reports the corresponding conflict. The checks use resource identity and do not compare values, so equal-value writes conflict.
- More than one staged write to an output sequence reports ConcurrentOutputConflict, including equal bytes.
- Separate memory spaces are separate resources. MemoryLocationId distinguishes outer memory from each Custom invocation. Custom-local registers, memory, and code are resolved within their invocation.

A conflict rejects the whole enclosing outer transition, including otherwise successful outer and Custom effects. A conflict does not choose a winner by thread order.

### 10.2 Error normalization and ordering

RuntimeError::new sorts and deduplicates participant thread IDs. RuntimeError::sort_canonical sorts and deduplicates complete errors using this exact key:

1. execution scope: Outer before Custom; Custom orders by caller thread ID, Custom ID, then internal transition;
2. error code name, lexicographically;
3. resource key, ordered by its current variants None, BoardCell, Register, Memory, CodeCell, CallerStack, then the variant's fields;
4. normalized participant thread IDs.

This produces repeatable diagnostics independent of host scheduling or container iteration. A conflict rejects the entire attempted outer transition, including otherwise valid staged effects.

## 11. Runtime errors and VM faults

Runtime errors are structured data; the VM does not panic on language execution. Defined error categories are:

| Error code | Meaning and context |
| --- | --- |
| ReturnWithoutCall | RETURN executes without a frame; records scope, thread, board, and source position. |
| ConcurrentWriteConflict | Multiple staged writes target one register. |
| ConcurrentInputConflict | Multiple threads try to consume a nonempty input queue in one context transition. |
| ConcurrentOutputConflict | Multiple writes target one output sequence in one context transition. |
| ConcurrentMemoryWriteConflict | Multiple writes target one memory address; records address and participants. |
| ConcurrentCodeWriteConflict | Multiple writes target one code cell; records static cell and participants. |
| ConcurrentCallerStackReadConflict | Multiple successful reads target one nonempty caller data stack in one Custom transition. |
| ConcurrentCallerStackReadWriteConflict | A successful read and one or more writes target the same caller stack. |
| ConcurrentCallerStackWriteConflict | Multiple writes target one caller stack in one Custom transition. |

One attempted outer transition may report multiple errors, including errors from an outer dispatch and a Custom internal transition. A Custom internal error outranks a same-transition Custom Halt request; outer errors also prevent a same-transition Halt from committing. Runtime errors are returned in stable order as in Section 10.2.

Metric counter overflow and impossible internal state are VM faults, not source diagnostics, host request errors, runtime errors, step-limit exhaustion, or work-limit exhaustion. Counters do not wrap or silently saturate. Invalid host configuration is rejected before a transition begins.

## 12. Snapshots and events

Each thread exposes nullable `private_registers`: null selects the shared Main bank; otherwise it contains ten private bytes. Each call frame exposes nullable `saved_registers` and `saved_register_pointer` for its suspended caller. Outer `registers` retains its shared Main-bank meaning. Private bank changes are exposed by the existing `ThreadChanged` before/after snapshots with thread and context identity; `RegisterChanged` describes the shared Main bank. No new event kind is introduced.

Owned snapshots and borrowed read-only views expose the same semantic state without forcing a clone merely for inspection. A Full snapshot includes:

- overall status and committed outer transition count;
- ten outer registers, outer sparse memory, remaining input, accumulated output, and the mutable outer program copy;
- each outer thread's ID, board, coordinate, direction, register pointer, status_flag (0 or 1), Page, data and instruction stacks, call frames, phase, and PRNG state;
- the committed-step summary, structured runtime errors, and any VM fault.

A successful step may emit ordered events for reached cells, consumed input, committed register/memory/code changes, and thread changes. Failed or rolled-back transitions emit no events and no newly committed output. A state-change event is emitted only when the committed value changes. Empty input, empty-stack WriteCode, and invalid DECODE no-ops do not emit a change event.

CellReached covers outer and Custom cells, Entry and empty cells, Folded Block shells, and visited Folded Block cells. Failed-transition cell visits are not returned as events; prepared candidates expose the visited-cell identities through `StepReport.footprint`. Custom scope identifies caller thread, Custom ID, and internal transition. Newly emitted outer output is a per-step ordered delta and is appended to cumulative output. Custom OUTPUT changes the caller stack instead.

On a successful commit, every `CellReached` event precedes committed state-change events. Cell visits are ordered by outer thread ID; for each outer thread, its outer-context visit precedes its Custom visits, which are ordered by Custom internal transition and internal thread ID. Custom state-change events follow in `(internal transition, caller thread ID, Custom ID)` order, then Outer state-change events. Within each context transition, state events are ordered by category: `InputConsumed`, `RegisterChanged`, `MemoryChanged`, `CodeChanged`, then `ThreadChanged`. Within a category, order by thread ID, register index, `MemoryLocationId`, `StaticCellId`, or thread ID respectively. Failed or rolled-back transitions emit no events.

## 13. Deterministic randomness

Randomness uses only the host-provided 64-bit seed. Arithmetic is modulo 2^64.

Define domains:

- OUTER_DOMAIN = 0x43474F5554455231
- CUSTOM_DOMAIN = 0x4347435553544F4D
- INTERNAL_DOMAIN = 0x4347494E54455231

For 64-bit x, mix64 is:

1. a = x + 0x9E3779B97F4A7C15
2. a = (a XOR (a >> 30)) × 0xBF58476D1CE4E5B9
3. a = (a XOR (a >> 27)) × 0x94D049BB133111EB
4. result = a XOR (a >> 31)

An outer thread with seed s and ID i starts at:

    mix64(mix64(s XOR OUTER_DOMAIN) XOR i)

A Custom invocation by outer caller c at one-based attempted outer transition t for Custom slot k derives:

    invocation_seed = mix64(mix64(mix64(mix64(s XOR CUSTOM_DOMAIN) XOR c) XOR t) XOR k)

Each internal Custom thread with local ID j starts at:

    mix64(mix64(invocation_seed XOR INTERNAL_DOMAIN) XOR j)

A thread's SplitMix64 state starts at the derived value. A draw replaces state with mix64(state) and returns the new state. RandomDirection takes its two low bits: 0 = Up, 1 = Down, 2 = Left, 3 = Right.

Function calls and Folded Blocks continue using the same thread stream. Each Custom invocation has independent internal streams. Repeat of RandomDirection draws once per Primary execution. PRNG state is transactional: rejecting an outer transition restores the old state while preserving its prepared candidate observations in `StepReport`. Clocks, host randomness, scheduling, and unordered iteration do not affect language results.

Golden vectors:

| Derivation | Input | Expected |
| --- | --- | --- |
| Outer initial state | seed 0, thread 0 | 0x034CA5DFE1C65CBB |
| Next outer draw | previous vector | 0x531E83D8554C75ED |
| Outer initial state | seed 0, thread 1 | 0xE655A6A95CAFD120 |
| Next outer draw | previous vector | 0xF21C3460065E08B3 |
| Outer initial state | seed 1, thread 0 | 0xAEE0C8FD2B9643C3 |
| Next outer draw | previous vector | 0xFB3966F27831B1E1 |
| Custom invocation seed | seed 0, caller 0, transition 1, Custom 0 | 0xC41E47EAB76E9229 |
| Internal initial state | previous vector, internal thread 0 | 0x4C7FD3BC90D86658 |
| Next internal draw | previous vector | 0xD49736B357C0DA75 |

## 14. Step counts and candidate footprint

The only cumulative execution counter is **Committed Steps**: the number of
successfully committed outer transitions. Failed transitions and work-limited
candidate transitions do not advance it. Runtime snapshots and host-neutral
JSON projections expose this counter, not cumulative resource or game
statistics.

`PreparedStep::report()` exposes a `StepFootprint` for its one candidate
transition. The footprint contains:

- an operation count by instruction category for each executed metric-bearing
  Primary, conditional-prefix evaluation, or ReadCode/WriteCode Attachment;
- a set of reached static cells, identified by code grid, board, optional
  Folded Block ID, and coordinate;
- a set of logical memory locations accessed by MEMORY_LOAD or nonempty
  MEMORY_STORE, with Custom invocation identity scoped to the candidate; and
- candidate stack high-water values for Data, Instruction, and Call stacks.

Repeated operations are counted. Cell and memory-location identities are
deduplicated within this candidate only. Stack values sum concurrently resident
Outer and Custom threads at normative logical instants; an Outer caller's Data
Stack is counted once while replacing its baseline at aligned Custom internal
transitions. The footprint contains no prices, budgets, or game interpretation.
It is discarded after the prepared transition is committed or aborted and is
not accumulated in VM state or Runtime API/CLI/WASM snapshots. A host such as
the Rust Game Runtime may apply its own policy before committing the candidate.

The same candidate's `StepReport` exposes `custom_internal_dispatches`, the
checked `u64` count of dispatches in all Custom execution scopes during that
candidate. It includes nested Custom invocations and excludes Outer dispatches;
`work_units` remains the combined Outer and Custom dispatch total. The count is
available before commit and in the report returned by abort, is reset for each
prepared candidate, and is not retained in VM state. Counter overflow is a VM
fault rather than wrapping. The VM defines no Custom quota or game-specific
error: hosts may apply their own per-case policy to the candidate count before
committing.

Operation-count and stack-counter overflow are VM faults, never wraparound.
Interrupted work-limit candidates return no `StepReport` and change neither
VM state nor committed-step count. Runtime-error candidates retain their
candidate-local footprint in the `StepReport`; their state and committed
events/output remain rolled back.


## 15. Deterministic work accounting

Hosts may bound synchronous VM work with a positive work-unit ceiling for one `step` or `run` call. One work unit is one scheduled dispatch of a live Outer or Custom thread. Each dispatch consumes exactly one unit regardless of its phase or Primary instruction. Folded Block execution uses its Outer or Custom thread's dispatch and does not add a second unit. Scheduler bookkeeping and state serialization do not consume VM work units.

The counter is shared across every outer transition and every synchronous Custom internal round attempted by one bounded `run` call. A bounded `step` has its own counter. There is no separate Custom invocation cap: Custom internal dispatches use the same generic work budget as Outer dispatches.

The VM checks the ceiling before beginning each dispatch. If the next required dispatch cannot start because the ceiling has been reached, the VM interrupts the current candidate outer transition and rejects that entire transition atomically. No state, event, output, or raw observation from the interrupted transition is committed; previously committed transitions in the same `run` call remain committed. The VM reports a host work-limit interruption, not a VM error or halt. If the last required dispatch finishes exactly at the ceiling and the transition completes, it commits normally; exhaustion is reported only when another dispatch is required.

A work budget is per API call and does not become persistent VM state. Repeating an interrupted call with the same inputs and verified program therefore retries the same uncommitted transition deterministically. Work units bound VM thread dispatches only; they do not bound host serialization, response size, retained state, process memory, or wall-clock time. `step` and `prepare_step` without a work limit are intentionally unbounded and may not return for nonterminating programs.

For a prepared candidate, `StepReport.work_units` is the exact number of Outer and Custom thread dispatches consumed while evaluating it. The count is candidate-local observation data, does not change VM state or committed-step counts, and remains available from the report returned by `abort`. If `prepare_step_with_work_limit` returns `WorkLimitExceeded`, no candidate report exists; the reported `maximum_work_units` equals the work consumed because the ceiling is checked before each next dispatch.

## 16. Determinism requirements

Given the same verified IR, input bytes, seed, initial outer memory, and sequence of bounded run/step requests, the VM returns the same statuses, snapshots, output, errors, events, committed-step counts, and candidate footprints.

Determinism requires transition-start snapshots for shared reads; conflict and commit decisions based on resource identity rather than scheduling; stable thread IDs and participant lists; stable ordering for errors, events, varieties, used cells, and memory addresses; fixed-width random arithmetic; and arbitrary-precision Page/address arithmetic. No language result may depend on wall-clock time, host randomness, scheduling, hash iteration, process state, or environment.

## 17. Conformance coverage gates

### Implemented 2026-10-08 changes

`function_registers_and_neg.rs` covers all byte values, stack returns, nested/private banks and pointers, concurrent Main/private writes, rollback, work yields and self-tail reuse. Five additional shared fixtures compare complete state across native CLI, browser, Node and Wasmtime, including private snapshots and Custom calls.

The VM decisions recorded in [Full Language Decisions](../docs/decisions.md#resolved-vm-semantic-decisions) are normative. The historical evidence identified there is not a substitute for direct acceptance coverage. Before claiming Full conformance, add deterministic cases for:

1. Multiple Custom internal threads returning on different and matching transitions, including a surviving thread, same-transition Halt, error, and host work-limit interruption rollback.
2. CALL ReadCode and WriteCode on AfterCall, inert RETURN attachments, and CALL-site mutation by a callee.
3. ReadCode's EMPTY behavior is not reachable from verified IR: Attachments require an initial Primary, each cell has at most one suffix Attachment, and WriteCode changes only that cell's Primary while retaining its Attachment. Preserve the specified defensive EMPTY result (Instruction Code 32), but do not require an impossible self-modify-then-ReadCode execution case. Cover ReadCode's transition-start Primary and stack usage, WriteCode clearing while retaining its Attachment, and Instruction Code 32 through reachable DECODE/ENCODE behavior.
4. Empty and nonempty POPADD/NAND register, stack, movement, and candidate
   operation-footprint behavior.
5. Custom READ from empty and nonempty caller stacks, direction, register preservation, caller-stack conflicts, and coexistence with Custom OUTPUT.
6. The full cross-effect conflict matrix, canonical multi-error order, successful and failed event-order golden cases, and the stack high-water formula at outer and aligned Custom transitions.
7. One-unit-per-dispatch accounting across Outer and Custom threads, one shared budget across a bounded `run`, exact-ceiling commit, pre-dispatch interruption, full candidate-transition rollback without metric changes, and preservation of earlier committed transitions.

Tail-call eligibility follow the verifier metadata and rules in Sections 5.4–5.5. Keep them aligned with direct VM cases; any discovered conflict must be recorded and resolved before changing either contract.

### Coverage audit (2026-09-29)

The following status is based on direct cases in the active `state_tests.rs` module and the VM unit tests. A fixture or implementation inspection alone does not count as a direct acceptance case.

| Gate | Status | Direct evidence and remaining work |
| --- | --- | --- |
| 1. CustomReturn completion and priority | Covered | `custom_invocation_waits_for_returns_from_every_internal_thread` and `custom_threads_returning_on_the_same_step_complete_together` cover staggered and matching return transitions. `custom_halt_takes_priority_over_same_step_custom_return` and `custom_internal_error_takes_priority_over_a_same_step_halt_request` cover the stated priority; work-limited Custom cases cover atomic interruption rollback. |
| 2. CALL/RETURN attachments and mutable CALL site | Covered | `calls_consume_a_step_and_return_uses_a_separate_resume_step`, `call_write_code_runs_after_return_using_the_callees_instruction_stack`, `write_code_attached_to_return_is_inert`, `recursive_callee_aftercall_uses_the_current_mutable_call_site`, and `deferred_call_attachments_do_not_run_when_the_callee_halts_or_keeps_wrapping` cover both attachment kinds, successful AfterCall behavior, inert RETURN attachments, recursive site mutation, and the callee halt/error cases. |
| 3. ReadCode, WriteCode, and code 32 | Covered for reachable behavior | `read_code_pushes_the_step_start_primary_onto_an_empty_stack`, `cleared_primary_keeps_write_code_attachment_for_later_noop_visits`, and reachable DECODE/ENCODE code-32 cases cover the acceptance requirements. The Empty-Primary ReadCode state after mutation is unreachable under verified IR and is intentionally not an execution gate. |
| 4. POPADD/NAND | Covered | `empty_popadd_and_nand_are_counted_noops`, `nonempty_popadd_updates_register_and_consumes_one_stack_value`, and `nand_complements_the_bitwise_and_of_register_and_stack_values` cover empty and nonempty state, movement, and candidate operation observations. |
| 5. Custom READ | Covered | `custom_read_transactionally_pops_the_outer_callers_data_stack`, `empty_custom_read_preserves_register_and_sets_flag_before_later_output`, `concurrent_custom_reads_of_a_nonempty_caller_stack_conflict`, `successful_custom_read_and_output_conflict_atomically`, and `exhausted_custom_read_can_coexist_with_one_caller_stack_output` cover nonempty/empty reads, direction, caller-stack conflict, and coexistence with output. |
| 6a. Cross-effect conflict matrix and canonical errors | Covered | The 10 `effects::tests` directly exercise the resolver rules: `rejected_input_reads_do_not_create_register_conflicts`, `empty_input_allows_multiple_reads_without_consumption_conflict`, `rejected_caller_stack_reads_do_not_create_register_conflicts`, `exhausted_caller_read_can_coexist_with_one_output`, `successful_caller_read_and_output_conflict_atomically`, `successful_caller_read_with_multiple_outputs_reports_both_conflicts`, `equal_shared_writes_still_conflict_and_participants_are_sorted`, `custom_memory_conflicts_are_scoped_to_the_address`, `code_write_conflict_uses_static_program_location`, and `combined_outer_and_custom_conflicts_have_exact_canonical_order`. The combined case asserts exact Outer/Custom scope, error-code/resource ordering, and normalized participants for register, memory, code, input, output, and caller-stack errors. Active VM cases cover successful input/caller-stack reads as register writers, simultaneous equal register/memory/code/output writes, Custom invocation state isolation, transaction rollback, and end-to-end multi-error ordering (`successful_outer_input_read_conflicts_with_a_sibling_register_write`, `successful_custom_caller_read_conflicts_with_a_sibling_register_write`, `simultaneous_writes_conflict_without_thread_order_winners`, `equal_memory_writes_conflict_and_roll_back_the_entire_step`, `equal_code_writes_conflict_and_leave_the_primary_unchanged`, `concurrent_outer_outputs_conflict_and_roll_back_the_committed_steps`, `simultaneous_custom_invocations_isolate_registers_and_local_memory`, and `concurrent_writes_at_wrapped_edges_are_reported`). `error.rs` unit cases cover canonical scope/code/resource/participant comparisons. No additional duplicate per-row integration cases are required for this gate. |
| 6b. Event order | Covered | `successful_step_emits_a_complete_ordered_outer_and_custom_event_golden` asserts the full event sequence across multiple Outer threads, Custom visits, and every state-event category. Failed transitions explicitly assert an empty event sequence in cases including `concurrent_writes_at_wrapped_edges_are_reported`, `concurrent_outer_outputs_conflict_and_roll_back_the_committed_steps`, Custom caller-stack conflicts, and `deferred_call_attachments_do_not_run_when_the_callee_halts_or_keeps_wrapping`. |
| 6c. Candidate stack-footprint formula | Covered | `failed_aligned_custom_step_reports_footprint_and_rolls_back_state` exercises an Outer candidate and aligned Custom internal transitions, checking that the failed candidate reports work and stack observations while state and committed steps roll back. `aligned_custom_stack_footprint_uses_one_caller_sample` checks staged stack growth, caller-stack replacement, and the resulting candidate footprint. |
| 7. Deterministic work ceiling | Covered | `work_limit_rolls_back_outer_step_and_preserves_committed_state`, `work_limit_covers_nested_custom_dispatch_and_rolls_back_the_committed_steps`, `bounded_run_keeps_prior_commits_but_rolls_back_the_exhausted_step`, and `detailed_bounded_run_retains_prior_events_and_output_on_work_yield` cover one unit per dispatch, the shared bounded-run budget, exact-ceiling completion followed by pre-dispatch yield, transition rollback, unchanged committed-step count, and retained earlier deltas. |

## 18. Host boundary

The VM receives verified IR and explicit data only. It does not read paths, environment variables, network, clocks, host randomness, CLI arguments, editor state, or operating-system data. CLI owns file access and process exit codes. LSP owns JSON-RPC and document lifecycle. Runtime API and WASM adapters own only their versioned request/response conversion, checks, resource ceilings, and instance lifecycle; they do not duplicate instruction tables, compilation, or VM semantics.

The low-level Rust VM exposes `set_thread_execution_location` for host/debug tooling between transitions. It only changes the location and direction of an existing normal thread within its current board. Runtime API and WASM contracts do not expose this operation; game selection and other host policy remain outside the language VM.

Core source spans remain UTF-8 byte offsets; conversion for editors is outside the VM. Browser and server hosts preserve 64-bit seeds and arbitrary-precision Page/address values without lossy JavaScript Number conversion. Native/WASM parity requires execution comparisons in actual native, browser, and server hosts; compilation alone does not prove parity.

## 18. Encoding migration (2026-10-03)

Instruction Codes follow single-symbol ASCII values or the sum of both symbols' ASCII values. RandomDirection is 126 and CMP is 124. All other retained encodings and EMPTY 32 remain unchanged. Codes 63, 95, 97, 129, and 153 are no longer decoded and are not reused; DECODE handles them by the existing invalid-byte counted-no-op rule. Prefixes and suffixes are not encodable Primaries. Existing non-encodable structural, immediate-output, and Halt forms remain excluded. This migration introduced IR format 2. Current executable IR format 3 rejects formats 1 and 2 after the Function/NEG semantic change.
