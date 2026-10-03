# Documentation Index

- [Error code specification](../spec/codegrid-error-codes.md) and [machine-readable registry](../spec/codegrid-error-codes.json): stable identifiers, triggers, return surfaces and compatibility.

## Full language specifications

- [CodeGrid source specification](../spec/codegrid-source-spec.md): normative Full `.cg` syntax and static acceptance.
- [CodeGrid VM specification](../spec/codegrid-vm-spec.md): normative Full execution, errors, snapshots, metrics, and determinism.
- [CodeGrid Runtime API specification](../spec/codegrid-runtime-api-spec.md): normative Full v3 lifecycle, request/response, resource-limit, and error contract.
- [AI contribution rules](../AGENTS.md): project language, dependency direction, prohibited shortcuts, and verification requirements.

## Local development task book

- [Level Core v1 specification](../spec/codegrid-level-core-spec-v1.md) and [Level Core architecture](level-core-architecture.md): shared Rust ExactIO level validation/evaluation and WASM integration boundaries for Web, Steam, and backend hosts.
- [ExactIO implementation contract](../spec/codegrid-level-exactio-contract-v1.md): decided first-phase schema, capabilities, metrics, seeds, execution limits, and failure priorities.
- [Rust level evaluation task book](../tasks/level-core-exactio-v1.md): ExactIO implementation, Environment extension design, native CLI evaluation, WASM delivery, and actual-host parity acceptance.
- [Level Host API v1](level-api-v1.md), [WASM transports v1](level-wasm-v1.md), and [safety accounting](level-safety-accounting.md): versioned integration and trusted local profiles.
- [Environment extension design](level-environment-extension.md): future Rust scene interfaces and author checklist; no production scenes are supported.
- [Five-scene architecture](level-scenes-architecture.md): selected product scenes, shared engines, Rust catalog, protocol gaps, and delivery sequence.
- [Level implementation traceability](level-traceability.md): normative rules, test coverage, reproducible host comparisons, and remaining integration dependencies.
- [Level build provenance](level-build-provenance.md): pinned local toolchain, exact evaluator source identity, and artifact/toolchain reports.
- [Level host integration readiness](level-host-integration-readiness.md): candidate Steam/backend workspace, existing ABI boundaries, and required actual-host acceptance.

- [Full language task book](../tasks/spec-completion.md): local implementation and verification scope derived from the source, VM, and Runtime API specifications.
- [Conditional prefix migration manual](conditional-prefix-migration.md): approved `?0`/`?1`/`?2`, `??`, and non-consuming CMP `?=` design, implementation sequence, and acceptance matrix.
- [Native CLI](cli.md): decided local Full `check`/`run` contract, including arguments, JSON schema, and exit codes.
- [Architecture](architecture.md): crate dependencies, shared semantic core, and host boundaries.
- [Implementation roadmap](roadmap.md): local Full delivery status and acceptance gates.
- [IR design](ir.md): Full executable representation and verified-program boundary.
- [Testing strategy](testing.md): Full source, VM, editor, CLI, and host acceptance.
- [Examples status](../examples/README.md): identifies Full feature examples and cases awaiting review.
- [Decision log](decisions.md): Full language decisions and open questions.

The shared Full conformance suite and reviewed historical source/VM cases provide current acceptance evidence. See the [test fixture guide](../tests/README.md).
