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
