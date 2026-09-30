# Documentation Index

- [Error code specification](../spec/codegrid-error-codes.md) and [machine-readable registry](../spec/codegrid-error-codes.json): stable identifiers, triggers, return surfaces and compatibility.

## Full language specifications

- [CodeGrid source specification](../spec/codegrid-source-spec.md): normative Full `.cg` syntax and static acceptance.
- [CodeGrid VM specification](../spec/codegrid-vm-spec.md): normative Full execution, errors, snapshots, metrics, and determinism.
- [CodeGrid Runtime API specification](../spec/codegrid-runtime-api-spec.md): normative Full v3 lifecycle, request/response, resource-limit, and error contract.
- [AI contribution rules](../AGENTS.md): project language, dependency direction, prohibited shortcuts, and verification requirements.

## Local development task book

- [Full language task book](../tasks/spec-completion.md): local implementation and verification scope derived from the source, VM, and Runtime API specifications.
- [Native CLI](cli.md): decided local Full `check`/`run` contract, including arguments, JSON schema, and exit codes.
- [Architecture](architecture.md): crate dependencies, shared semantic core, and host boundaries.
- [Implementation roadmap](roadmap.md): local Full delivery status and acceptance gates.
- [IR design](ir.md): Full executable representation and verified-program boundary.
- [Testing strategy](testing.md): Full source, VM, editor, CLI, and host acceptance.
- [Examples status](../examples/README.md): identifies Full feature examples and cases awaiting review.
- [Decision log](decisions.md): Full language decisions and open questions.

The shared Full conformance suite and reviewed historical source/VM cases provide current acceptance evidence. See the [test fixture guide](../tests/README.md).
