# Scene Document and Author Example Audit

Date: 2026-10-05.
Scope: documentation consistency, not runtime conformance.

Revision notice: the later Robot author-map decision supersedes the dense-map
rows and example paths below. The current contract uses terrain rows, sparse
colors, mutually exclusive objects, and equal-ID trigger/door pairs in author
format v1. This audit records earlier evidence; example and loader migration
must be re-audited before claiming conformance to the revised contract.

## Sources and provenance

Reviewed the selected conversation's accessible consolidated Scene Spec v1
and accepted follow-ups, the repository Scene Spec, Level JSON v2 contract,
continuous session design, conformance plan, and all 12 author examples.
The conversation tool returns a truncated 20,000-character final document:
Sections 1–12 and the substantive beginning of Section 13 are visible, ending
mid-sentence after the scene-specific metric list. It provides no older-page
cursor. This review cannot certify unseen tail text or every earlier discussion.
The source limitation does not invalidate the directly readable rules.

The six execution-boundary recommendations and subsequent JSON format choices
were accepted in this repository chat. They are recorded additions rather than
claims that every detail appeared in the original conversation.

## Rule comparison

| Area | Readable conversation rules | Repository outcome |
| --- | --- | --- |
| Shared | u8 I/O, queue-tail append, fresh cases, HALT alone is not a goal | Preserved; continuous-case lifetime explicitly confirmed later |
| Dual actors | Collect A/B, execute A first, stop immediately on terminal A | Preserved; later accepted actor-order validation and partial-effect rules |
| ExactIO | Full input at start; immediate byte comparison; empty expected needs silent termination | Preserved with existing VM/constraint priorities |
| Baudot | Codes 0–31, persistent LTRS/FIGS display mode, raw-code correctness | Preserved; no invented character table or shift numbers |
| Elevator | Floors 0–9, 1–2 elevators, 1–10 passengers, author order, current-floor no-op, round arrivals | Preserved; full floor/record fields selected in v2 |
| Robot | 16x16 map, encoded positions, five actions, adjacent jump, patrol goal, delayed doors, collisions/snapshots | Preserved; later accepted coordinate/turn conventions and wall/door exclusion |
| QualityControl | Item/Batch, Color/PackedRobot, six fault flags, one 0/1 decision | Preserved; corrected ambiguous fault-flag wording |
| MechanicalArm | Six directions, fixed layouts, hidden inspection, state bits, four tables, delayed readiness, direct normal Packing, FIFO goal | Preserved; JSON slot ordering is explicitly fixed |
| Metrics | Only Elevator adds travel_distance/stop_count | Preserved; v2 fixes visible-case SUM and minimize scoring |

Runtime errors, resource limits, and faults retain existing typed distinctions.
Informal examples of stack underflow/timeout in the conversation are not new VM
rules. Current error identities are not inferred or changed by this audit.

## Example-by-example review

The common envelopes use explicit required fields, current capability names,
positive board limits, one visible case, and no unauthorized initial VM state.
All examples use format 2 and their prescribed scene/family pairing.

| Pair | Valid document review | Invalid variant and offending path |
| --- | --- | --- |
| ExactIO | Empty config, input [9], expected [9], legal bytes | Unknown config key: $.scene_config.unexpected |
| Baudot | Empty config, [1,31] input/expected within five-bit domain | Code 32: $.evaluation.tests[0].input[0] |
| QualityControl | PackedRobot Batch; [1,65] have legal low-bit colors; one expected decision [1] | Reserved color 3: $.evaluation.tests[0].input[0] |
| Elevator | One initial floor 0; one passenger 2→7; empty sequential list; Elevator-only metrics legal | Destination equals origin: $.evaluation.tests[0].initial_passengers[0].to |
| Robot | Exactly 256 explicit cells; position 0 facing RIGHT; position 1 required patrol; no blockers/associations | Trigger references absent door 7: $.evaluation.tests[0].map[0].trigger.door_id |
| MechanicalArm | Four slots; Inspection/Packing; BLUE NORMAL author robot; expected 33 permits direct Packing | Byte 21 means DEFECT BLUE with processed=1: $.evaluation.tests[0].expected_output[0] |

These invalid documents each introduce one intentional rule violation. Their
manifest paths match the offending fields. Valid means author-contract validity,
not a successful program run, proven solvability, or optimal scoring. In
particular, QC expected 1 is author-defined and elevator scoring targets do not
claim that any supplied program attains them.

## Corrections from this review

- Scene-session advance checks constraints/terminal outcomes before appending
  observations. Terminal goals or failures append no new input. Successful
  round capacity is reserved before publishing its queued observation.
- QualityControl fault bits are described as flag values 0/1, not misleading
  references to the color bit positions 0/1.
- Completed A effects retained on B failure may enter permitted visible feedback;
  uncommitted resource candidates and hidden-case data remain excluded.

## Verification and remaining gates

Parsed all 12 example documents and the manifest with a standard JSON reader;
checked concrete array lengths, manifest file/path references, Markdown tables,
code fences, relative links, and `git diff --check`. Per-scene validity above
is a document review, not a second semantic validator in Python/TypeScript.

At the time of the original document audit, the Rust v2 loader was not
implemented. Subsequent loader evidence is recorded below; these examples
alone do not establish native/WASM parity. Stable scene failure transport,
concrete trusted profile fields, public projections, and continuous append
implementation remain delivery gates. The accepted author/gameplay rules are
sufficiently specified to proceed with those integration designs; unseen
conversation tail material cannot be certified by this audit.

## Subsequent integration contract closure

After this audit, the user authorized the [Scene Host Contract v2](../spec/codegrid-scene-host-contract-v2.md),
which fixes the previously pending failure mapping, profile fields, result
variants, and visible Debug events. The remaining gates are implementation
and verification, rather than unspecified wire fields. This does not remove
the conversation-source truncation limitation recorded above.

## Rust loader implementation evidence

The shared `load_scene_level_json` entry point now validates all six author
examples and rejects all six invalid variants with the documented paths.
Seven schema/metric integration tests pass, alongside the 35 existing level core
tests. This verifies author loading and metric policy only; it does not establish
scene execution or native/WASM parity.
