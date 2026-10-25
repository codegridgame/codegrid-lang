# Full Language Specification Status

The current language includes conditional prefixes `?0`–`?2` and `?!`, CMP
`?=`, random direction `??`, directionless READ, Function-private registers,
and register-count Repeat; executable IR is format 6. The language specifications below define syntax,
compiler acceptance, VM transitions, and the Runtime API.

- [Error code specification](codegrid-error-codes.md) and [machine-readable registry](codegrid-error-codes.json): language identities and generated language consumers. Game-level error/result policy is documented in the Rust Game Runtime repository.
- [Four-digit error table](codegrid-error-numbers.md): numeric presentation identities alongside their original codes and triggers.
- [Source specification](codegrid-source-spec.md): normative Full source syntax and static acceptance.
- [VM specification](codegrid-vm-spec.md): normative Full execution contract, including transitions, calls, memory, determinism, committed-step counts, and candidate-local `StepFootprint` observations.
- [Runtime API specification](../docs/runtime-api-spec.md): normative Full v6 host-neutral lifecycle, step/work-unit limits, and transport-independent results.

## Game Runtime contracts moved to the game repository

The current Level Core, ExactIO, Scene, game-level error, author JSON, and host API contracts are maintained in the [Rust Game Runtime specification index](../../bf-steam-wt/rust/spec/README.md). Its implementation notes and verification records are in the [Rust Game Runtime documentation index](../../bf-steam-wt/rust/docs/README.md).

The former `codegrid-level-*` and `codegrid-scene-*` contract paths and fixtures have been removed from this repository. Game rules for limits, scene evaluation, and result judgment are owned by the game repository. Language VM execution semantics and candidate-local observations remain in the language specifications above; cumulative game metrics belong to the Rust Game Runtime.

The shared error registry contains language identities only; game error identities belong to the Rust Game Runtime. Runtime API v6 reports committed steps and generic work-unit limits; game resource and progression policy is owned by Rust Game Runtime.
