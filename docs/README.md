# Documentation Index

## Full language specifications

- [CodeGrid source specification](../spec/codegrid-source-spec.md): normative Full `.cg` syntax and static acceptance.
- [CodeGrid VM specification](../spec/codegrid-vm-spec.md): normative Full execution, errors, snapshots, metrics, and determinism.
- [CodeGrid Runtime API specification](runtime-api-spec.md): normative Full v5 lifecycle, request/response, generic step/work-unit limits, and language error contract.
- [AI contribution rules](../AGENTS.md): project language, dependency direction, prohibited shortcuts, and verification requirements.
- [Mandatory downstream synchronization](downstream-synchronization.md): required impact review and same-task updates to `C:/source/bf-steam-wt` after language changes.
- [Shared error code specification](../spec/codegrid-error-codes.md) and [machine-readable registry](../spec/codegrid-error-codes.json): language identities and their generated language consumers. Game error identities are maintained by the Rust Game Runtime.

## Game-domain documentation moved to the game repository

The current authority for levels, scenes, game metrics and scoring, host evaluation, and game fixtures is the [Rust Game Runtime documentation index](../../bf-steam-wt/rust/docs/README.md), its [specifications](../../bf-steam-wt/rust/spec/README.md), [examples](../../bf-steam-wt/rust/examples/README.md), and [fixtures](../../bf-steam-wt/rust/fixtures/README.md).

Former active `level-*`, `scene-*`, and game-specific `gas-*` content has moved to the Rust Game Runtime. Lang maintains no Gas/Size policy or history documents. Game scene examples and level/scene fixtures also live in the Rust Game Runtime, with no active game example or fixture trees remaining here.

Raw instruction and VM transition semantics remain owned by the language specifications. Runtime API v5 exposes committed steps and generic host work-unit protections; game tick/Gas/Size policy, case aggregation, scene rules, game error identities, and result judgment belong to the Rust Game Runtime. Historical game decisions and application verification formerly mixed into Lang records are archived in the [Game Runtime history index](../../bf-steam-wt/doc/codegrid-lang-history/README.md).

## Language implementation and conformance

- [Full language task book](../tasks/spec-completion.md): implementation and verification scope derived from the source, VM, and Runtime API specifications.
- [Conditional prefix migration manual](conditional-prefix-migration.md): approved conditional-prefix, random-direction, and CMP design.
- [Native CLI](cli.md): local Full `check`/`run`/`debug` contract, arguments, JSON schema, and exit codes.
- [Architecture](architecture.md): language crate dependencies and host boundaries.
- [Implementation roadmap](roadmap.md): Full language delivery status and acceptance gates.
- [IR design](ir.md): Full executable representation and verified-program boundary.
- [Testing strategy](testing.md): Full source, VM, editor, CLI, and host acceptance.
- [Examples status](../examples/README.md): language examples and cases awaiting review.
- [Decision log](decisions.md): dated language and VM decisions; current contracts remain the specifications.

The shared Full conformance suite and reviewed historical source/VM cases provide current language acceptance evidence. See the [test fixture guide](../tests/README.md).

## Game history and current synchronization

The [Game Runtime documentation index](../../bf-steam-wt/rust/docs/README.md) and its specs are the current authority for game behavior. The [history index](../../bf-steam-wt/doc/codegrid-lang-history/README.md) holds dated game decisions and verification reports formerly mixed into Lang documentation. The [downstream synchronization guide](downstream-synchronization.md) records the current ownership boundary, workflow, and immutable-revision reproducibility limit.
