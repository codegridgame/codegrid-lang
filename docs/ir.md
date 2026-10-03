# CodeGrid Full Intermediate Representation

## Layer boundary

CodeGrid separates source, compilation, executable program, and mutable runtime state:

```text
UTF-8 source
  -> lossless syntax and byte spans
  -> resolved Full HIR and symbols
  -> verified executable IR
  -> isolated VM instance
```

- `codegrid-syntax` owns tokenization, source structure, and UTF-8 byte spans.
- `codegrid-hir` represents Main, Function, Custom, and Folded Block structure, references, and source locations.
- `codegrid-compiler` owns static source validation, symbol resolution, tail-call analysis, and lowering.
- `codegrid-ir` owns the versioned executable representation and verifier.
- `codegrid-vm` creates mutable execution state only from verified programs.

## Full executable program

Full IR represents all executable CodeGrids, board relationships and paths, dimensions, row-major cells, Entry markers, Primary instructions, Attachments, and static execution metadata. It does not contain mutable thread state, input/output cursors, memory contents, stacks, or other per-instance runtime data.

The current executable Full IR format is 2; format 1 is rejected. Cells store fixed conditional prefixes independently of their Primary and suffix, and Folded Blocks store prefix metadata by occupied cell index. Rust struct layout is not a host interchange format. No adapter may expose it as a byte ABI or treat its in-memory representation as stable serialization.

## Verification boundary

`VerifiedProgram` is the only supported VM input. The Full verifier validates at least:

- IR version, bounded and internally consistent collections, and all referenced definitions;
- Main/Function/Custom/Folded Block shape, dimensions, owner relationships, and Entry constraints;
- row-major cell counts, complete Primary inventory, legal Attachments and combinations;
- function and Custom call targets, stack-encodable instruction forms, and derived tail-call metadata;
- any static metadata that changes execution or is serialized into IR.

The verifier returns structured errors for malformed external IR. Any decoder, cache loader, or future host-input path must invoke it; a caller-supplied “validated” flag is not sufficient. Source-backed programs preserve paths to source spans so diagnostics and tooling can locate the originating token.

## Version boundaries

Source profile, IR format, CLI result JSON, Runtime API, and WASM ABIs are separate contracts. Assign each version independently after the Full schema and migration plan are settled. Never infer accepted source syntax from an IR/API/ABI version, and never pass wide integers through imprecise JavaScript numbers.

## Conditional prefix and CMP migration (2026-10-03)

The approved source/VM migration uses executable IR format 2, RandomDirection `??` (126), CMP `?=` (124), and fixed conditional prefixes `?0`–`?2`. Normal code-view cells expose an additive nullable `prefix` field containing the canonical prefix spelling. This field survives Primary mutation and clearing. Folded Block views keep their existing arrays of nullable strings; nonempty strings include any prefix followed by the Primary token. These are read-only projections, not executable interchange data.

The Runtime API v3, browser binding envelope, server ABI v4, CLI JSON schema 1, and debug protocol 2 remain unchanged; existing lifecycle and transport fields are preserved. Language source acceptance, IR version, capability vocabulary, and new code-view fields follow the recorded migration decision. Removed source forms and obsolete instruction bytes are not compatibility aliases. Consumers displaying cells should include the prefix.
