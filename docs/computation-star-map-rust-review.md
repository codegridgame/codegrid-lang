# Computation Star Map v2: Rust contract review

Date: 2026-10-11. Status: historical preimplementation review. The accepted
Rust changes and verification are recorded in
[Gas, Size and Custom implementation](gas-size-implementation.md).

## Scope and evidence

This review evaluates the supplied “Computation Star Map + Multi-solution
Trajectory v2” draft against the current Rust source tree. It covers static
program measurement, VM execution accounting, level evaluation, and Rust-owned
API/WASM contracts. UI, solution storage, community services, leaderboard
implementation, and the separate TypeScript game engine are outside scope.

The inspected upstream base commit is
`4f9068d20285594a36225744942d1ccf8524047e`. Findings refer to working-tree
contents, including existing uncommitted changes, rather than that commit alone.
The supplied draft is a proposal, not a replacement for the normative source,
VM, level, or error contracts. References below are repository-relative.

## User clarifications after review

The following records the user's requested direction; it does not implement or
amend the normative contracts. The task remains documentation/review only.

- Target one coherent complete change, rather than an MVP implementation.
- Folded Block body cells participate in Size, and each outer reference cell
  costs an additional 1. Apply the ordinary nonempty-cell rule inside the body;
  empty cells remain 0 under draft Section 2.4.
- Temporarily do not expose Custom execution in the VM, while retaining its
  implementation code. A future implementation must enforce this at the shared
  execution boundary, including verified-IR inputs; a level permission alone
  would not disable other VM entry points. This replaces the review's suggestion
  to exclude Custom only from Gas-ranked evaluation. Exact rejection identity
  and entry-point behavior must be recorded before changing the VM contract.
- Complete the missing Gas price mappings using the reviewer's recommendations,
  assess representative programs, and freeze the resulting schedule as part of
  the complete change. Candidate rates are not yet a validated final schedule.
- Current Rust level `ProgramRules` and verified-IR cell data do not define
  level-supplied locked instruction cells or player-versus-system ownership.
  Consequently, the draft's fixed-code Size exemption has no current Rust
  representation to apply to. Do not introduce fixed-code ownership merely to
  support that hypothetical exemption. Explicit Entry remains a real source
  cell; implicit start inserts no cell. The proposed current Size rule counts
  explicit Entry as 1 and adds no Size for implicit start.

Final user confirmation: explicit Entry counts as 1 Size. An implicit start
adds no extra cell; its actual instruction cell follows ordinary Size rules.
Custom is currently to be unavailable for use, with an explicit structured
error and readable diagnostic, while its implementation remains in the source
tree. No further product-choice confirmation is pending for this review;
implementation details follow the recommendations above and require the normal
recorded contract decisions before development. Development is not started by
this confirmation.

Evidence for the fixed-code finding: `crates/codegrid-level-core/src/schema.rs`
(`ProgramRules`, `ValidatedLevel`), `schema/scene_v2.rs`, and
`crates/codegrid-ir/src/lib.rs` (`Cell`). Fixed scene-world data is not fixed
program instruction code.

## Assessment

The three-metric design is feasible within the existing Rust architecture, but
it is not currently implemented. Tick mostly matches the existing contract.
Gas requires new accounting, limits, and wire data; renaming Cost is insufficient.
Size needs an explicit counting decision before reusing existing static metrics.
The largest blockers are incomplete operation pricing, undefined Custom scope,
ambiguous Entry ownership, and unspecified Gas interruption semantics.

| Draft requirement | Current Rust evidence | Assessment |
| --- | --- | --- |
| Committed global Tick | `spec/codegrid-vm-spec.md` Sections 2, 14; `crates/codegrid-vm/src/metrics.rs` | Matches: failed attempts do not advance Tick |
| Visible-case Tick sum | `crates/codegrid-level-core/src/metrics.rs`, `scene_session.rs` | Already supported |
| Hidden correctness without ranked metrics | `scene_session.rs` uses test visibility for `include_metrics` | Already supported; keep independent hard limits |
| Stop Official evaluation at first failure | `scene_evaluate.rs` checks Official mode after a failed case | Already supported |
| No successful final metrics on failure | `scene_evaluate.rs` sets `final_metrics` only when passed | Already supported |
| Gas with operation-specific prices | `metrics.rs` maps `cost` to `operation_count` | Missing; current Cost is a different quantity |
| Per-case cold memory and stack charges | VM records distinct memory locations and aggregate stack peaks | Useful primitives exist, but these are not Gas accounting |
| Static Size excluding fixed Entry | `static_metrics` counts explicit Entry cells | Different contract; no direct alias |
| Gas schedule identity and hard ceiling | Level schema/profile and VM metric summary inspected | Proposed fields are not part of current contracts |
| Tick/Gas/Size result | Level API projects metric maps as decimal strings | Requires metric/schema/projection updates |

## Findings requiring resolution

### 1. Gas is a semantic addition, not a Cost rename

Draft Sections 2.3 and 3 correctly reject treating old Cost values as Gas.
Current `dynamic_metrics` returns both `cost` and `operation_count` from the
same VM counter. That counter excludes direction instructions and includes
Halt; the candidate Gas table charges direction instructions and makes Halt
free. For example, a direction execution can add 0 Cost but 1 candidate Gas,
while Halt can add 1 Cost but 0 candidate Gas.

An operation-count total cannot reconstruct weighted execution Gas because it
does not retain execution multiplicities by price category. Instruction Variety
is a set, not a frequency table. Do not calculate Gas by multiplying Cost by a
constant or by pricing the variety set.

Evidence: `crates/codegrid-level-core/src/metrics.rs`; VM specification Section
14; `crates/codegrid-vm/src/metrics.rs`.

### 2. Size and Entry definitions do not match current representation

The draft excludes a system-generated, immutable Entry. Current Full supports
explicit Entry and an implicit start when no Entry exists. Under the recorded
2026-10-11 decision, implicit start executes the real cell at `(0,0)` and does
not insert an Entry cell. A nonempty first cell therefore remains ordinary
countable program code; implicit start is not a free replacement for it.

Current `static_metrics` increments `non_empty_cells` for either `entry` or
`primary`, including explicit Entry. Verified IR does not by itself establish
whether an explicit Entry was player-authored or supplied as an immutable level
cell. Resolve whether all explicit Entries count, only player-authored Entries
count, or Entry is excluded as a language category. If ownership matters, define
a trusted representation and validation boundary rather than guessing from the
instruction token.

Also specify the treatment of fixed non-Entry instructions. The draft's formula
refers to player-authored cells, but its exclusions identify only fixed Entry.

Evidence: `spec/codegrid-vm-spec.md` Sections 2 and 5;
`docs/decisions.md`, “Toroidal boards and implicit entries (2026-10-11)”;
`crates/codegrid-level-core/src/metrics.rs::measure_scope`.

### 3. Folded Blocks and Custom definitions need an explicit Size rule

Current static traversal counts outer and Custom program scopes, all declared
Functions regardless of reachability, Folded Block shell cells, and nonempty
Folded Block body cells. Function reuse does not multiply the static count.
Those properties largely support the draft's static intent, but “functions,
macros and independent boards” does not fully specify these actual Full forms.

Decide whether a Folded Block shell and its body both count, whether unused
Custom definitions count, and how built-in versus player-authored Custom code
is classified. Size should come from the accepted original program, not visited
cells or runtime-mutated code. `Used Cells` is a dynamic visitation metric and
cannot substitute for Size. Keep `non_empty_cells` distinct unless a recorded
decision intentionally changes its current meaning.

Evidence: `metrics.rs::static_metrics`, `measure_scope`; VM specification
Sections 6 and 14.

### 4. The candidate Gas table does not cover Full instruction semantics

The authoritative Primary inventory includes `RandomDirection`, `Compare`,
`Clear`, `Add`, `Sub`, `OutputImmediate`, `PopAdd`, `Decode`, `Encode`,
`MovePage`, `Shift`, `FoldedBlock`, `Custom`, and `CustomReturn`. Several have
no explicit mapping in the draft. “INC / DEC” and “immediate write” are pricing
descriptions rather than exact Full Primary names.

Create an exhaustive mapping for every Primary, conditional prefix, and
Attachment before implementation. Clarify whether `PopAdd` has one complete
base price or separate pop/add prices; similarly distinguish OutputImmediate
from register writes. Define Repeat, instruction-stack playback, Folded Block
entry/exit, empty input reads, empty pops, and empty memory stores. Specify
whether an ineffective operation still pays its base fee. Never silently price
unlisted operations at zero.

Gas events must follow actual VM transitions, including repeated execution and
self-modifying code, rather than a second evaluator in the API layer.

Evidence: `crates/codegrid-model/src/lib.rs` Primary/Attachment enums;
`crates/codegrid-vm/src/metrics.rs::InstructionKind`; VM specification
execution and Attachment sections.

### 5. Custom cannot remain an unspecified path in a ranked Gas MVP

Custom performs synchronous internal ticks inside an outer Tick. Existing raw
operation and resource metrics include internal work. Excluding Custom from
Gas would permit substantial computation with no execution charge and would
make the resource metric dependent on code packaging.

Before Gas-ranked evaluation accepts Custom, either define complete internal
charging and shared hard-budget behavior, or explicitly restrict eligible
programs through the existing Rust validation/permission boundary. A draft
statement that final Custom pricing is deferred does not settle this MVP issue.
Define whether the invocation shell has a price, how CustomReturn is charged,
and whether outer and internal work consume one common Gas budget.

### 6. Cold memory identity must include the logical memory space

Current memory metrics use an address plus memory-space identity. Outer memory
is shared. Each Custom invocation has fresh memory, identified by attempted
outer Tick, caller thread, and Custom ID. Address 10 in two fresh Custom
invocations is therefore not one shared warm location.

The draft should specify a cold key equivalent to `(memory_space, address)`;
threads sharing a space share warmth, while fresh spaces do not. Define absent
zero reads, empty stores, code access, and any initially supplied memory.
Current raw metrics count MEMORY_LOAD and nonempty MEMORY_STORE, including
absent zero reads; empty stores do not add a used address. Gas may adopt another
rule, but that must be intentional and separately documented.

Evidence: VM specification Section 14; `metrics.rs::MemoryLocationId` and
`MemorySpaceId`; existing Custom-memory metric tests.

### 7. Stack charging needs a normative sampling rule

The VM already records aggregate peaks across resident outer and Custom
threads, counting the caller data stack once. However, it samples specific
logical instants: outer/Custom boundaries, attempted staged growth before
commit without subtracting staged pops, and committed post-state afterward.
Failed attempted growth can survive rollback in raw metrics.

“The same logical instant” in the draft must either adopt those exact rules or
record a different rule. A push and pop in one transition can produce different
charges depending on sampling. Clarify empty initial stacks, Folded Block
frames, Custom residency, and verifier-derived self-tail-call frame reuse.
If reuse avoids actual call-stack growth, frame Gas differs from recursive
depth counted abstractly; CALL base Gas should be specified independently.

Per-type peaks can price independent capacities, but their weighted sum is not
simultaneous memory occupancy. Preserve the draft's distinction.

### 8. Gas exhaustion and host interruption are separate contracts

The draft proposes `gas_hard_limit` without specifying the charge boundary.
Define whether exceeding a limit rejects an operation before execution,
aborts and rolls back the whole attempted outer Tick, or retains partial
attempted diagnostic charges. Define equality at the limit, arithmetic
overflow, pending output, internal Custom failure, and same-Tick error/Halt
precedence. Associate structured errors at detection through the error registry.

Current per-call work limits interrupt without changing normative metrics.
Gas charged twice on resume would violate deterministic step/run equivalence.
Repeated small-budget calls must produce the same result and Gas as one large
budget call. Specify whether the hard Gas budget resets per test or spans an
evaluation; the draft only explicitly resets cold-address and stack state.

Keep Tick, work, state, output, and scene limits independent. Free empty/Halt
operations and scene-side work make Gas alone an insufficient safety ceiling.

Evidence: VM specification Sections 14–15; level safety profiles;
`crates/codegrid-level-api/src/profile_v2.rs`.

### 9. Aggregate Gas must sum independently priced cases

Current aggregation sums only `ticks`, `cost`, and `operation_count`; all other
metrics use maximum. Adding `gas_used` without updating this function would
incorrectly return the maximum case Gas instead of the sum.

Calculate each visible case's Gas with independently reset cold locations and
capacity peaks, then sum case Gas. Do not apply `10U + D + 4I + 16C` to current
evaluation-wide maxima. Two cases each reaching data-stack peak 5 owe 10 total
stack Gas, even if the aggregate auxiliary peak is 5. Optional execution,
memory, and stack Gas breakdowns must each sum and exactly reconcile with total.
Static Size is measured once and must not multiply by the visible-case count.

### 10. Determinism requires seed and execution configuration

Draft Section 3.1 lists program, input, VM version, and schedule but omits the
explicit VM seed. Full has RandomDirection and deterministic PRNG state;
different seeds can visit different instructions and consume different Gas.
Include seed, initial memory, accepted program/Custom definitions, relevant
evaluation configuration, and test identity in replay/comparability rules.
Schedule identity alone is not proof that two evaluations used equal inputs.

### 11. Rust contracts need coordinated changes; preserve integer precision

Current level schema enumerates `cost`, `max_cost`, `non_empty_cells`, and other
metric names. Gas/Size cannot be supplied as arbitrary new authoring fields
without corresponding validation, scoring, constraint, and result changes.
The runtime VM summary also needs an intentional decision about exposure of
Gas, schedule identity, diagnostic attempted Gas, and optional breakdowns.

Retain checked integer arithmetic and decimal-string host conversion for
values beyond JavaScript's exact range. Do not expose Gas as an unrestricted
JavaScript Number. Keep a validated schedule/configuration as explicit data,
not host clocks, environment variables, or host-dependent memory allocation.

Evidence: `crates/codegrid-level-core/src/schema.rs`, `metrics.rs`;
`crates/codegrid-level-api/src/common.rs`, `projection_v2.rs`;
`spec/codegrid-runtime-api-spec.md`.

### 12. “No total score” does not automatically remove star ratings

The Rust level evaluator already computes an optional overall star rating as
the minimum rating among metrics with targets. This is not a weighted total
leaderboard score, so it need not contradict the draft. Explicitly preserve
that behavior or record a deliberate change. Replacing Cost also requires
reviewing author-provided Cost targets: equal numeric Gas targets do not imply
equal difficulty under the new prices.

Evidence: `metrics.rs::rating`; `spec/codegrid-level-core-spec-v1.md` scoring
and rating sections.

### 13. Prepublication compatibility policy supersedes mandatory migration

Draft Sections 3.8 and 16 require old-score archival and migration-like version
handling. No user-published release establishes a compatibility baseline.
Update current unpublished contracts coherently; do not require legacy Cost
readers or old binaries merely because internal version numbers exist.
Preserve unrelated user data. A schedule identifier remains useful for replay
and metric comparability, independently of backward compatibility.

Evidence: repository `AGENTS.md` release policy;
`docs/downstream-synchronization.md` precedence statement.

## Suggested Rust decision sequence

1. Record Size rules for explicit/implicit Entry, fixed cells, Folded Blocks,
   unused Functions, and Custom definitions.
2. Record the exhaustive Gas operation mapping, memory-space keys, stack
   sampling, tail-call behavior, and Custom policy.
3. Record attempted/committed Gas, hard-limit scope, overflow, errors, and
   pause/resume behavior. Select representative programs to assess candidate
   rates before freezing them.
4. Define level metric names, constraints, schedule identity, scoring targets,
   integer wire representation, and replay data. Update normative documents
   only through the required recorded decisions.
5. Only after authorization to develop, implement shared Rust accounting and
   evaluator integration, then synchronize actual host contracts and artifacts.

Suggested ownership: VM accounts deterministic execution/resource events and
enforces its execution budget; level-core selects validated evaluation policy,
computes static Size, and aggregates visible-case Gas; runtime/level APIs and
WASM adapters perform request validation and wire conversion. Model/compiler/VM
must not acquire UI, storage, leaderboard, or game-scoring responsibilities.

## Verification needed in a future implementation

| Area | Required focused evidence |
| --- | --- |
| Static Size | Default-entry real instruction; explicit/fixed Entry decisions; compound cell; Folded shell/body; unused Function/Custom; runtime code mutation does not change static Size |
| Operation pricing | Exhaustive inventory; failed condition; Repeat; code attachments; empty READ/POP/store; Halt; Custom return and internal work |
| Resources | Same address in shared space versus fresh Custom space; per-case reset; multiple resident threads; caller stack counted once; push/pop sampling; tail-call reuse |
| Limits | Exact budget and one unit above; checked overflow; failed outer Tick; Custom exhaustion; step/run equivalence under resumable work ceilings |
| Evaluation | Two visible cases sum Gas; hidden cases affect correctness but no score; failure has no final metrics; breakdown sum equals Gas; Size counted once |
| Hosts | Actual native, browser-WASM, and server-WASM result comparison with large integer values and identical replay configuration |

## Changes and verification limits for this review

Only this review document is added. No source, specification, decision, test,
schema, dependency, generated artifact, or executable behavior is changed.
Repository searches and direct source/specification reads support the findings;
no build, runtime tests, or cross-host parity checks were run. Existing tests
were inspected as source evidence, not claimed as passing acceptance.

The mandatory downstream synchronization instructions and both root rules were
read. The downstream synchronization record and current consumers were checked
to establish this document's impact. It proposes no current contract change,
so no downstream source/artifact synchronization or rebuild is needed. This
does not establish readiness of the proposed Gas/Size feature. Existing dirty
working-tree files in both repositories were preserved.
