# Level Host Integration Readiness

Recorded: 2026-10-01. This is an integration inventory, not host acceptance.

The user explicitly deferred Steam, backend, and Web integration on 2026-10-01.
The current delivery is CLI only. Target selection and the steps below are
future work, not blockers for that delivery.

## Candidate host found locally

Read-only inspection found a CodeGrid workspace at `C:/source/bf-steam-wt`.
Its root package declares Steam Electron 31.7.7 packaging and an API workspace.
The workspace contains substantial uncommitted changes. Its selection as the
delivery target is pending confirmation; no files there were changed by this audit.

The Steam app has isolated, sandboxed renderers and a narrow preload bridge.
Its browser-compatible renderer is a possible Level browser binding host.
Acceptance must execute the evaluator in the actual Electron runtime, record
Electron/Chromium versions and artifact identity, and compare the full manifest.
An ordinary Chrome worker run does not establish that acceptance.

The API includes both a Node service and a separate Cloudflare Worker adapter.
The current `apps/api/src/codegrid/server-runtime.ts` explicitly requires
language Server ABI 3 and Runtime API 2. It is not a Level ABI 1 integration and
must not be pointed at the new artifact without a separate Level adapter.
The local Worker smoke entry is isolated from production authentication.
The selected backend surface (Node, Worker, or both) still needs confirmation.

## Required integration work after target selection

1. Preserve the existing language runner and its independent version contract.
   Add a thin Level ABI 1 adapter using the published byte ownership rules.
2. Compose trusted logical levels and immutable profiles on the backend;
   compile submitted source in Rust. Do not trust client evaluation results.
3. Keep local smoke authentication isolated. Production execution must retain
   its existing fail-closed authorization and request admission controls.
4. Vendor or build the exact evaluator artifacts with source identity, artifact
   digests, profile identity, and compatibility metadata.
5. Run all shared conformance cases in actual Electron and the selected backend
   runtime, compare complete permitted results, and exercise resource/lifecycle
   failures through those adapters.
6. Update the host's module rules and run its type checks and focused tests.
   Record runtime versions and remaining deployed-host limitations.

These steps are host conversion and composition work. They must not add another
parser, validator, interpreter, scoring implementation, or scene implementation.
Steam SDK behavior and production deployment remain outside this task.
