# Module Rules: `codegrid-vm`

## Responsibility

Execute verified Full CodeGrid IR with deterministic multi-thread semantics and return structured state, events, errors, and raw metrics. The VM owns registers, thread pointers and stacks, Pages, sparse memory, mutable code, calls, Folded Blocks, Custom invocations, Attachments, and all Full Primary instruction behavior.

## Allowed dependency and call direction

- This crate may depend only on `codegrid-ir` and `codegrid-model` among workspace crates.
- `codegrid-cli` and `codegrid-runtime-api` may create isolated VM instances and call step/run/snapshot APIs.
- The Full VM receives verified IR and explicit boundary, input, seed, initial-memory, Custom-limit, and tick-limit data. `run` must use the same outer-tick transition semantics as repeated `step` calls.
- The IR verifier is the single trust boundary for Full structures, board shapes, Entry counts, instructions, and Attachments. Do not duplicate that validation in the VM constructor.
- Expose owned snapshots for callers that need detached state and borrowed snapshot views for read-only inspection; do not clone the full VM state merely to inspect it.
- A host work-unit limit counts thread dispatches and interrupts only at a dispatch boundary; over-limit ticks must roll back without changing normative VM metrics.
- Keep deterministic work units distinct from VM semantic errors and from actual engine fuel, wall-clock, or memory budgets.

## Prohibited

- Do not parse source, accept unchecked IR, read files/environment/network/clocks, or use hidden host randomness.
- Do not depend on CLI, LSP, VS Code, browser/JavaScript/WASM bindings, a particular WASM runtime, or game rules.
- Do not add scoring, animation timing, sound, levels, telemetry, or UI policy to VM events or state.
- Do not use unordered iteration where it can affect observable language behavior.
- Implement the specified deterministic random, register-selection, stack, memory, function, Folded Block, Custom, Attachment, and Repeat semantics in the shared VM; do not delegate language behavior to a host.
- Treat an unsupported Primary or non-normal internal thread phase reaching dispatch as an internal invariant fault; source acceptance and verified-IR shape remain the verifier's responsibility.

## Validation

Test every Full tick phase, Primary and Attachment behavior, input/output, concurrent conflicts, commit/rollback, calls/resumes/tail calls, Folded Blocks, Custom isolation/limits, deterministic random vectors, snapshots, events, and raw metrics. Run the shared Full fixtures and compare native and actual WebAssembly hosts when available.
