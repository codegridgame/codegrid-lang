# Toroidal boards and default entries

The approved [decision](decisions.md#toroidal-boards-and-implicit-entries-2026-10-11)
removes normal-board boundary selection and makes explicit Entry markers optional.

Every ordinary Main and Function board wraps horizontally and vertically,
including boards inside Customs. Crossing an edge preserves direction and the
other coordinate, and uses the ordinary movement transition without an extra
transition or operation. Folded Blocks do not wrap: movement that leaves the
block exits through Fold Resume, and direction Primaries are forbidden inside
them (see the Linear Folded Block sweep decision, 2026-10-20).

A board without explicit Entries starts at (0, 0), facing Right. Main and Custom
Main create one default thread; Functions use the default call target. Explicit
Entries suppress the fallback. Functions allow at most one explicit Entry,
while Main retains multiple explicit threads. The default entry does not insert
a cell or instruction, and the first dispatch executes the real cell contents.
An empty starting cell consumes one VM transition and moves Right. CALL still transfers
on its own transition and executes the target cell on the following transition.

## Current contracts

Ordinary boards have one fixed toroidal topology and no language-host selection
field. Runtime API, CLI, debug, VS Code launch/settings, and WASM contracts do
not expose boundary selection. Invalid board geometry still fails verification,
and invalid internal movement state is a VM fault.
Tail-call analysis uses toroidal reachability, including the default entry, and
retains loop detection.

The unpublished-release policy permits updating current contracts directly
without aliases, migrations or a compatibility-only version bump.

## Verification (2026-10-11)

- All 104 shared Full fixtures matched complete native results in browser WASM,
  Node browser/server WASM and Wasmtime. New fixtures cover empty/instruction
  default Main entries, Function, Custom and Custom Function defaults, all four
  edge crossings and a one-cell loop.
- Actual VS Code native Run/Debug cases passed. The full native-enabled extension
  suite reported 132 passing, one pending real-LSP integration case, and one
  failing standalone completion case that received a residual fake-LSP
  `#lsp-only` suggestion. The same failure repeated without code changes. This
  unrelated test limitation remains recorded rather than being hidden.

Game-level, application, selection, and WASM artifact verification for this
change is archived in the [Game Runtime history index](../../bf-steam-wt/doc/codegrid-lang-history/README.md).
