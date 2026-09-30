# Module Rules: `codegrid-wasm-browser`

## Responsibility

Expose the shared runtime contract to browser JavaScript through a thin browser-specific WebAssembly binding.

## Allowed dependency and call direction

- Among workspace crates, this crate may depend only on `codegrid-runtime-api`; use browser-binding/serialization dependencies required for the ABI separately.
- The browser game host calls this adapter; it must create explicit runtime instances, supply resource limits, and release handles.
- Preserve exact wide integers and addresses through decimal strings, `BigInt`, or a documented lossless encoding; validate all bytes and handles.

## Prohibited

- Do not strip shared diagnostic codes or rename published error identifiers; preserve the aliases documented in the error registry.

- Do not implement source parsing, validation, instruction semantics, compilation, or a second VM here.
- Do not import DOM/UI/game policy into semantic crates or expose Rust pointers as durable identifiers.
- Do not trust frontend results as server authority or claim browser/server parity from compilation or native unit tests alone.
- Do not add server-runtime imports or make the browser binding a dependency of the semantic core.
- Check JavaScript string UTF-8 byte lengths and `Uint8Array` byte lengths before copying/converting payloads into Rust/WASM memory; also validate payload types.
- Bound serialized response bytes and return a valid limit error rather than partial JSON.
- Pass through the host-neutral deterministic work-unit ceiling and preserve its distinction from tick-slice exhaustion; do not present it as browser-engine fuel or a physical memory limit.
- Bound the canonical snapshot JSON size of retained instances after creation and public execution/snapshot operations; release an instance that exceeds the configured quota.
- Do not treat request/response/snapshot JSON quotas as actual allocator usage, temporary per-call allocation, WASM linear-memory, or fuel limits; those require separate budgets and actual-host tests.
- Require `CODEGRID_WASM_MAX_MEMORY_BYTES` for every build targeting `wasm32`; keep the value deployment-configurable, validate it as a 64-KiB page multiple within the wasm32 address-space ceiling, and encode it as the module's maximum linear memory.
- Describe the linked memory maximum only as a WASM linear-memory ceiling; it does not cap JavaScript heap usage, browser-process memory, wall-clock time, or concurrency.

## Validation

Run native adapter tests, `wasm32-unknown-unknown` compilation, and actual-browser fixtures for lifecycle, limits, exact serialization, deterministic results, and instance isolation. Verify the generated release module encodes the configured linear-memory maximum. Report unavailable host verification explicitly.
