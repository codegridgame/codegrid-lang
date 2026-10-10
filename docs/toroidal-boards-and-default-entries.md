# Toroidal boards and default entries

The approved [decision](decisions.md#toroidal-boards-and-implicit-entries-2026-10-11)
removes normal-board boundary selection and makes explicit Entry markers optional.

Every ordinary Main and Function board wraps horizontally and vertically,
including boards inside Customs. Crossing an edge preserves direction and the
other coordinate, and uses the ordinary movement transition without an extra
tick or operation. Folded Blocks retain horizontal wrapping and vertical exit
through Fold Resume.

A board without explicit Entries starts at (0, 0), facing Right. Main and Custom
Main create one default thread; Functions use the default call target. Explicit
Entries suppress the fallback. Functions allow at most one explicit Entry,
while Main retains multiple explicit threads. The default entry does not insert
a cell or instruction, and the first dispatch executes the real cell contents.
An empty starting cell consumes one tick and moves Right. CALL still transfers
on its own tick and executes the target cell on the following tick.

## Current contracts

Ordinary boards have one fixed toroidal topology and no host selection field.
Runtime, Level and WASM requests/results no longer expose boundary selection.
CLI run/evaluate and debug launch no longer accept boundary options; VS Code
settings and launch schemas are synchronized. Invalid board geometry still
fails verification, and invalid internal movement state is a VM fault.
Tail-call analysis uses toroidal reachability, including the default entry, and
retains loop detection. Level thread limits count the implicit default thread.

The unpublished-release policy permits updating current contracts directly
without aliases, migrations or a compatibility-only version bump.

## Verification (2026-10-11)

- All 549 Rust workspace tests, formatting, error registry/generated catalogs
  and dependency-graph checks passed.
- All 104 shared Full fixtures matched complete native results in browser WASM,
  Node browser/server WASM and Wasmtime. New fixtures cover empty/instruction
  default Main entries, Function, Custom and Custom Function defaults, all four
  edge crossings and a one-cell loop. Former boundary errors now test wrapping
  commits, genuine write conflicts or Custom limits.
- All 58 scene cases matched complete results across native CLI, browser Worker,
  Node WASM and Wasmtime; WASM Debug traces also matched.
- Actual VS Code native Run/Debug cases passed. The full native-enabled extension
  suite reported 132 passing, one pending real-LSP integration case, and one
  failing standalone completion case that received a residual fake-LSP
  `#lsp-only` suggestion. The same failure repeated without code changes. This
  unrelated test limitation remains recorded rather than being hidden.

The four WASM modules and generated bindings were rebuilt and synchronized to
Web, API, editor and Steam selection consumers in C:/source/bf-steam-wt.
The final evaluator identity is
`codegrid-level-source-sha256:bab6dc20abb37f1305d6f8d681dd5bbe2e619b7675090915cdb65fb4555ba59b`.
Provenance records the actual base commit, dirty source state and file hashes;
the editor retains its explicit-byte initialization patch with separate hashes.
The user subsequently extended the scope to the independent application GameRunner
and terminal paths; these consumers also use the fixed toroidal topology.

## Downstream acceptance

Reviewed and adopted the synchronized consumer changes and artifact manifests
in C:/source/bf-steam-wt. The downstream synchronization record includes actual
source/build identities and checks. Verification passed workspace type checks;
API-contract 21, Web 62, API 45, editor 67 and Steam selection 13 unit tests;
editor/Web/Steam builds; and a Worker bundle dry run. Actual-host checks passed
96 endpoint-compatible Full fixtures in Workerd, 10 Web browser tests, four
browser-to-Workerd tests, five editor playtest cases with three duplicate checks
skipped, and bundled Electron file:// offline scene evaluation and persistence.
Editor Main/Function and offline sources omit explicit Entry. Steam selection
tests cover default empty-cell start, all four edge directions, far-edge loops
and preserved Function target display. No commit, push or deployment occurred.

## Application-wide cleanup and final artifact verification

Every independent application game and terminal board also uses the fixed
toroidal topology. Mode fields, source directives, controls, projection fields,
edge-failure handling and its achievement are removed. The existing HALT
capability validates the active goal and completes explicitly. Game output
completion on an exact match remains available. Empty and instruction-bearing
starting cells execute normally; entering (0,0) preserves direction.

The final source identity above includes the updated debug configuration
messages. All four adapters were rebuilt and synchronized with truthful hashes.
The 549 Rust tests, 104 complete Full results across native/browser/Node/Wasmtime,
and 58 complete scene results and WASM Debug traces passed again. Final
Workerd verification covers 96 endpoint-compatible cases; the browser-to-Workerd
suite passed four tests. Editor artifact-integrity tests and packaged Electron
file:// offline evaluation and persistence passed against the final artifacts.

Searches found no alternative board-mode implementation or authoring contract.
Array/index validation, robot-world obstacles, bitmap cropping, process exits
and Fold Resume are separate rules and remain intact.

Final application checks passed workspace type checks, 945 unit tests across
workspace runs and focused reruns (including 17 terminal, 128 core, 125 content,
183 application and 290 Steam tests), editor/Web/Steam builds, and six focused
Steam browser cases. The final starting-cell edit regression passed all 23
playtest runtime tests. The guide now illustrates wrapping and has no
edge-failure scene; both placement paths allow editable cell (0,0).

The broader old Steam browser suite also exposed two unrelated UI expectations
that remain unresolved: click-to-rotate a held toolbox direction and the old
standalone READ direction panel. The changed startup, level completion and
keyboard execution cases were corrected and passed. The previously recorded
VS Code standalone completion test limitation also remains. No production
deployment, commit or push was performed.
