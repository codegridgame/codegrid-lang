# Native Scene Level API v2

`codegrid-level-api::LevelApiV2` exposes the native scene lifecycle described by
the [Scene Host Contract v2](../spec/codegrid-scene-host-contract-v2.md).
`SafetyProfileV2` validates explicit trusted ceilings; level/source content never
relaxes them. The current API implementation loads and executes ExactIO, Robot,
and MechanicalArm
under the selected `format_version: 1` author contract and shares the compiler
and evaluator with [browser/portable transports](level-wasm-v2.md). API 1
remains unchanged. Full direct protocol and production host coverage is tracked
in the [scene conformance plan](scene-conformance-plan.md).

The standard operations and fields follow [API v1](level-api-v1.md), with explicit
`api_version: 2`. The additional `scene_feedback` operation uses exactly
`evaluation_handle`, `after_sequence`, and `max_events` canonical decimal strings.
It is restricted to visible Debug events. Stale/undelivered cursors are rejected;
a complete response is prepared before delivered/acknowledged watermarks change.
Feedback reads do not advance the VM. Official results expose only permitted
visible cases and coarse hidden failure categories.

The CLI selects this API explicitly:

```powershell
codegrid evaluate fixtures/scene-v2/mechanical-arm.json fixtures/scene-v2/mechanical-arm.cg --api-version 2 --mode debug --seed 18446744073709551615 --custom-limit 1000 --limits-file examples/scene-host-v2/profile-local-v2.json
```

Omitting `--api-version` selects 1. The CLI only owns file access, flags, output,
and exit mapping. Use an API session to page Debug feedback; the CLI evaluate
command prints the terminal result. `--format human` also presents visible cases.

Handles belong to one API instance and one object kind, are never reused, and
must be released. Shutdown permanently closes the instance. Current native
scene-state reservations assign all remaining scene units to one retained
evaluation, preventing oversubscription; another evaluation may require release
of the first. This conservative resource policy does not promise concurrency.
Profile replay hashing uses compact canonical profile JSON with decimal-string
ceilings; original level/source bytes are hashed unchanged.

Native API tests and actual host comparisons are recorded in the
[conformance plan](scene-conformance-plan.md). The remaining protocol/resource
coverage gates apply independently from successful request dispatch.
