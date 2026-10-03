# Full Language Specification Status

The current language includes fixed conditional prefixes `?0`–`?2`, CMP `?=`, and random direction `??`; executable IR is format 2. See the [migration manual](../docs/conditional-prefix-migration.md) for semantics, retired forms, compatibility, and verification.

The [error code specification](codegrid-error-codes.md) defines stable source, IR, VM, API, CLI, debug, editor, LSP and WASM error categories, response shapes, compatibility and state effects. Its machine-readable registry is [codegrid-error-codes.json](codegrid-error-codes.json).

The [four-digit error table](codegrid-error-numbers.md) lists every numeric
presentation identity alongside its original code and trigger.

The [source specification](codegrid-source-spec.md) defines the normative Full source syntax and static acceptance contract, including Main, Function, Custom, and Folded Block structures, the complete Primary and Attachment inventory, scopes, dimensions, references, and diagnostics. Source decisions are recorded in [`docs/decisions.md`](../docs/decisions.md), and compiler conformance tests cover the accepted rules.

The [VM specification](codegrid-vm-spec.md) defines the normative Full execution contract for multi-thread transitions, calls and resumes, Custom invocations, Folded Blocks, Repeat, stacks, memory, Page, randomness, self-modifying code, transactions, errors, metrics, deterministic work accounting, and determinism. Remaining direct conformance cases are listed in [Section 17](codegrid-vm-spec.md#conformance-coverage-gates) and tracked in [`docs/decisions.md`](../docs/decisions.md).

The [Runtime API specification](codegrid-runtime-api-spec.md) defines the normative Full host-neutral contract in v3, including requests, responses, instance lifecycle, exact wide integers, limits, and errors. Browser bindings and Server ABI v4 are separate versioned host contracts. See [`docs/decisions.md`](../docs/decisions.md) for language and host decisions. Historical source/IR/VM tests and fixtures are supplemental evidence; normative wording and the shared Full conformance suite define current acceptance.
