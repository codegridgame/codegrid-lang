# CodeGrid Full Virtual Machine Specification

**Status:** Normative Full execution contract. Direct acceptance cases listed in Section 17 remain implementation conformance gates.  
**Source contract:** [CodeGrid Source Language Specification](codegrid-source-spec.md)  
**Executable input:** verifier-produced Full IR only

This specification covers deterministic execution of a verified Full CodeGrid program: state transitions, concurrent effects, instructions, calls, Folded Blocks, Custom invocations, mutable code, diagnostics, events, metrics, and seeded randomness. Source parsing, host JSON schemas, UI behavior, game scoring, levels, and deployment policy are outside this document.

Historical Full VM tests and existing helper code informed the explicit decisions in [Full Language Decisions](../docs/decisions.md). They are evidence for those decisions, not independent normative sources.

## 1. Terms and state domains

- A **Global Tick** is one outer VM transition. Its committed counter starts at zero; the first attempted tick is 1.
- An **outer thread** is created for one Entry in the outer Main Board. Outer threads are evaluated once per Global Tick.
- A **Custom invocation** is a synchronous execution context entered by one outer thread. It may contain several internal threads. Its internal ticks run as part of the caller's current Global Tick and do not advance the outer committed tick counter.
- A **thread dispatch** evaluates one thread's phase for one outer or Custom tick. A deterministic host work limit counts dispatches.
- A **static cell** is identified by code grid, board, optional Folded Block ID, and coordinate. A Custom invocation does not change the static identity of its definition's cells.
- A **committed state** is VM state after an accepted Global Tick. Effects produced during a tick remain staged until the transaction commits.

Values are unsigned 8-bit bytes. Each execution context has one ten-byte register bank. Each thread has its own register pointer selecting R0 through R9. Threads in one context share registers, sparse memory, and the mutable program copy; each thread retains its own direction, pointer, Page, stacks, call frames, phase, and PRNG stream.

The outer context owns the host input queue, output byte sequence, outer memory, and mutable outer program. A Custom invocation owns a fresh register bank, memory, runtime copy of its Custom definition, internal threads, and local PRNG streams. Custom READ and OUTPUT interact with the invoking outer thread's data stack as specified in Sections 5 and 7. Custom register values, memory, code mutations, and PRNG state do not merge back into the outer context.

## 2. Verified input and initialization

The VM accepts only a verifier-produced Full IR value. Any decoded, deserialized, or externally supplied IR must pass the IR verifier before execution. The VM does not repeat source acceptance or IR validation.

The host supplies a normal-board boundary mode, Exit or Wrap; an ordered input byte sequence; a 64-bit seed; a positive Custom execution limit; optional sparse initial outer memory; and a positive bound for bounded execution.

All outer registers start at zero. Outer output starts empty. The input queue contains the supplied bytes in order. Zero-valued initial-memory entries are observably equivalent to absent entries.

The VM creates one outer thread for each Entry in the outer Main Board, in row-major board order. IDs start at zero and increase in that order. Each starts at its Entry cell and direction, with pointer R0, Page zero, empty data and instruction stacks, empty call stack, Normal phase, and deterministic outer PRNG state. Revisiting an Entry does not create another thread.

Each Custom invocation creates internal threads for every Entry in its Custom Main Board, also in row-major order with invocation-local IDs starting at zero. Registers, Page, pointers, stacks, call frames, memory, and runtime code start fresh. Custom memory is zero-initialized; outer initial memory is not copied into it. One invocation's code changes do not persist into a later invocation.

## 3. Coordinates and movement

Coordinates are zero-based: (0,0) is upper-left, x increases rightward, and y increases downward. Normal boards have positive width and height.

A Normal-phase dispatch evaluates the current cell and then makes the movement or control transition defined by its Primary and Attachment. Empty and Entry cells execute no Primary but still consume a tick and move once.

On a normal board, Exit makes a move beyond any edge a runtime error; Wrap moves across the corresponding edge. The configured normal-board boundary mode applies to Main and Function Boards, including Custom Main and Function Boards.

A Folded Block has its owner's width and one row. Horizontal movement wraps inside it regardless of the normal boundary mode. Moving vertically exits the block through a Fold Resume transition; it is not an OutOfBounds error.

## 4. Global Tick transaction

For attempted Global Tick N, where N is the prior committed count plus one, the VM evaluates every outer thread present at tick start. Thread IDs give deterministic evaluation and reporting order only; they do not determine which thread wins a write.

Every outer thread reads shared registers, memory, input, and code from the same tick-start snapshot. Private thread changes and shared effects are staged. A synchronous Custom invocation runs its internal ticks within the caller's staged transition. All dispatches in one context tick observe the same tick-start snapshot for that context.

The VM evaluates all outer threads for the attempted tick, even if a dispatch requested Halt or another dispatch already produced an error. It collects local errors and shared-effect conflicts before deciding whether to commit.

If no runtime error, conflict, or VM fault occurred, the VM atomically commits staged thread transitions and register, memory, input, output, code, caller-stack, and Custom effects, then increments the committed Global Tick. A successful Halt tick commits and increments the counter, then changes overall status to Halted. Halt ends the whole VM after all dispatches and conflict checks for that Global Tick.

If any runtime error or conflict occurs, the entire attempted Global Tick is rejected. Registers, thread positions and phases, stacks, call frames, PRNG state, Page values, input consumption, output, memory, mutable code, and caller-stack effects revert to their tick-start state. The committed Global Tick counter does not advance. Attempt metrics remain as described in Section 14. Runtime errors take precedence over a same-tick Halt request.

A step request on a terminal Halted or Error VM performs no work. The attempt number is committed tick count plus one. A failed attempt does not increment the committed tick count.

Bounded run repeatedly applies the exact same transition as step. A zero bound is invalid. If a positive bound expires while status remains Running, the result is TickLimitReached; this is not a VM runtime error and does not change the VM state. Earlier successful ticks remain committed.

A deterministic host work limit is separate from language execution. It counts each outer or Custom thread dispatch and interrupts only at a dispatch boundary. Exceeding it rejects the whole current Global Tick without changing VM state or normative VM metrics. Earlier complete ticks in a bounded run remain committed. Work-limit exhaustion is not a VM runtime error and does not make the VM terminal.

## 5. Execution phases

Each thread has an explicit phase, and the phase is part of transactional thread state.

### 5.1 Normal

Evaluate the cell at the current board and coordinate. Apply its Primary, then its allowed Attachment, then perform that instruction's movement or control transition. An ordinary instruction moves once in the resulting direction. Empty and Entry cells move once. Halt does not move.

Primary-before-Attachment is the rule for ordinary instructions with WriteCode: the current Primary executes, then WriteCode changes the Primary for later visits. CALL attachments are deferred to AfterCall as specified in Section 8.4. RETURN attachments have no runtime effect.

### 5.2 Repeat

Repeat(N) executes its attached Primary N times over N ticks, for N in 2 through 5. The first execution occurs on the tick that reaches the cell. While repetitions remain, the thread stays at that cell and does not move; it reexecutes the Primary on each following tick. After the final execution it moves once in the resulting direction and returns to Normal.

The phase records total repetitions and successful repetitions already completed. A failed tick does not advance this phase. Each Primary execution counts separately in Operation Count. Repeat itself is not an operation kind.

### 5.3 Fold

Executing a Folded Block instruction enters the referenced block at internal position (0,0) facing Right, regardless of the caller thread's current direction. The caller direction is saved separately for Fold Resume. That same Global Tick evaluates the first folded cell. The Folded Block shell and each visited block cell have distinct static cell identities.

Horizontal movement wraps inside the block. A vertical direction exits the block and records a Fold Resume phase, retaining the direction the outer thread had when entering. The following Global Tick consumes Fold Resume by moving once from the Folded Block call site in that saved outer direction, then returns to Normal. Direction changes inside the block do not replace the saved outer direction.

Folded execution uses the caller thread's register pointer, registers, Page, data stack, instruction stack, input/output context, and PRNG stream. Changes are staged with the enclosing Global Tick. A Custom call from an allowed outer Folded Block returns to the Folded Block execution state.

The IR verifier rejects forbidden Primaries in Folded Blocks. They cannot contain nested Folded Block, CALL, RETURN, or CUSTOM_RETURN. Custom calls are permitted in outer Folded Blocks only.

### 5.4 Function call and Resume

CALL consumes a tick and transfers the thread to the target Function Board's Entry. It saves the caller board, CALL-cell coordinate, and caller direction in a call frame. The caller stays at the CALL cell while the function executes. The Function Entry itself executes on a later tick.

Function execution retains the thread's register pointer, Page, data stack, instruction stack, and PRNG stream. These are not reset by CALL. Registers, memory, and runtime code remain shared within the execution context.

RETURN consumes a tick. It restores the caller board and CALL-cell coordinate and the saved caller direction, then enters AfterCall. A separate AfterCall tick advances from the call site in the saved direction and returns to Normal. Changes the function made to registers, pointer, Page, stacks, memory, code, and PRNG stream remain visible after return.

RETURN without an active call frame is a ReturnWithoutCall runtime error. Source and verified IR should reject RETURN in Main, but self-modifying code can place one there.

A statically proven self-tail CALL may reuse the current call frame. Only verifier-derived TailCallSite metadata may enable this optimization; hosts must not construct it independently. The current IR analyzer accepts a direct call from a Function to itself only if the CALL has no Attachment and every reachable post-call navigation path, for the selected boundary mode, consists only of empty cells and direction changes and reaches RETURN without looping, entering an Entry, encountering an Attachment or another Primary, or leaving the board. The analysis conservatively includes all directions that may reach the site, including both outcomes of conditional direction and reads, and every RandomDirection result. This conservative verifier rule is normative.

### 5.5 Custom invocation

Executing a Custom Primary starts a fresh isolated invocation of the referenced Custom CodeGrid. It executes synchronously inside the current outer thread dispatch. Its internal threads progress on aligned internal ticks until the invocation returns, halts, errors, or reaches the Custom execution limit.

Custom calls are available only from the outer execution context, including allowed outer Folded Blocks. A Custom CodeGrid cannot invoke another Custom. A Custom definition may contain Functions and Folded Blocks; its Function calls remain within that Custom CodeGrid.

Custom Main may have multiple internal Entry threads. Its registers and memory are shared within the invocation; direction, pointer, Page, stacks, call frames, phase, and PRNG stream are thread-local. Custom READ and OUTPUT use the invoking outer thread's data stack. Local memory and mutable code belong only to that invocation.

Custom READ consumes the top value of the caller's data stack when available and writes it to the selected Custom register. When the caller's stack is empty, it leaves the selected register unchanged and sets the encoded direction, matching exhausted outer READ behavior. Custom OUTPUT stages a push of the selected Custom register byte onto the caller's data stack. These effects are transactional with the outer Global Tick.

CustomReturn is allowed only in Custom Main. It consumes a Custom internal tick and terminates only the executing internal thread. Other internal threads continue on later aligned internal ticks. The invocation returns normally when every internal Entry thread has terminated through CustomReturn. On successful return, the caller advances once from the Custom call cell in its pre-invocation direction, in the same outer Global Tick. Custom direction changes do not alter caller direction. A Custom Halt requests whole-VM termination and does not advance the caller; a same-tick Halt takes precedence over normal Custom return, and any runtime error takes precedence over Halt.

The Custom execution limit bounds internal ticks in one invocation. Entry-only ticks and ticks executing Repeat count toward it. If the invocation remains incomplete after the allowed number of internal ticks, CustomExecutionLimitExceeded is reported in the attempted outer Global Tick and all staged effects are rolled back. Its scope identifies caller thread, Custom ID, and the exhausted internal tick. The host work limit is separate: it counts dispatches, not Custom internal ticks.

## 6. Registers, stacks, Page, and memory

### 6.1 Registers and pointer

Each context has ten shared unsigned-byte registers, initially zero. Every thread's pointer starts at R0. Moving left from R0 selects R9; moving right from R9 selects R0. Instructions that access a register use the executing thread's pointer.

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

MEMORY_LOAD reads the effective address and pushes the byte onto the current thread's data stack. MEMORY_STORE, when the data stack is nonempty, pops its top value and stages it at the effective address. MEMORY_STORE with an empty stack does nothing and does not count the address as used. Reads observe tick-start memory; a sibling write does not affect a same-tick read.

Initial outer-memory addresses and Pages are signed arbitrary-precision values. Hosts preserve them without narrowing to JavaScript Number or machine integers.

## 7. Primary instruction behavior

Every ordinary Primary except Halt is followed by its defined movement or control transition. Attachments and operation metrics are specified in Sections 8 and 14.

| Primary | Behavior |
| --- | --- |
| Direction(up/down/left/right) | Set the thread direction to the encoded direction. |
| RandomDirection | Draw once from that thread's deterministic PRNG and set its direction. |
| IfZero(direction) | If the selected register is zero in the tick-start register snapshot, set the encoded direction; otherwise retain the current direction. |
| Read(direction) | Outer context: consume one front input byte if available and stage it into the selected register. If empty, retain the register and set the encoded direction. Custom context: use the caller's stack as specified in Section 5.5; on empty, retain the register and set the encoded direction. |
| Clear | Set the selected register to zero. |
| Add / Sub | Increment / decrement the selected register modulo 256. |
| MoveRegisterPointer(left/right) | Move the thread pointer one slot in the encoded direction, wrapping between R0 and R9. |
| Output | Outer context: stage the selected byte into the shared output sequence. Custom context: stage a push onto the caller's data stack. |
| OutputImmediate(digit 0 through 9) | Stage the literal raw byte into outer output, or onto the Custom caller's data stack. Preserve all registers and the register pointer. Use ordinary Output movement, conflicts, rollback, tick and work accounting. |
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
| Custom(slot) | Start the isolated Custom invocation using Section 5.5. |
| CustomReturn | Return from Custom Main using Section 5.5. |
| Halt | Request successful whole-VM termination after the current Global Tick dispatches and conflict checks. |

Outer READ uses one shared input queue. When the queue is nonempty and exactly one thread reads in a tick, the first byte is consumed and staged for that thread's selected register. Multiple reads while the queue is nonempty conflict rather than receiving the same byte or consuming successive bytes. When input is empty, READ does not write a register or consume a byte; it sets the encoded direction and empty-queue reads do not conflict.

## 8. Attachments

A cell has at most one Attachment.

### 8.1 ReadCode

ReadCode reads the Primary at the same static cell from the tick-start mutable code snapshot and pushes its Instruction Stack item onto the executing thread's Instruction Stack. If that Primary is EMPTY, it pushes the EMPTY stack item (Instruction Code 32). It is an operation distinct from the Primary.

### 8.2 WriteCode

When the executing thread's Instruction Stack is nonempty, WriteCode pops its top item and stages replacement of the attached cell's Primary. EMPTY clears the Primary; an encodable Primary replaces it. With an empty stack, the Attachment is a counted no-op and leaves the cell unchanged.

A WriteCode change affects later visits, not the already executed Primary. It changes only the Primary field; the Attachment remains attached, including after clearing the Primary. All threads in one execution context observe tick-start code for that tick. Multiple writes to the same static cell in one context tick conflict even when replacement values are equal.

WriteCode modifies the current mutable code copy. In a Custom invocation it changes only that invocation's copy, which is discarded when the invocation ends.

### 8.3 Repeat

Repeat(N) reexecutes its attached Primary N times over separate ticks, for N in 2 through 5, as in Section 5.2. The verifier rejects unsupported counts and Repeat on CALL or RETURN. Since a cell has only one Attachment, Repeat cannot combine with ReadCode or WriteCode.

### 8.4 CALL and RETURN timing

CALL's ReadCode or WriteCode Attachment is deferred until the caller's separate AfterCall tick, after the Function returns successfully. The deferred Attachment executes once against the CALL site's tick-start mutable Primary, then the caller advances from that site in its saved direction. It does not execute if the call halts or fails before returning. The CALL Primary itself executes and is counted on the original call tick; its deferred ReadCode or WriteCode is counted on AfterCall.

ReadCode and WriteCode attached to RETURN are accepted by the source and IR compatibility matrix but have no runtime effect and do not contribute an Attachment operation or Instruction Variety entry. RETURN itself still executes and is counted normally.

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
| - | 45 |
| . | 46 |
| < | 60 |
| > | 62 |
| ? | 63 |
| $& | 74 |
| $( | 76 |
| $) | 77 |
| $+ | 79 |
| $- | 81 |
| ] | 93 |
| ^ | 94 |
| #< | 95 |
| $< | 96 |
| #> | 97 |
| $> | 98 |
| ,< | 104 |
| ,> | 106 |
| v | 118 |
| { | 123 |
| } | 125 |
| #^ | 129 |
| ,^ | 138 |
| Call(slot 0 through 9) | 139 through 148, respectively |
| #v | 153 |
| ,v | 162 |

FoldedBlock, Custom, CustomReturn, and Halt have no Instruction Code. EMPTY has code 32 but is not a Primary. Attachment tokens are not Instruction Codes.

## 10. Concurrent effects and conflicts

OutputImmediate has no Instruction Code and accepts no Attachment. It cannot
be represented on the Instruction Stack. Existing Instruction Codes remain
unchanged. All immediate forms count as the existing Output metric kind, not
as separate kinds for each digit.

Threads within one context read that context's shared register, memory, input, and code snapshots from the beginning of the context tick. Reads do not see sibling writes from that tick. An overlapping memory or code read and write is not itself a conflict; the read sees the old value.

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

A conflict rejects the whole enclosing outer Global Tick, including otherwise successful outer and Custom effects. A conflict does not choose a winner by thread order.

### 10.2 Error normalization and ordering

RuntimeError::new sorts and deduplicates participant thread IDs. RuntimeError::sort_canonical sorts and deduplicates complete errors using this exact key:

1. execution scope: Outer before Custom; Custom orders by caller thread ID, Custom ID, then internal tick;
2. error code name, lexicographically;
3. resource key, ordered by its current variants None, BoardCell, Register, Memory, CodeCell, CallerStack, then the variant's fields;
4. normalized participant thread IDs.

This produces repeatable diagnostics independent of host scheduling or container iteration. A conflict rejects the entire attempted outer Global Tick, including otherwise valid staged effects.

## 11. Runtime errors and VM faults

Runtime errors are structured data; the VM does not panic on language execution. Defined error categories are:

| Error code | Meaning and context |
| --- | --- |
| OutOfBounds | An Exit-mode move leaves a normal board; records scope, thread, board, source position, and attempted direction. |
| ReturnWithoutCall | RETURN executes without a frame; records scope, thread, board, and source position. |
| CustomExecutionLimitExceeded | A Custom invocation is incomplete at its configured internal-tick limit; records Custom scope and limit. |
| ConcurrentWriteConflict | Multiple staged writes target one register. |
| ConcurrentInputConflict | Multiple threads try to consume a nonempty input queue in one context tick. |
| ConcurrentOutputConflict | Multiple writes target one output sequence in one context tick. |
| ConcurrentMemoryWriteConflict | Multiple writes target one memory address; records address and participants. |
| ConcurrentCodeWriteConflict | Multiple writes target one code cell; records static cell and participants. |
| ConcurrentCallerStackReadConflict | Multiple successful reads target one nonempty caller data stack in one Custom tick. |
| ConcurrentCallerStackReadWriteConflict | A successful read and one or more writes target the same caller stack. |
| ConcurrentCallerStackWriteConflict | Multiple writes target one caller stack in one Custom tick. |

One attempted outer tick may report multiple errors, including errors from an outer dispatch and a Custom internal tick. A Custom internal error outranks a same-tick Custom Halt request; outer errors also prevent a same-tick Halt from committing. Runtime errors are returned in stable order as in Section 10.2.

Metric counter overflow and impossible internal state are VM faults, not source diagnostics, host request errors, runtime errors, tick-limit exhaustion, or work-limit exhaustion. Counters do not wrap or silently saturate. Invalid host configuration is rejected before a tick begins.

## 12. Snapshots and events

Owned snapshots and borrowed read-only views expose the same semantic state without forcing a clone merely for inspection. A Full snapshot includes:

- overall status and committed Global Tick count;
- ten outer registers, outer sparse memory, remaining input, accumulated output, and the mutable outer program copy;
- each outer thread's ID, board, coordinate, direction, register pointer, Page, data and instruction stacks, call frames, phase, and PRNG state;
- cumulative raw metrics, structured runtime errors, and any VM fault.

A successful step may emit ordered events for reached cells, consumed input, committed register/memory/code changes, and thread changes. Failed or rolled-back ticks emit no events and no newly committed output. A state-change event is emitted only when the committed value changes. Empty input, empty-stack WriteCode, and invalid DECODE no-ops do not emit a change event.

CellReached covers outer and Custom cells, Entry and empty cells, Folded Block shells, and visited Folded Block cells. Failed-tick cell visits remain in Used Cells but are not returned as events. Custom scope identifies caller thread, Custom ID, and internal tick. Newly emitted outer output is a per-step ordered delta and is appended to cumulative output. Custom OUTPUT changes the caller stack instead.

On a successful commit, every `CellReached` event precedes committed state-change events. Cell visits are ordered by outer thread ID; for each outer thread, its outer-context visit precedes its Custom visits, which are ordered by Custom internal tick and internal thread ID. Custom state-change events follow in `(internal tick, caller thread ID, Custom ID)` order, then Outer state-change events. Within each context tick, state events are ordered by category: `InputConsumed`, `RegisterChanged`, `MemoryChanged`, `CodeChanged`, then `ThreadChanged`. Within a category, order by thread ID, register index, `MemoryLocationId`, `StaticCellId`, or thread ID respectively. Failed or rolled-back ticks emit no events.

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

A Custom invocation by outer caller c at one-based attempted Global Tick t for Custom slot k derives:

    invocation_seed = mix64(mix64(mix64(mix64(s XOR CUSTOM_DOMAIN) XOR c) XOR t) XOR k)

Each internal Custom thread with local ID j starts at:

    mix64(mix64(invocation_seed XOR INTERNAL_DOMAIN) XOR j)

A thread's SplitMix64 state starts at the derived value. A draw replaces state with mix64(state) and returns the new state. RandomDirection takes its two low bits: 0 = Up, 1 = Down, 2 = Left, 3 = Right.

Function calls and Folded Blocks continue using the same thread stream. Each Custom invocation has independent internal streams. Repeat of RandomDirection draws once per Primary execution. PRNG state is transactional: rejecting a Global Tick restores the old state while retaining attempt metrics. Clocks, host randomness, scheduling, and unordered iteration do not affect language results.

Golden vectors:

| Derivation | Input | Expected |
| --- | --- | --- |
| Outer initial state | seed 0, thread 0 | 0x034CA5DFE1C65CBB |
| Next outer draw | previous vector | 0x531E83D8554C75ED |
| Outer initial state | seed 0, thread 1 | 0xE655A6A95CAFD120 |
| Next outer draw | previous vector | 0xF21C3460065E08B3 |
| Outer initial state | seed 1, thread 0 | 0xAEE0C8FD2B9643C3 |
| Next outer draw | previous vector | 0xFB3966F27831B1E1 |
| Custom invocation seed | seed 0, caller 0, tick 1, Custom 0 | 0xC41E47EAB76E9229 |
| Internal initial state | previous vector, internal thread 0 | 0x4C7FD3BC90D86658 |
| Next internal draw | previous vector | 0xD49736B357C0DA75 |

## 14. Raw metrics

Raw metrics describe deterministic VM work and state use; they do not affect program results.

- **Global Tick:** committed outer Global Ticks. Failed attempts do not advance it.
- **Operation Count:** each execution of a metric-bearing Primary or ReadCode/WriteCode Attachment, including repeated executions and Custom internal work. Direction-only instructions, empty and Entry cells, FoldedBlock and Custom shells, and Repeat Attachments are excluded. Halt, CustomReturn, and other executable Primary kinds are included.
- **Instruction Variety:** stable set of distinct metric-bearing Primary and ReadCode/WriteCode kinds. It is not a count. Directions, structural shells, and Repeat are excluded.
- **Used Cells:** distinct static cells reached. Identity includes code grid, board, optional Folded Block ID, and coordinate; it excludes thread and Custom invocation identity. Entry, empty cells, Folded Block shell, and Folded Block body cells count. Failed attempted visits remain counted.
- **Used Memory Addresses:** distinct logical locations accessed by MEMORY_LOAD or nonempty MEMORY_STORE. Outer memory has one identity. Custom memory identity includes attempted Global Tick, caller thread ID, and Custom ID. Absent zero-valued locations count when read. An empty MEMORY_STORE does not count. Failed attempted accesses remain counted.
- **Peak Data Stack Usage, Peak Instruction Stack Usage, Peak Call Stack Usage:** high-water values over corresponding stacks of concurrently resident outer and Custom threads at normative logical instants. A Custom caller stack is the outer caller's data stack and is counted once. Failed attempted work can raise a peak even though the transition rolls back.

Operation Count, Instruction Variety, Used Cells, and Used Memory Addresses record attempted work and survive runtime-error rollback. Stack high-water values are sampled at outer-tick boundaries and aligned Custom internal-tick boundaries. Each sample sums the stacks of all resident Outer and Custom threads at that instant; the caller's Outer Data Stack is included once. Before commit, staged pushes and call-frame growth contribute to the attempted high-water value, but staged pops do not reduce it. After a successful commit, sample the committed post-state as well. Preserve attempted peaks through runtime-error rollback. A host work-limit interruption changes no normative metric. Metric overflow is a VM fault, not wraparound.

## 15. Deterministic work accounting

Hosts may bound synchronous VM work with a positive work-unit ceiling for one
`step` or `run` call. One work unit is one scheduled dispatch of a live Outer
or Custom thread for an outer or Custom tick. Each thread dispatch consumes
exactly one unit regardless of its phase or Primary instruction. Folded Block
execution uses its Outer or Custom thread's dispatch and does not add a second
unit. Scheduler bookkeeping and state serialization do not consume VM work
units.

The counter is shared across every outer tick and every synchronous Custom
internal tick attempted by one bounded `run` call. A bounded `step` has its own
counter. Custom execution limits remain separate: they count aligned internal
Custom tick rounds per invocation, not thread dispatches or host work units.

The VM checks the ceiling before beginning each dispatch. If the next required
dispatch cannot start because the ceiling has been reached, the VM interrupts
the attempted outer Global Tick and rejects that entire tick atomically. No
state, event, output, or normative metric from the interrupted tick is
committed; previously committed ticks in the same `run` call remain committed.
The VM reports a host work-limit interruption, not a VM error or halt. If the
last required dispatch finishes exactly at the ceiling and the tick completes,
the tick commits normally; exhaustion is reported only when another dispatch
is required.

A work budget is per API call and does not become persistent VM state. Repeating
an interrupted call with the same inputs and verified program therefore
retries the same uncommitted tick deterministically. Work units bound VM thread
dispatches only; they do not bound host serialization, response size, retained
state, process memory, or wall-clock time.

## 16. Determinism requirements

Given the same verified IR, boundary mode, input bytes, seed, initial outer memory, Custom execution limit, and sequence of bounded run/step requests, the VM returns the same statuses, snapshots, output, errors, events, and raw metrics.

Determinism requires tick-start snapshots for shared reads; conflict and commit decisions based on resource identity rather than scheduling; stable thread IDs and participant lists; stable ordering for errors, events, varieties, used cells, and memory addresses; fixed-width random arithmetic; and arbitrary-precision Page/address arithmetic. No language result may depend on wall-clock time, host randomness, scheduling, hash iteration, process state, or environment.

## 17. Conformance coverage gates

The VM decisions recorded in [Full Language Decisions](../docs/decisions.md#resolved-vm-semantic-decisions) are normative. The historical evidence identified there is not a substitute for direct acceptance coverage. Before claiming Full conformance, add deterministic cases for:

1. Multiple Custom internal threads returning on different and matching ticks, including a surviving thread, same-tick Halt, error, and Custom execution-limit exhaustion.
2. CALL ReadCode and WriteCode on AfterCall, inert RETURN attachments, and CALL-site mutation by a callee.
3. ReadCode's EMPTY behavior is not reachable from verified IR: Attachments require an initial Primary, each cell has at most one Attachment, and WriteCode changes only that cell's Primary while retaining its Attachment. Preserve the specified defensive EMPTY result (Instruction Code 32), but do not require an impossible self-modify-then-ReadCode execution case. Cover ReadCode's tick-start Primary and stack usage, WriteCode clearing while retaining its Attachment, and Instruction Code 32 through reachable DECODE/ENCODE behavior.
4. Empty and nonempty POPADD/NAND register, stack, movement, and metric behavior.
5. Custom READ from empty and nonempty caller stacks, direction, register preservation, caller-stack conflicts, and coexistence with Custom OUTPUT.
6. The full cross-effect conflict matrix, canonical multi-error order, successful and failed event-order golden cases, and the stack high-water formula at outer and aligned Custom ticks.
7. One-unit-per-dispatch accounting across Outer and Custom threads, one shared budget across a bounded `run`, exact-ceiling commit, pre-dispatch interruption, full attempted-tick rollback without metric changes, and preservation of earlier committed ticks.

Tail-call eligibility and the Custom limit boundary follow the verifier metadata and rules in Sections 5.4–5.5. Keep them aligned with direct VM cases; any discovered conflict must be recorded and resolved before changing either contract.

### Coverage audit (2026-09-29)

The following status is based on direct cases in the active `state_tests_full_v2.rs` module and the VM unit tests. A fixture or implementation inspection alone does not count as a direct acceptance case.

| Gate | Status | Direct evidence and remaining work |
| --- | --- | --- |
| 1. CustomReturn completion and priority | Covered | `custom_invocation_waits_for_returns_from_every_internal_thread` and `custom_threads_returning_on_the_same_tick_complete_together` cover staggered and matching return ticks. `custom_halt_takes_priority_over_same_tick_custom_return`, `custom_internal_error_takes_priority_over_a_same_tick_halt_request`, and `custom_execution_limit_rolls_back_the_caller_stack_transaction` cover the stated priority and limit cases. |
| 2. CALL/RETURN attachments and mutable CALL site | Covered | `calls_consume_a_tick_and_return_uses_a_separate_resume_tick`, `call_write_code_runs_after_return_using_the_callees_instruction_stack`, `write_code_attached_to_return_is_inert`, `recursive_callee_aftercall_uses_the_current_mutable_call_site`, and `deferred_call_attachments_do_not_run_when_the_callee_halts_or_errors` cover both attachment kinds, successful AfterCall behavior, inert RETURN attachments, recursive site mutation, and the callee halt/error cases. |
| 3. ReadCode, WriteCode, and code 32 | Covered for reachable behavior | `read_code_pushes_the_tick_start_primary_onto_an_empty_stack`, `cleared_primary_keeps_write_code_attachment_for_later_noop_visits`, and reachable DECODE/ENCODE code-32 cases cover the acceptance requirements. The Empty-Primary ReadCode state after mutation is unreachable under verified IR and is intentionally not an execution gate. |
| 4. POPADD/NAND | Covered | `empty_popadd_and_nand_are_counted_noops`, `nonempty_popadd_updates_register_and_consumes_one_stack_value`, and `nand_complements_the_bitwise_and_of_register_and_stack_values` cover empty and nonempty state, movement, and metrics. |
| 5. Custom READ | Covered | `custom_read_transactionally_pops_the_outer_callers_data_stack`, `empty_custom_read_preserves_register_and_turns_before_later_output`, `concurrent_custom_reads_of_a_nonempty_caller_stack_conflict`, `successful_custom_read_and_output_conflict_atomically`, and `exhausted_custom_read_can_coexist_with_one_caller_stack_output` cover nonempty/empty reads, direction, caller-stack conflict, and coexistence with output. |
| 6a. Cross-effect conflict matrix and canonical errors | Covered | The 10 `effects::tests` directly exercise the resolver rules: `rejected_input_reads_do_not_create_register_conflicts`, `empty_input_allows_multiple_reads_without_consumption_conflict`, `rejected_caller_stack_reads_do_not_create_register_conflicts`, `exhausted_caller_read_can_coexist_with_one_output`, `successful_caller_read_and_output_conflict_atomically`, `successful_caller_read_with_multiple_outputs_reports_both_conflicts`, `equal_shared_writes_still_conflict_and_participants_are_sorted`, `custom_memory_conflicts_are_scoped_to_the_address`, `code_write_conflict_uses_static_program_location`, and `combined_outer_and_custom_conflicts_have_exact_canonical_order`. The combined case asserts exact Outer/Custom scope, error-code/resource ordering, and normalized participants for register, memory, code, input, output, and caller-stack errors. Active VM cases cover successful input/caller-stack reads as register writers, simultaneous equal register/memory/code/output writes, Custom invocation state isolation, transaction rollback, and end-to-end multi-error ordering (`successful_outer_input_read_conflicts_with_a_sibling_register_write`, `successful_custom_caller_read_conflicts_with_a_sibling_register_write`, `simultaneous_writes_conflict_without_thread_order_winners`, `equal_memory_writes_conflict_and_roll_back_the_entire_tick`, `equal_code_writes_conflict_and_leave_the_primary_unchanged`, `concurrent_outer_outputs_conflict_and_roll_back_the_global_tick`, `simultaneous_custom_invocations_isolate_registers_and_local_memory`, and `concurrent_writes_and_out_of_bounds_are_reported_together`). `error.rs` unit cases cover canonical scope/code/resource/participant comparisons. No additional duplicate per-row integration cases are required for this gate. |
| 6b. Event order | Covered | `successful_tick_emits_a_complete_ordered_outer_and_custom_event_golden` asserts the full event sequence across multiple Outer threads, Custom visits, and every state-event category. Failed ticks explicitly assert an empty event sequence in cases including `concurrent_writes_and_out_of_bounds_are_reported_together`, `concurrent_outer_outputs_conflict_and_roll_back_the_global_tick`, Custom caller-stack conflicts, and `deferred_call_attachments_do_not_run_when_the_callee_halts_or_errors`. |
| 6c. Stack high-water formula | Covered | `failed_aligned_custom_tick_preserves_integrated_stack_high_water_formula` drives one failing Outer tick with a Custom invocation containing two aligned internal threads. It asserts the combined Data/Instruction/Call peaks (`7/3/3`), counts the caller stack once while replacing its aligned sample, includes staged pushes without reducing peaks for staged pops, and verifies full state rollback with attempted peaks retained. `failed_aligned_custom_tick_preserves_the_aggregate_stack_high_water_formula` independently checks the arithmetic helpers and metric merge. |
| 7. Deterministic work ceiling | Covered | `work_limit_rolls_back_outer_tick_and_normative_metrics`, `work_limit_covers_nested_custom_dispatch_and_rolls_back_the_global_tick`, `bounded_run_keeps_prior_commits_but_rolls_back_the_exhausted_tick`, and `detailed_bounded_run_retains_prior_events_and_output_on_work_yield` cover one unit per dispatch, the shared bounded-run budget, exact-ceiling completion followed by pre-dispatch yield, tick rollback, unchanged interrupted-tick state/metrics, and retained earlier deltas. |

## 18. Host boundary

The VM receives verified IR and explicit data only. It does not read paths, environment variables, network, clocks, host randomness, CLI arguments, editor state, or operating-system data. CLI owns file access and process exit codes. LSP owns JSON-RPC and document lifecycle. Runtime API and WASM adapters own only their versioned request/response conversion, checks, resource ceilings, and instance lifecycle; they do not duplicate instruction tables, compilation, or VM semantics.

Core source spans remain UTF-8 byte offsets; conversion for editors is outside the VM. Browser and server hosts preserve 64-bit seeds and arbitrary-precision Page/address values without lossy JavaScript Number conversion. Native/WASM parity requires execution comparisons in actual native, browser, and server hosts; compilation alone does not prove parity.
