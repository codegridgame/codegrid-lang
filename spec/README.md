# Full Language Specification Status

The current language includes fixed conditional prefixes `?0`–`?2`, CMP `?=`, and random direction `??`; executable IR is format 2. See the [migration manual](../docs/conditional-prefix-migration.md) for semantics, retired forms, compatibility, and verification.

The [error code specification](codegrid-error-codes.md) defines stable source, IR, VM, API, CLI, debug, editor, LSP and WASM error categories, response shapes, compatibility and state effects. Its machine-readable registry is [codegrid-error-codes.json](codegrid-error-codes.json).

The [four-digit error table](codegrid-error-numbers.md) lists every numeric
presentation identity alongside its original code and trigger.

The [source specification](codegrid-source-spec.md) defines the normative Full source syntax and static acceptance contract, including Main, Function, Custom, and Folded Block structures, the complete Primary and Attachment inventory, scopes, dimensions, references, and diagnostics. Source decisions are recorded in [`docs/decisions.md`](../docs/decisions.md), and compiler conformance tests cover the accepted rules.

The [VM specification](codegrid-vm-spec.md) defines the normative Full execution contract for multi-thread transitions, calls and resumes, Custom invocations, Folded Blocks, Repeat, stacks, memory, Page, randomness, self-modifying code, transactions, errors, metrics, deterministic work accounting, and determinism. Remaining direct conformance cases are listed in [Section 17](codegrid-vm-spec.md#conformance-coverage-gates) and tracked in [`docs/decisions.md`](../docs/decisions.md).

The [Runtime API specification](codegrid-runtime-api-spec.md) defines the normative Full host-neutral contract in v3, including requests, responses, instance lifecycle, exact wide integers, limits, and errors. Browser bindings and Server ABI v4 are separate versioned host contracts. See [`docs/decisions.md`](../docs/decisions.md) for language and host decisions. Historical source/IR/VM tests and fixtures are supplemental evidence; normative wording and the shared Full conformance suite define current acceptance.

The [Scene Specification v1](codegrid-scene-spec-v1.md) defines the accepted ExactIO, Baudot, Elevator, Robot, QualityControl, and MechanicalArm protocols above the VM. Native API 2 and both WASM adapters execute these protocols; the document records the remaining direct-vector and deployment gaps.

The Scene Spec is supported by the [continuous session design](../docs/scene-session-design.md) and [scene conformance plan](../docs/scene-conformance-plan.md). They describe implementation boundaries and the evidence still required for full coverage.

The [Custom Scene architecture](../docs/custom-scenes-architecture.md) describes a future package and host-execution boundary. It is not a normative package, guest ABI, or author-file contract; current scene behavior remains governed by the versioned Scene Spec and host contracts below.

The [Scene Level JSON format v1 contract](codegrid-scene-level-json-v2.md) defines author-file structure and validation, supported scene/family combinations, metrics, and replay seeds. Its filename retains the earlier v2 name. The Rust loader, examples, and fixtures use this v1 contract; direct protocol and complete actual-host coverage remain tracked separately.

The [Scene Host Contract v2](codegrid-scene-host-contract-v2.md) freezes scene failure mapping, trusted ceilings, results, Debug feedback, and independent API/profile/transport versioning. Native dispatch and initial browser/server verification are implemented; full direct-vector and production deployment coverage remains open.
