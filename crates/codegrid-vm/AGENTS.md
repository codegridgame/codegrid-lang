# Module Rules: `codegrid-vm`

## Responsibility

Execute verified Full CodeGrid IR with deterministic multi-thread semantics and return structured state, events, errors, committed-step counts, and prepared candidate footprints. The VM owns registers, thread pointers and stacks, Pages, sparse memory, mutable code, calls, Folded Blocks, Custom invocations, Attachments, and all Full Primary instruction behavior.

## Allowed dependency and call direction

- This crate may depend only on `codegrid-ir` and `codegrid-model` among workspace crates.
- `codegrid-cli` and `codegrid-runtime-api` may create isolated VM instances and call step/run/snapshot APIs.
- The Full VM receives verified IR, explicit input, a seed, and optional initial memory. Game resource prices, budgets, and scoring do not belong in this crate. `run(max_steps)` bounds outer transitions; a generic host work-unit limit bounds synchronous thread dispatches within a transition. Custom is a production language instruction and has no VM-specific invocation cap. `run` must use the same transition semantics as repeated `step` calls.
- The IR verifier is the single trust boundary for Full structures, board shapes, Entry counts, instructions, and Attachments. Do not duplicate that validation in the VM constructor.
- Expose owned snapshots for callers that need detached state and borrowed snapshot views for read-only inspection; do not clone the full VM state merely to inspect it.
- A host work-unit limit counts thread dispatches and interrupts only at a dispatch boundary; over-limit transitions must roll back without changing VM state or committed-step counts.
- Each prepared `StepReport` exposes the candidate's exact consumed `work_units`, counting Outer and Custom dispatches together, and `custom_internal_dispatches`, counting only dispatches in Custom scopes (including nested Custom invocations), plus its `StepFootprint`. These observations are candidate-local and are not accumulated as VM statistics. A work-limit preparation error reports the fully consumed `maximum_work_units` and its exact Custom-scope subset in `custom_internal_dispatches`; when a repeated Custom state proves the invocation cannot return, remaining dispatches are counted logically so both values still match the requested cap exactly.
- Hosts executing untrusted or potentially nonterminating programs must use work-limited APIs for single-step, batch, and debug execution. The low-level `step` and `prepare_step` methods are unbounded and may not return when Custom execution does not terminate.
- `set_thread_execution_location` is a generic host/debug operation between transitions. It accepts only an existing normal thread and an in-bounds coordinate within its current board; it changes only position and direction and contains no game selection policy.
- Keep this method at the low-level VM host boundary; do not expose it through Runtime API or WASM contracts.
- Keep deterministic work units distinct from VM semantic errors and from actual engine fuel, wall-clock, or memory budgets.

## Prohibited

- Do not parse source, accept unchecked IR, read files/environment/network/clocks, or use hidden host randomness.
- Do not depend on CLI, LSP, VS Code, browser/JavaScript/WASM bindings, a particular WASM runtime, or game rules.
- Do not add scoring, animation timing, sound, levels, telemetry, or UI policy to VM events or state.
- Do not use unordered iteration where it can affect observable language behavior.
- Implement the specified deterministic random, register-selection, stack, memory, function, Folded Block, Custom, Attachment, and Repeat semantics in the shared VM; do not delegate language behavior to a host.
- Treat an unsupported Primary or non-normal internal thread phase reaching dispatch as an internal invariant fault; source acceptance and verified-IR shape remain the verifier's responsibility.

## Validation

- `set_thread_execution_location` is an explicit host/debug operation between transitions. It accepts only an existing normal thread in a running VM and an in-bounds coordinate within its current board. It changes only position/direction, preserving all execution resources and metrics; it contains no selection or game policy. Runtime API and WASM contracts do not expose it.

Test every Full transition phase, Primary and Attachment behavior, input/output, concurrent conflicts, commit/rollback, calls/resumes/tail calls, Folded Blocks, Custom isolation and work-limit rollback, deterministic random vectors, snapshots, events, and raw observations. Run the shared Full fixtures and compare native and actual WebAssembly hosts when available.

The Rust-only `replace_input` atomically replaces unread bytes between completed
steps after reserving the whole vector, preserving registers, state, and
committed-step counts.
The game runtime owns the policy and timing for requesting input replacement.

## Register-call contract (2026-10-26)

The canonical Primaries are `CallRegister` (`[r`, code 205) and
`FoldedBlockRegister` (`$r`, code 150). Use the shared model inventory. Function
Repeat and both register calls inside Folded Blocks are forbidden. Runtime
selection, empty-slot execution and per-pass target reads belong only to VM.
