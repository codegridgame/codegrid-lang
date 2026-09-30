# Level Work Traceability and Verification

Status: implementation evidence map for the Rust level-evaluation work. This
document records implementation evidence. It distinguishes implemented local
behavior, design-only Environment work, and host integrations that have not been
tested in their production projects.

On 2026-10-01 the user limited delivery to CLI and deferred Steam, backend, and
Web integration. Native CLI delivery is complete; the external integration
limitations below remain accurate but are future work rather than current gates.

The normative authorities are the [Level Core v1 specification](../spec/codegrid-level-core-spec-v1.md),
the [ExactIO implementation contract](../spec/codegrid-level-exactio-contract-v1.md),
the [delivery task book](../tasks/level-core-exactio-v1.md), the
[Level Host API v1 contract](level-api-v1.md), the
[WASM transport contract](level-wasm-v1.md), and the
[stable error registry](../spec/codegrid-error-codes.md). Compiler acceptance
and VM behavior remain defined by the [source specification](../spec/codegrid-source-spec.md)
and [VM specification](../spec/codegrid-vm-spec.md). Tests provide evidence for
those contracts; they are not a second semantic authority.

## Work-package traceability

| Task package and rule | Implementation | Focused evidence | Boundary of the evidence |
| --- | --- | --- | --- |
| WP0: publish schemas, host contracts, safety accounting, errors, and executable vectors before relying on them | [Level API](level-api-v1.md), [WASM transports](level-wasm-v1.md), [safety accounting](level-safety-accounting.md), [CLI contract](cli.md), [error identities](../spec/codegrid-level-errors-v1.md), and [local profile](../fixtures/levels/profiles/local-v1.json) | [Conformance manifest](../fixtures/levels/conformance-v1.json); `cargo test -p codegrid-level-core`; `cargo test -p codegrid-level-api`; `node scripts/check_error_codes.js` (passed: 244 scoped identifiers, 265 emitted references) | `local-v1.json` is explicitly a repeatable local test profile, not a production recommendation. Host deployment limits still require host owners to select and enforce trusted profiles. |
| WP1: strict bounded JSON decoding, opaque validated levels, verified-program restrictions, and explicit unsupported-scene dispatch | [`schema.rs`](../crates/codegrid-level-core/src/schema.rs), [`validate.rs`](../crates/codegrid-level-core/src/validate.rs) | Schema tests `format_dispatch_precedes_payload`, `nested_duplicate_and_unknown_paths`, `types_domains_and_required_fields`, `reject_metric_capability_visibility_and_scene`, `trusted_decode_limits_and_utf8`, and `weak_targets_and_aliases_permitted`; validator tests `all_canonical_variants_and_independent_attachments`, `folded_unreachable_and_custom_bodies_are_checked`, and `memory_generated_and_dimensions_threads_counts` | The implemented production evaluation type is ExactIO. Environment dispatch rejects unsupported scene types; example payloads do not enable any production scene. |
| WP2: deterministic ExactIO lifecycle, isolation, priorities, generated-code checks, metrics, constraints, ratings, slicing, and safety outcomes | [`evaluate.rs`](../crates/codegrid-level-core/src/evaluate.rs), [`metrics.rs`](../crates/codegrid-level-core/src/metrics.rs), and [`result.rs`](../crates/codegrid-level-core/src/result.rs) | Evaluator tests `fresh_state_visible_only_metrics_and_slices`, `debug_continues_all_and_official_hidden_failure_is_redacted`, `constraint_breach_overrides_correct_output_and_debug_runs_rest`, `empty_output_requires_halt_and_safety_is_distinct`, `ratings_use_widened_thresholds`, `shuffle_vectors`, `insufficient_budget_rolls_back_and_larger_retry_advances`, `custom_generated_forbidden_primary_is_checked_after_commit`, `runtime_error_rolls_back_same_tick_output_and_keeps_stable_identity`, `exact_work_ceiling_allows_completed_test_and_cancellation_has_no_rating`, `concurrent_equal_output_conflicts_cannot_pass`, `function_folded_and_repeat_static_and_dynamic_metrics`, `every_constraint_mapping_and_static_debug_behavior`, and `memory_and_stack_measurements_follow_vm_raw_metrics`; `cargo test -p codegrid-level-core` passed (16 unit and 16 evaluator tests) | Tests establish local core behavior against the fixed ExactIO contract. They do not provide scene behavior or production-host resource policy. |
| WP3: specify Environment interfaces, ownership, transitions, metrics, privacy, and future scene-author checks without shipping production scenes | [Environment extension design](level-environment-extension.md) | Design sections cover typed registration, immutable validated data, scene/VM ownership, transition state machine, bounded events, privacy, metric example, and a scene-author checklist | Design only. `Elevator`, `MaintenanceRobot`, and every other production Environment scene remain unsupported. No Environment protocol is implemented or claimed complete. |
| WP4: expose one versioned host lifecycle, handles, bounded results, exact integers, and privacy-safe projection | [`session.rs`](../crates/codegrid-level-api/src/session.rs), [`profile.rs`](../crates/codegrid-level-api/src/profile.rs) | [`lifecycle.rs`](../crates/codegrid-level-api/tests/lifecycle.rs): `result_lifecycle_exact_seed_and_scoring`, `isolation_versions_unknown_duplicates_and_shutdown`, and `rejection_seed_source_response_and_resource_limits`; the API integration tests passed as part of `cargo test -p codegrid-level-api -p codegrid-cli` (2 API unit and 8 API integration tests) | The API is a local Rust implementation. These tests cover API behavior, not all adapter execution environments. Full manifest comparison against the direct API is also in the CLI integration suite. |
| WP5: add native `evaluate`, preserve API semantics, define exit/status presentation, and read bounded UTF-8 files | [`main.rs`](../crates/codegrid-cli/src/main.rs), [CLI module rules](../crates/codegrid-cli/AGENTS.md), and [CLI contract](cli.md) | [`cli.rs`](../crates/codegrid-cli/tests/cli.rs): `level_manifest_cli_matches_direct_rust_api_complete_results`, `evaluate_official_echo_u64_max_matches_the_direct_level_api`, `evaluate_reports_source_and_level_rejections_as_complete_api_responses`, `evaluate_maps_player_rejection_failure_constraints_and_hidden_redaction`, `evaluate_rejects_bad_arguments_profiles_and_files_without_json`, and `evaluate_returns_resource_status_and_rejects_oversized_inputs`; `cargo test -p codegrid-level-api -p codegrid-cli` passed (6 CLI unit and 17 CLI integration tests), including full response equality for all manifest cases | This verifies the native executable and filesystem boundary against the shared Rust API for the committed manifest. It does not certify a browser, backend, or Steam host. |
| WP6: provide browser binding and import-free portable ABI adapters with bounded conversion and lifecycle | [`codegrid-level-wasm-browser`](../crates/codegrid-level-wasm-browser/src/lib.rs), [`codegrid-level-wasm-server`](../crates/codegrid-level-wasm-server/src/lib.rs), [WASM smoke cases](../crates/codegrid-level-wasm-browser/tests/level-smoke.mjs) and [portable smoke cases](../crates/codegrid-level-wasm-server/tests/server-smoke.mjs) | `cargo test -p codegrid-level-wasm-browser -p codegrid-level-wasm-server` passed (4 native adapter tests). Actual transport runs are orchestrated by [`test-level-wasm.ps1`](../scripts/test-level-wasm.ps1) | Node WebAssembly and a local Chromium worker are test harnesses. The portable artifact is import-free; this alone does not establish integration into a selected backend or game runtime. The final actual-host harness was rerun after replay-result changes and matched all 36 cases. |
| WP7: compare completed results on actual native, browser, and portable execution surfaces | [`compare-level-hosts.mjs`](../scripts/compare-level-hosts.mjs), [browser worker runner](../crates/codegrid-level-wasm-browser/tests/run-browser-smoke.mjs), [portable Node runner](../crates/codegrid-level-wasm-server/tests/server-smoke.mjs), and optional [Wasmtime harness](../tools/wasmtime-host/src/bin/level-parity.rs) | The shared 36-case manifest is run by the adapter harnesses and native CLI; `scripts/test-level-wasm.ps1` builds/runs them and compares complete result projections. The CLI integration test also compares every manifest case directly to `LevelApi` | All final reports were regenerated after replay-provenance changes; full local semantic comparison passed for all 36 cases. Production Steam embedding and the real backend project are unavailable here. |

## Environment sections 18–33

These mappings connect the normative Environment sections in Level Core v1 to
the design and current implementation boundary. “Design only” means no
production scene behavior or scene acceptance test exists.

| Level Core section | Rule represented in the design | Implementation and test status |
| --- | --- | --- |
| §18 Environment Evaluation | Environment is a distinct evaluation type with persistent scene state and VM-driven decisions. | [`schema.rs`](../crates/codegrid-level-core/src/schema.rs) dispatches explicitly and rejects unsupported Environment data; `reject_metric_capability_visibility_and_scene` and CLI `evaluate_reports_source_and_level_rejections_as_complete_api_responses` cover rejection. Design only beyond dispatch. |
| §19 Scene Types | A scene type is an explicit, registered capability; unknown scenes never fall back to ExactIO. | [`level-environment-extension.md`](level-environment-extension.md) sketches a static typed registry and sorted capability reporting. No scene is registered; unsupported behavior is tested. |
| §20 Environment Data Model | Scene definitions, goals, and persistent runtime state are distinct typed data; validated definitions are immutable. | The design sketches opaque `ValidatedScene`/`ValidatedGoals` and per-scene associated types. Production scene schema/state validation is not implemented. |
| §21 Environment Goals | Configured goals compose with AND semantics and are evaluated against scene state. | The design assigns goal predicates to the scene and orchestration boundary. No scene goal implementation or test exists. |
| §22 Environment Execution Model | A run initializes once, then repeats observe, decide, apply, and persist transitions. | The design state machine defines these transitions and atomic installation of next scene state. Environment execution is not implemented. |
| §23 Environment VM Reset | Every decision begins with the standard VM state; only scene state crosses decisions. | The design’s `DecisionSession` owns the current VM and drops it after one complete action. ExactIO fresh-VM coverage exists in `fresh_state_visible_only_metrics_and_slices`; no Environment reset test exists. |
| §24 Observation Protocol | Scene observations are bounded bytes delivered through standard VM input. | The design assigns observation encoding to the scene and bounded output to `BoundedBytes`. No production observation format is specified or implemented. |
| §25 Action Protocol | Actions use normal VM output and a complete scene-specific frame decoder. | The design gives each scene a fixed action-frame length and typed decoder. No production action schema or decoder exists. |
| §26 Dynamic Step Completion | A decision ends only when a complete action or terminal VM/evaluator outcome is established. | The design routes completed action frames to a committed scene transition and keeps a suspended VM only inside its current decision. No Environment runtime test exists. |
| §27 Incomplete and Invalid Actions | Incomplete or malformed actions are player failures, distinct from VM runtime errors. | The design maps typed decode failures to `TestFailed(InvalidAction)`. No production action decoder currently emits this outcome. |
| §28 Environment Completion and Failure | Initial goals, scene failures, goal completion, constraints, cancellation, and resource outcomes have explicit priority. | The design enumerates the needed checks and shared terminal statuses. Environment priority remains a later normative scene decision; the ExactIO priority contract is implemented separately. |
| §29 Metrics | Each scene adds a fixed typed vocabulary with direction, aggregate, and applicable constraints. | ExactIO metrics are implemented in [`metrics.rs`](../crates/codegrid-level-core/src/metrics.rs); Environment scene metric registration is design only. |
| §30 Metric Aggregation | Dynamic VM and persistent scene metrics aggregate by fixed rules across decisions. | ExactIO aggregation has focused coverage in `function_folded_and_repeat_static_and_dynamic_metrics` and `every_constraint_mapping_and_static_debug_behavior`. Environment aggregation is not implemented. |
| §31 Metric Scope | ExactIO official aggregates use visible tests; Environment aggregates all decision steps. | ExactIO visible-only behavior is covered by `fresh_state_visible_only_metrics_and_slices` and `debug_continues_all_and_official_hidden_failure_is_redacted`. Environment scope is design only. |
| §32 Constraints | Level constraints apply to configured aggregate values and remain separate from hard resource ceilings. | ExactIO constraint evaluation and status priority are implemented and tested; scene-specific constraints are not implemented. |
| §33 Constraint Scope | ExactIO constraints use visible-test aggregates; Environment constraints use persistent state/final level aggregates. | Visible-only ExactIO scope is tested; Environment scope and irreversible-violation handling remain design decisions until a production scene contract is approved. |

## Reproduction commands

Run the Rust suites independently when narrowing a failure:

```powershell
cargo test -p codegrid-level-core
cargo test -p codegrid-level-api
cargo test -p codegrid-cli
cargo test -p codegrid-level-wasm-browser -p codegrid-level-wasm-server
node scripts/check_error_codes.js
```

The CLI fixture example is also runnable directly:

```powershell
cargo run -p codegrid-cli -- evaluate fixtures/levels/echo.json fixtures/levels/echo.cg --mode official --boundary exit --seed 18446744073709551615 --custom-limit 1000 --limits-file fixtures/levels/profiles/local-v1.json
```

For actual local adapter execution, run
`pwsh -File scripts/test-level-wasm.ps1`. It builds both `wasm32-unknown-unknown`
artifacts, generates browser bindings, runs the portable module under Node,
launches an installed Chromium browser worker, and compares the complete
manifest results. The optional pinned Wasmtime host can be run with
`cargo run --manifest-path tools/wasmtime-host/Cargo.toml --bin level-parity -- . target/wasm32-unknown-unknown/release/codegrid_level_wasm_server.wasm`.
These commands require the documented WASM target/toolchain, `wasm-bindgen`,
Node, and— for the browser run—an installed Chromium browser. Reports are local
build artifacts, not checked-in production-host attestations.

Observed final local comparison on 2026-09-30: after replay source/level/profile
hashes were added to the shared result, `node scripts/compare-level-hosts.mjs`
passed all 36 complete terminal results against the native CLI, Chromium worker,
portable Node WASM, and the Wasmtime 49.0.1 harness. Reports recorded Node
`v24.21.0`, Chrome `153.0.8010.54`, and a configured 64 MiB linear-memory
ceiling. The portable artifact SHA-256 was
`4a5be919a2791ee06127cecc21d8c1092e8edd8b9000173ab96e0afb538a44cb`; the
generated browser worker artifact SHA-256 was
`3e40f4865f8eb5321a91f6ce8c7e80f80ba474ee32f8c4e52912e89466413a72`. Fresh
reports with runtime versions, artifact hashes, and full permitted results are
written under ignored `target/`. This verifies the selected local harnesses;
it does not establish production Steam or backend integration.

## Follow-up safety verification (2026-10-01)

The core passes 16 unit and 16 evaluator tests; the API passes 2 unit and
8 lifecycle tests. Feedback and terminal-result retention are reserved within
the host state budget. Bounded projection checks cover compiler diagnostics,
logical-level errors, and complete result serialization. Generated diagnostics
remain private while permitted feedback stays redacted.

`cargo test --locked --workspace` and `./scripts/test-level-wasm.ps1` passed after
these changes. Actual browser and portable Node transports passed shared
negative request, release, resource, slicing, and hidden-feedback probes.
Portable buffer reservation and Wasmtime fuel rejection were exercised in their
actual runtimes. All 36 complete semantic results match across local hosts.

The repeat script verifies the pinned toolchain and records source identity,
lockfiles, artifact digests, and runtime metadata in
`target/level-build-provenance.json`; see [build provenance](level-build-provenance.md).
Production Steam/backend integration remains open.

## Remaining acceptance limits

- Environment is a designed extension only. No production scene schema,
  protocol, or evaluator is supported; unknown scenes are rejected.
- The 36-case fixture manifest and its native, browser-worker, portable Node,
  and optional Wasmtime runs exercise repository-local harnesses. Pending
  sequences may differ by host slice budget; only terminal semantic results
  are compared.
- No Steam desktop embedding project or real production backend project is
  available in this repository. The local Wasmtime harness, if run, verifies
  only that explicit runtime configuration and artifact; it does not certify
  Steam SDK integration, server deployment, authentication, persistence,
  leaderboard authority, or production resource policy.
- Hidden tests sent to clients are not secret. Official certification requires
  the backend to load its trusted full level, recompile submitted source, and
  evaluate it with its own immutable safety profile. Client results do not
  establish server-authoritative records.
- Local profiles and process/linear-memory limits do not measure physical Rust
  heap overhead or replace embedding-host memory, fuel, stack, or deadline
  controls. Resource accounting is deterministic logical accounting as defined
  in the [safety contract](level-safety-accounting.md).
- Build success alone is not host parity. The task remains open until the actual
  selected Steam and backend runtimes run the shared fixture comparison and
  their runtime versions, artifact digests, result reports, and limitations are
  recorded.
