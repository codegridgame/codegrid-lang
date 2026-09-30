# Rust Level Evaluation and WASM Delivery Task Book

Status: complete for the current native CLI delivery scope.
Scope updated by explicit user decision: 2026-10-01.

The user limited this delivery to CLI implementation. Steam, backend, and Web
integration are deferred and are not completion gates for this task. The shared
Rust ExactIO core/API and native CLI are delivered and verified. Existing WASM
artifacts and local harnesses are retained as optional prior work; they do not
claim production integration. The original wider work packages below remain
as historical planning and future integration guidance.

## 1. Objective and completion boundary

Deliver one Rust level evaluator that loads logical level JSON, checks a player
`.cg` program, performs ExactIO evaluation, and returns correctness, typed
failures, metrics, constraint results, scoring values, and optional star rating.
Expose the evaluator through the native CLI for the current delivery. Browser,
Steam desktop, and backend integration belong to a future delivery.

Design the Environment extension completely at the shared orchestration
boundary, but do not implement Elevator, MaintenanceRobot, or another production
scene in this task. Unsupported scenes must return UnsupportedSceneType, never
fall back to ExactIO or appear to pass. Environment scene delivery is a separate
task following this design.

Current completion requires actual native CLI execution compared with the shared
Rust API, not only successful builds. Steam/backend/Web comparisons are deferred.

## 2. Authorities and fixed decisions

- [Level Core v1](../spec/codegrid-level-core-spec-v1.md): logical evaluation rules.
- [ExactIO implementation contract](../spec/codegrid-level-exactio-contract-v1.md): first-phase schema, capabilities, metrics, seed/shuffle rules, resource outcomes, and terminal priorities.
- [Level architecture](../docs/level-core-architecture.md): proposed Rust layers and WASM ownership.
- [Source specification](../spec/codegrid-source-spec.md), [VM specification](../spec/codegrid-vm-spec.md), and [IR design](../docs/ir.md): existing language acceptance and execution authorities.
- [Stable errors](../spec/codegrid-error-codes.md) and [registry](../spec/codegrid-error-codes.json): identifiers assigned at detection.
- [Decision log](../docs/decisions.md), [CLI contract](../docs/cli.md), and [repository rules](../AGENTS.md): compatibility, process behavior, dependencies, and verification obligations.

Preserve these decisions:

- cost is VM Operation Count; ExactIO scoring/constraints aggregate visible tests only.
- Explicit Exit/Wrap, seeds, Custom limits, evaluator identity, and trusted safety profile are part of replay/result identity.
- Support WriteCode; check initial and generated instructions against level rules. Generated-code rejection occurs after VM commit and before output acceptance, without inventing rollback or VM errors.
- Attachments have an independent explicit whitelist. Inspect all code, including unused definitions and Custom contexts.
- Debug runs all visible tests after ordinary test/constraint failures. Official runs visible and hidden tests and stops at the first established failure. Program rejection and terminal resource/fault outcomes stop evaluation.
- Tests/Environment decisions receive fresh standard VMs. Host slicing retains committed state only and cannot resume a partially executed tick.
- Tick/work call yields, cumulative resource exhaustion, logical constraint failures, compiler diagnostics, VM errors, and faults remain distinct.
- Hidden test data, positions, order, traces, and hidden metric contributions never enter public results or progress.

If implementation exposes a contract gap, resolve it in the decision log and
affected specification before depending on a new interpretation. Do not make
tests or this task book a second semantic authority.

## 3. Layers and dependency changes

Add crates only as implementation starts, with corresponding English AGENTS.md
files. Update the root graph and module rules when actual dependencies change.

```text
codegrid-level-core         -> codegrid-ir, codegrid-model, codegrid-vm
codegrid-level-api          -> codegrid-level-core, codegrid-compiler
codegrid-level-wasm-browser -> codegrid-level-api
codegrid-level-wasm-server  -> codegrid-level-api
codegrid-cli level command  -> codegrid-level-api
```

The level API may use foundational IR/model types directly if its public
contract requires them; record those actual dependencies. Existing CLI
check/run/debug continue using their current language dependencies and contracts.
Using the shared level API for the new CLI command gives CLI and WASM the same
level request/result conversion without routing existing CLI commands through
the language Runtime API.

Language crates must not depend on level policy. Existing Runtime API v3 and
Server ABI v4 remain independently versioned and unchanged unless a separately
recorded compatibility change is necessary. No second parser, instruction
inventory, compiler, interpreter, scene evaluator, metric calculator, or rating
calculator may appear in host code.

## 4. Work packages

### WP0. Publish implementable contracts and fixtures

- Review the existing ExactIO contract against real Rust types and APIs, especially Custom committed CodeChanged events and dispatch accounting.
- Publish versioned level API and both WASM transport contracts before exposing public operations. Select explicit versions without conflating them with language Runtime API/ABI versions.
- Publish a named local/test safety profile with concrete positive limits, all applicable ceilings, version, and provenance. Official callers use trusted immutable profiles; client submissions cannot relax backend limits.
- Document safety accounting for retries, permitted budget growth, state size, serialization, and terminal resource outcomes. Resource accounting must not modify VM metrics.
- Register level/API/CLI errors and typed reasons, including generated instruction/memory rejection, unsupported capabilities, numeric overflow, stale handles, and resource exhaustion.
- Establish shared valid Full `.cg` and level JSON fixtures with expected permitted results; keep malformed fixtures separate.

Acceptance: no unresolved contract choice blocks ExactIO implementation;
documented schema, numeric domains, result categories, safety profile, and golden
seed/shuffle vectors have focused fixtures. Actual findings may require a
recorded refinement rather than silently filling gaps.

### WP1. Logical schema and static validation

- Implement bounded JSON decoding with duplicate-key rejection, unknown-field rejection, exact integer handling, and format-version-first dispatch.
- Implement opaque ValidatedLevel and structural program checks over VerifiedProgram, including independent Attachment rules, dimensions, counts, threads, memory, all nested boards, and unreachable code.
- Preserve dedicated unsupported-format/scene errors and field-path validation details. Reject unregistered metrics; permit duplicate tests and weak rating configurations.
- Make Environment schema dispatch explicit without pretending examples are complete scene schemas. No unsupported scene may produce a ValidatedLevel ready for execution.
- Add reviewed example levels and companion Full programs with documented expected results.

Acceptance: invalid input never panics or reaches execution; test every rejection
family, numeric/UTF-8 boundary, zero/empty case, and supported capability family.

### WP2. Deterministic ExactIO evaluator and metrics

- Implement a unified evaluation session and convenience evaluate operation that drive the same state machine.
- Compile source once through the existing compiler at the level API boundary; use fresh VMs per test with the specified seeds, boundary, and limits.
- Implement test selection/shuffle, Debug continuation, Official fail-fast, exact output matching, empty expectations, HALT/incomplete output, and terminal-tick priorities.
- Check all committed generated-code changes, including transient Custom state discarded during the same outer tick. A final snapshot alone is insufficient. Retain typed rejection details internally and redact hidden details publicly.
- Implement the fixed metric registry, checked aggregation, constraints, per-metric rating, overall minimum rating, and unrated levels. Keep official metrics absent on failure.
- Implement bounded advancement, insufficient-budget rollback/retry, cumulative safety accounting, cancellation, faults, and released state. Distinguish test runtime failure from terminal infrastructure failure.
- Make each lifecycle phase and terminal result explicit; callbacks cannot override correctness or scoring.

Acceptance: focused cases prove VM isolation, post-commit checking, Custom
changes, tick/work yields, retry progress, output/error priorities, visible-only
metrics, static constraints, star rounding, and no final metrics on failure.

### WP3. Environment extension design without production scenes

Publish an internal design covering these interfaces and ownership rules:

- Typed scene registration, supported capability reporting, scene schema/goal validation, and immutable validated scene data.
- Deterministic persistent scene initialization, observation encoding, complete-action framing/decoding, action application, AND-goal checking, and scene failure classification.
- Fresh VM creation for each decision using normal byte input/output. Only scene state persists; a suspended VM persists only within its current decision.
- Fixed scene metric vocabularies, types, optimization directions, aggregation, constraint applicability, and metric namespace collisions.
- Initial-goal behavior, action completion/goal/constraint priority, atomic scene transition semantics, step limits, replay seed domains, and cancellation/resource cleanup.
- Bounded scene state/events and privacy-safe logical events. Animation, assets, wall-clock timing, UI selection, and Steam state stay outside the evaluator.
- A shared orchestration enum/state machine with ExactIO and future Environment paths; unified result, failure, resource, and session lifecycle handling.

Do not design arbitrary JavaScript scene callbacks or CustomJudge plugins as
v1 features. Production scene schemas and protocols require separate normative
specifications. A test-only minimal deterministic scene may exercise extension
interfaces without shipping a supported scene type or claiming Environment
feature completion. Document which test outcomes are illustrative until a
production scene contract is approved.

Acceptance: provide interface/type sketches, transition diagrams, scene-author
checklist, metric integration example, and a mapping to Level Core sections
18–33. Adding a future Rust scene must not require a new host evaluation
algorithm or a separate top-level API. Mark all unsupported scenes explicitly.

### WP4. Shared level host API and wire result

- Implement capabilities, logical-level loading, source compilation, start/advance/result, handle release, and shutdown using the same evaluator.
- Validate session-local handles, isolate instances, enforce bounded retained state, and define release/cancellation behavior for pending and terminal sessions.
- Resolve optional seeds at the entry boundary via an explicit host seed source; portable requests can always supply an explicit seed.
- Define one common result projection for CLI and WASM: level/evaluator identities, mode/configuration, status, typed failure/diagnostics, visible test results, constraint outcomes, partial/final metrics, scoring metric values/targets/directions, per-metric/overall optional rating, and replay information.
- Distinguish a terminal evaluation result from API failure and Pending. Include hidden failures only as the permitted generic category and optional typed reason.
- Serialize wide integers as canonical decimal strings with stable collection ordering. Keep full JSON responses bounded; never truncate output into apparent success.
- Exclude hidden order and data from replay information exposed to clients. Reproduction uses trusted level content and recorded seeds/configuration.

Acceptance: lifecycle, version rejection, exact integer round-trips, stale/cross-
session handles, resource failures, public redaction, and complete serialization
have positive and negative tests. CLI/WASM serialize the same semantic result.

### WP5. Native CLI level evaluation

Add a dedicated command; proposed spelling to publish in docs/cli.md before
implementation:

```text
codegrid evaluate <level.json> <program.cg>
    --mode <debug|official> --boundary <exit|wrap> --seed <u64>
    --custom-limit <positive-u64> --limits-file <trusted-profile.json>
    [--format <json|human>]
```

- Default to complete machine-readable JSON; human output is an explicit presentation option, not another result schema or scoring implementation.
- Read both files as UTF-8 with documented BOM policy and size ceilings; reject missing/unreadable files and malformed input with stable errors.
- Read the versioned safety profile as CLI host configuration. Do not allow level JSON or source to override it. Require explicit seed for reproducible initial CLI delivery.
- Do not accept level-run initial memory, register state, or inline input overrides; the level supplies test data and standard VM initialization applies.
- Drive pending sessions to completion using the shared API's documented retry policy. Return a distinct terminal resource outcome if trusted ceilings prevent completion.
- Emit one complete JSON envelope for every evaluable request, including source/level rejection. Keep human I/O diagnostics on stderr and define behavior when file loading fails before evaluation.
- Display correctness, rejection/failure category, permitted visible test feedback, constraint values/limits, final or partial metrics, chosen scoring metrics, and optional stars. No weighted total or implicit one-star rating.
- Publish command-specific exit codes for pass, player failure, source rejection, invalid/unsupported level, invalid host arguments/profile, I/O, resource termination, and fault. Existing check/run/debug exit meanings must remain compatible.
- Add help text and runnable example commands using committed fixture paths. Update CLI module rules and Cargo dependencies in the implementation change.

Acceptance: actual CLI subprocess tests read level/program files and compare
parsed JSON with direct Rust evaluation. Cover success, wrong/incomplete output,
structural/generated rejection, malformed files, unsupported scenes, constraints,
unrated/rated scoring, wide seeds, hidden redaction, streams, and exit codes.

### WP6. Browser and portable WASM delivery

- Browser adapter contains only host conversion/lifecycle glue; run bounded evaluation in a worker-compatible harness and provide a minimal JavaScript caller example.
- Portable adapter has a versioned no-import byte ABI with bounded allocation/request/response buffers and explicit lifecycle; provide backend and desktop embedding examples.
- Keep all semantic fields exact across JSON/binding conversion, including u64::MAX seeds and wide metrics. Never use JavaScript Number for wide integers.
- Pin build commands/toolchain inputs and publish artifact/API compatibility metadata. Artifact identity is recorded for replay and comparison.
- Test malformed requests, capability/version checks, handle isolation, buffer ownership, shutdown, traps, memory ceilings, and complete oversized-response errors.
- Coordinate Steam integration through its real embedding runtime. A compatible browser runtime may use the browser artifact; an embedded engine may use the portable artifact. Record the chosen integration route rather than assuming it.

Acceptance: actual browser and portable server/desktop runtimes execute the
same evaluator artifact family. Provide runnable integration harnesses without
placing Steam SDK calls, filesystem access, or game policy in the core.

### WP7. Cross-host parity and final acceptance

Use a shared fixture manifest and canonical permitted-result projection. Compare:

| Execution surface | Required evidence |
| --- | --- |
| Native Rust | Direct shared level API/evaluator baseline |
| Native CLI | Actual executable reading JSON and `.cg` files |
| Browser WASM | Actual browser worker execution; Node tests supplement it |
| Backend WASM | Actual selected server runtime executing portable WASM |
| Steam desktop WASM | Actual chosen desktop embedding runtime and integration route |

Compare statuses, stable error categories/details, seeds/configuration, visible
results, partial/final metrics, constraints, scoring values, rating, and supported
capabilities. Exclude only documented host envelope metadata, private trusted
test data, and physical timings. Do not discard meaningful differences merely
to make projections match.

Compare the same effective input/profile, source and level revision, evaluator
contract/build, and boundary/seed data. Different host slice sizes may produce
different Pending sequences; completed semantic results must match when safety
budgets allow completion. If retries consume different cumulative work, report
that resource distinction rather than claiming unconditional slice invariance.

Required fixture families:

- ExactIO pass/fail, empty input/output, READ exhaustion, duplicate tests, seeded shuffle, Debug vs Official, visible and hidden failures.
- Functions, Folded Blocks, Custom, multiple threads, Repeat, Attachments, randomness, self-modification, and generated forbidden code in outer/Custom contexts.
- Every metric/constraint mapping, static restrictions, rating thresholds including zero/max targets, overflow, aliases, no targets, and no official metrics on failure.
- Atomic output/error/HALT cases, Custom limits, insufficient work retry, exact-ceiling completion, cumulative resource termination, and faults.
- Strict JSON/UTF-8 validation, wide integers, unsupported Environment, API versioning, lifecycle/handles, buffer ceilings, and cancellation.
- Negative disclosure checks for hidden data through results, progress, errors, traces, snapshots, order, and partial metrics.

Acceptance: repeatable comparison command, per-host result report, runtime
versions, fixture count, artifact identity, and exact remaining limitations.
If the real Steam/backend host project is unavailable, finish independent
artifacts and harnesses, mark integration blocked on that named dependency,
and do not mark whole-task parity complete.

## 5. Additional preparation and delivery artifacts

- A traceability table connecting normative rules to implementation and focused test cases.
- Named local/test safety configuration, production configuration guidance, and resource/accounting limitations.
- Reviewed level JSON/Full `.cg` example pairs with expected outcomes and reproducible CLI/WASM commands.
- Published level API/browser/portable ABI and CLI evaluate contracts, error registry entries, compatibility notes, and independent version identities.
- Environment extension design and future scene-spec checklist.
- Build/run scripts for actual-host comparisons and privacy/lifecycle checks.
- Updated root/module rules, README/documentation index, architecture graph, and task progress evidence.
- Backend trust guidance: load trusted full levels, recompile submissions, certify official records server-side; hidden data distributed to clients is not secret. Steam/offline achievements remain game-host policy.

Authentication, game assets/UI, production deployment, Steam SDK behavior,
leaderboard service implementation, and production scene implementation are not
deliverables here. Their boundaries and required inputs must be documented so
hosts can integrate the evaluator without redefining its semantics.

## 6. Verification and progress record

Run cargo fmt --all -- --check, focused Rust/CLI/API tests, relevant workspace
regression tests, WASM builds, and actual-host comparison tests at their proper
milestones. Reuse language fixtures where appropriate, but existing language
parity does not prove level parity. Record exactly what ran and what it proves.

Do not add placeholders or mark a package done based on a proposal, build alone,
or an untested integration. Preserve unrelated changes. Each completed package
must link implementation, contract, tests, commands, and remaining limitations.

| Package | Status | Evidence / remaining work |
| --- | --- | --- |
| WP0 Contracts and fixtures | Delivered locally | [API](../docs/level-api-v1.md), [transports](../docs/level-wasm-v1.md), [safety accounting](../docs/level-safety-accounting.md), [errors](../spec/codegrid-level-errors-v1.md), [profile](../fixtures/levels/profiles/local-v1.json), and [36-case manifest](../fixtures/levels/conformance-v1.json); golden shuffle vectors are tested. |
| WP1 Schema and validation | Implemented and tested | [Core](../crates/codegrid-level-core/src/schema.rs) and [program checks](../crates/codegrid-level-core/src/validate.rs); 16 focused unit tests, strict/version-first decoding, canonical capability inspection, bounded tests and byte data. |
| WP2 ExactIO evaluator | Implemented and tested | [Unified session](../crates/codegrid-level-core/src/evaluate.rs), metrics/result modules and 16 evaluator tests; committed Custom guards, retry accounting, redaction, constraints, rating, and exact-ceiling outcomes. Physical memory remains host-controlled. |
| WP3 Environment design | Designed; no production scenes | [Extension design](../docs/level-environment-extension.md) includes Rust interface sketches, transitions, metric example, ownership and scene-author checklist. All scenes remain explicitly unsupported. |
| WP4 Shared host API | Implemented and tested | [Level API v1](../crates/codegrid-level-api/src/session.rs); exact wire results, instance-local handles, seed source, release/shutdown, trusted profiles, response bounds, source/level/profile replay hashes; 2 unit and 8 lifecycle tests. |
| WP5 Native CLI evaluate | Implemented and tested | [CLI contract](../docs/cli.md#evaluate-level-command); actual file/subprocess tests include 36-case full-envelope comparison with direct Rust Level API and source/level/host rejection, exact seed, streams and exit codes. Existing CLI semantics remain compatible. |
| WP6 WASM delivery | Independent artifacts and local harnesses delivered | [Browser](../crates/codegrid-level-wasm-browser) and [portable](../crates/codegrid-level-wasm-server) Level transports v1; 4 native adapter tests, actual Chrome worker and portable Node/Wasmtime execution, no-import ABI and 64 MiB encoded memory ceiling. Real game-host integration requires its embedding project. |
| WP7 Actual-host parity | Local 36-case comparison passed; external integration open | Direct Rust API, native CLI, Chrome module worker, Node portable WASM and Wasmtime 49.0.1 compare complete permitted results. No production backend or Steam desktop project/runtime integration is present; their acceptance is blocked on those named external host projects. |

### Follow-up verification (2026-10-01)

After the explicit CLI-only scope decision, `cargo test --locked -p
codegrid-level-core -p codegrid-level-api -p codegrid-cli` passed: 32 core,
10 API, and 23 CLI tests, including complete CLI/API result equality for all
36 manifest cases. The CLI provides JSON/human results, stable exit codes,
bounded UTF-8 file loading, trusted profiles, and exact seed handling. Current
native CLI delivery is complete; the host integration rows above describe
deferred work and no longer block this delivery.

A read-only search located a candidate CodeGrid Steam/API workspace at
`C:/source/bf-steam-wt`. Its target selection and backend surface remain
unconfirmed. The [integration inventory](../docs/level-host-integration-readiness.md)
records the existing version mismatch, runtime routes, and required acceptance.
No changes were made to that workspace.

The safety audit added bounded feedback/result retention, private generated
diagnostics, actual transport negative tests, and pinned build provenance.
Core tests now total 16 unit plus 16 evaluator cases; API tests total 2 unit
plus 8 lifecycle cases. The locked workspace suite and actual four-host
36-case comparison passed. See the [follow-up evidence](../docs/level-traceability.md#follow-up-safety-verification-2026-10-01)
and [build provenance](../docs/level-build-provenance.md).

### Recorded local verification (2026-09-30)

- `cargo fmt --all -- --check` and `cargo test --workspace`.
- `node scripts/check_error_codes.js` and
  `python scripts/check_workspace_dependency_graph.py` (using the bundled Python
  executable when the Windows `python` alias is unavailable).
- `./scripts/test-level-wasm.ps1`: adapter tests, release wasm32 builds,
  wasm-bindgen generation, actual portable Node, Chrome module worker, Wasmtime
  49.0.1, and full-result comparison for all 36 fixtures.
- `cargo fmt --manifest-path tools/wasmtime-host/Cargo.toml` and the actual
  `level-parity` binary against the built Level ABI v1 artifact.
- `git diff --check`.

The repeat script writes native CLI, browser, portable Node, and Wasmtime reports
under `target/level-*-report.json`; browser/portable reports record profile,
runtime and artifact digest. Native CLI integration tests establish the matching
direct Rust baseline. See [traceability](../docs/level-traceability.md) for rule
coverage and exact integration limitations. These are local test-host results,
not production suitability or whole-task Steam/backend parity certification.
