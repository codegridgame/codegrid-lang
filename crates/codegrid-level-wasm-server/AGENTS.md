# Portable Level Adapter Rules

Depend only on codegrid-level-api among workspace crates. Own the versioned
no-import byte ABI, exact buffer ownership, and session lifecycle. Never duplicate
language or level policy. Do not import WASI, browser bindings, filesystem, clocks,
network, or a particular runtime. No unsafe blocks; narrowly scoped export
attributes may allow unsafe_code. Check exact live pointer/length pairs before
access; bound individual and aggregate bytes and count. Reserve response storage
before dispatching a request. Preserve complete JSON and original profile/request
text. Shutdown is permanent for a module session. Require explicit configured
linear-memory maximum for wasm32 builds; physical fuel/stack limits belong to
embedding runtimes. Actual runtime tests are required for parity claims.

Select ABI/API/profile 1 or 2 explicitly at initialization; that choice remains
immutable. Preserve `level_abi_version()` as 1 and expose v2 support through
`level_abi_version_v2()`. Both versions share the exact buffer allocator and
request entry point. Reject v2 profiles whose response ceiling plus transport
version field exceeds reserved buffer capacity before semantic initialization.
