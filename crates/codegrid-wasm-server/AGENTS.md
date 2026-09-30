# Module Rules: `codegrid-wasm-server`

## Responsibility

Expose the host-neutral Runtime API lifecycle to server WASM hosts through the
versioned no-import byte ABI, preserving Runtime API state for the lifetime of
one WebAssembly module instance.

## Allowed dependency and call direction

- Among workspace crates, depend only on `codegrid-runtime-api`.
- Convert byte ABI operations into explicit Runtime API requests and convert
  results back into bounded, versioned JSON responses.
- Serialize large check, compile, step, run, and snapshot responses from
  borrowed Runtime API/compiler views through `serde::Serializer`; use the
  bounded counter for retained-state checks and the bounded writer for final
  response bytes.
- Treat host limits as trusted deployment configuration supplied by the
  embedding server; source and input remain untrusted data.
- Keep handles and all mutable runtime state local to one module instance.
  Require one immutable host-limit configuration at initialization. Compile
  source through the shared compiler and never accept client-supplied IR or
  validation claims.
- Support check, compile, isolated instance creation, step, bounded run,
  snapshot, explicit program/instance release, and module-session shutdown.

## Prohibited

- Do not strip compiler diagnostic codes or silently rename published server error aliases or ABI zero-sentinel meanings.

- Do not implement source parsing, validation, instruction tables, IR lowering,
  VM transitions, or game policy.
- Do not depend on browser bindings, JavaScript, DOM, WASI, a specific server
  runtime, filesystem, network, clocks, or process APIs.
- Do not expose Rust pointers as durable identifiers or carry wide values
  through floating-point numbers.
- Do not use unsafe blocks. The only permitted `unsafe_code` lint exceptions
  are the narrowly scoped attributes required to export the documented C ABI
  symbols; keep each exception on its individual export function.
- Do not let request-provided configuration override trusted deployment
  ceilings; do not return partial JSON when a response exceeds its byte bound.
- Do not materialize a full `serde_json::Value` tree for large response or
  retained-state projections; keep lifecycle and cleanup decisions in the
  session adapter and field serialization in the projection module.
- Do not share one module instance across untrusted tenants. Shutdown closes the
  session permanently; create a fresh WASM instance for a new isolation scope.
- Bound both individual and aggregate retained ABI buffers, and verify that
  the host can release them without exposing durable pointer identities.
- Do not claim production-host parity or production resource safety from a
  `wasm32` build or Node smoke test alone.

## Validation

Check the native library, compile the `wasm32-unknown-unknown` artifact, and run
the exported ABI against shared Full conformance fixtures in an actual WebAssembly
host. Verify the complete Full Runtime API lifecycle, isolation between VM
instances and WebAssembly module sessions, import/export shape, buffer
lifecycle, versions, exact numeric encoding, diagnostic/runtime error
distinctions, and all configured limits.
