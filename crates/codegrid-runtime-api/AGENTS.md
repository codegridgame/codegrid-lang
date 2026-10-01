# Module Rules: `codegrid-runtime-api`

## Responsibility

Define the host-neutral, versioned request/response contract and manage verified programs and isolated VM instance lifecycles.
Re-export shared model error translation helpers for adapter presentation;
keep raw request/response error identities and details independent of locale.

## Allowed dependency and call direction

- This crate may depend on `codegrid-compiler` and `codegrid-vm` for semantic operations, and on foundational `codegrid-ir`/`codegrid-model` types for its host data contract and read-only borrowed projection views.
- Browser/server adapters call this contract and translate it to their own transport/ABI without changing semantics. The native CLI intentionally composes compiler and VM APIs directly; it is not required to depend on this host API.
- Require verified program handles, validate every handle and request, enforce explicit host limits including a per-call deterministic VM work-unit ceiling, and expose explicit release operations.
- Offer both owned snapshots for detached responses and borrowed snapshot/program views for read-only checks and streaming projections that must not clone retained VM state or program IR.
- Report work-limited single-step requests as a distinct host error; report bounded-run interruption with a distinct yield reason and the post-interruption snapshot.

## Prohibited

- Do not remove compiler diagnostic codes or change `ApiError::code()` identifiers without updating the error contract and compatibility decision.

- Do not add browser, JavaScript, WASI/server-runtime, LSP, filesystem, or process dependencies.
- Do not implement source parsing, source validation, an instruction inventory, IR lowering, VM transitions, or runtime semantics; delegate them to compiler and VM APIs.
- Do not let process-global mutable state or host scheduling/randomness affect results.
- Do not represent opaque handles as durable cross-process identifiers or accept client-asserted validation/results as authoritative.
- Do not silently truncate wide integers, byte input, diagnostics, ordered events, or metric counters.
- Do not claim these request-time limits bound cumulative VM memory, stacks, output, or snapshot/JSON size; production hosts must add and verify those budgets before executing untrusted programs.

## Validation

Test API version/error distinctions, host limits, handle isolation and release, multiple instances, and parity with the shared conformance fixtures.
