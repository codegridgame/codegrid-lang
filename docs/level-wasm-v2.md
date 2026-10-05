# Scene WASM Transports v2

The [Scene Host Contract v2](../spec/codegrid-scene-host-contract-v2.md) defines
semantics and wire fields. Both adapters invoke shared `LevelApiV2`; they do
not implement scene validation, action processing, scoring, or feedback policy.
API/profile 1 remain available on their existing paths.

## Browser binding 2

The generated browser module exports `SceneLevelSession` alongside legacy
`LevelSession`. Initialize the scene class with original trusted profile-2 JSON,
then pass original API-2 request JSON strings to `request`. `shutdown` permanently
closes that session. JS type and UTF-8 byte checks share the existing bounded
string conversion. Canonical wide integers remain strings throughout.

```javascript
const session = new SceneLevelSession(profileJson);
const capabilities = JSON.parse(session.request(JSON.stringify({
  api_version: 2, operation: "capabilities"
})));
```

Each constructor selects its API/profile version explicitly. A profile-2 string
is not accepted by the legacy constructor. Worker scheduling and rendering are
host responsibilities; no animation timing or hidden VM state is exposed.

## Portable ABI 2

The no-import artifact retains `memory`, `level_alloc`, `level_dealloc`, and
`level_request` with the [v1 exact buffer rules](level-wasm-v1.md).
`level_abi_version()` remains 1; `level_abi_version_v2()` returns 2. Initialization
selects ABI/API/profile 1 or 2 explicitly and permanently for that module session.
A later request cannot switch the initialized version.

```json
{"abi_version":2,"api_version":2,"operation":"initialize","profile_json":"<original profile-2 JSON>"}
{"abi_version":2,"api_version":2,"operation":"request","request_json":"<original API-2 JSON>"}
{"abi_version":2,"api_version":2,"operation":"shutdown"}
```

Request results use the same exact packed pointer/length u64 and single-release
ownership convention. Responses add `abi_version: 2`. Invalid UTF-8 is rejected
before shared request dispatch. Response storage is reserved before dispatch.
A v2 profile whose `max_response_bytes` exceeds 8 MiB minus 16 bytes is rejected
before initialization; the reserved buffer must fit the API response and added
transport version field. This prevents acknowledging feedback that the transport
cannot return in a complete response. Physical memory/fuel/stack ceilings remain
embedding-runtime responsibilities.

## Reproducible verification

Run `scripts/test-scene-wasm.ps1` with the explicit linear-memory maximum.
It builds both versions, generates browser bindings, runs the v1 regression
pipeline, executes the [scene manifest](../fixtures/scene-v2/conformance-v2.json),
and compares complete results plus permitted Debug traces across actual hosts.
The default maximum is 67108864 bytes and is checked in the encoded WASM memory.

Initial evidence covers 14 runs: all six scenes in Debug and Official modes,
plus small-work-slice Robot/MechanicalArm variants. Complete results match native
CLI, an actual Chromium module Worker, Node WebAssembly, and Wasmtime 49.0.1.
Debug event traces match all three WASM hosts, including the slice variants.
The portable artifact imports nothing; Wasmtime also checks fuel exhaustion.
This fixture set does not prove every protocol vector, complete resource/work
accounting, production integration, JS/process memory ceilings, or deployment.
