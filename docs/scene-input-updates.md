# Dynamic scene input updates

Decision: user request on 2026-10-13; normative contract in
[Scene Spec v1](../spec/codegrid-scene-spec-v1.md), with the explicit decision
recorded in [decisions](decisions.md#2026-10-13-dynamic-scene-input-replacement).

Robot snapshots are `[color, position]` or
`[A_color, A_position, B_color, B_position]`. Each valid nonterminal round
replaces all unread input, including partially consumed old snapshots.
Already read registers and continuous execution state persist. Single actors
update per command; dual actors retain the complete A/B frame boundary.

Both arm layouts publish current-hand snapshots every round. Initial input is
`[255]` or `[255,255]`; empty hands remain 255. A command updates observations
without changing the item byte. GRAB preserves the acquired state; only
worktable transformations change it. New input robots start UNINSPECTED:
a blue robot is 13 even when its hidden true state is irreparable; inspection
reveals state 9. Existing terminal and illegal-action rules remain.

The Rust VM exposes atomic input replacement at completed tick boundaries.
Queue ceilings measure replacement length; cumulative input ceilings count all
published bytes. Shared Debug feedback uses InputUpdated and RoundCompleted
contains the current-hand snapshot for both layouts. ExactIO is unchanged.

## Source and downstream synchronization

Source base: `b9a71a0d1d2c497f903e6f33c038f4281273d75c`, source_dirty=true.
Evaluator: `codegrid-level-source-sha256:37550f19fbc4f3e343b942ead45e8097ae23dcb5c348134b3f12b38973fd63ea`.
Preserved pre-existing changes in both working trees.

Updated downstream `apps/editor/src/scene-wasm/` and
`apps/api/src/codegrid/level-eval/` WASM/binding sets, truthful provenance,
separate local-byte binding patch hashes, the API evaluator identity, editor
feedback labels, current scene documentation, module rules, and focused server
fixtures. The server evaluator executes Rust WASM, with no TypeScript scene
semantics. Web Full/Steam selection language hosts and API Full have no changed
language contract and do not consume dynamic scene input updates; the embedded
Steam editor receives the updated scene set.

## Verification

- Rust VM, Level Core and Level API: 352 tests pass; formatter and diff checks pass.
- Browser/server scene adapter tests and release WASM build pass.
- 72 native/Node/browser scene cases pass, including unread/partial-input
  replacement and per-command single-arm snapshots.
- Downstream editor: 91 tests; API: 95 tests, including actual WASM replacement fixtures.
- Downstream full workspace type checks and architecture check pass; editor build passes.
- Offline sandboxed Electron loads the bundled WASM, evaluates a scene and persists data.
- All 72 complete results agree across native, Node, browser Worker, and Wasmtime;
  all three WASM Debug traces agree. Large fallback fixtures use work slices
  of 1000 to remain within the harness fuel per request.

No production deployment, physical hardware, or Steamworks acceptance is claimed.
