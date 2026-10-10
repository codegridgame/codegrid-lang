# CodeGrid Full Source Language Specification

**Current amendment (2026-10-08):** [Status Flag and directionless READ](../docs/status-flag-and-read.md) records the approved decision implemented by the current source and VM contract.

**Status:** Normative Full source contract; source decisions and implementation gates are recorded in §11  
**File extension:** `.cg`  
**Execution semantics:** [CodeGrid VM Specification](codegrid-vm-spec.md)  
**Authority:** This document defines accepted Full source syntax and static validation. The shared syntax/compiler implementation is the sole source-acceptance authority for every host.

**Implemented (2026-10-08):** `$!` / NEG is accepted by the shared compiler and rebuilt WASM hosts; see §11.

## 1. Source text and lexical units

A source file is UTF-8 text. LF and CRLF are supported line endings; a final line ending is optional. A bare carriage return is invalid. Core source spans are half-open UTF-8 byte offsets. Editor boundaries convert these offsets explicitly to their position encoding.

ASCII space and tab separate cells and directive arguments. Other Unicode whitespace is not a separator. A non-directive, non-comment physical line is a grid row. Blank lines and comment-only lines do not create rows. Every cell atom is one complete token; a token is never split into a supported prefix and ignored suffix.

Line comments start with `//` and continue through the physical line ending. Block comments start with `/*` and end at the next `*/`; they may span LF or CRLF, do not nest, and do not create grid rows or cells. A comment separates adjacent source atoms. Unterminated or nested block comments are invalid. The semicolon `;` is an instruction token and never starts a comment.

Instruction tokens and attachments are case-sensitive. Directive keywords and structural path segments are case-insensitive. Structural IDs use ASCII digits only.

## 2. Program and board structure

A complete program defines exactly one Main CodeGrid and may define zero or more Custom CodeGrids. Every CodeGrid has a Main Board. A Main Board may contain multiple Entry markers, each of which starts an initial thread. A Function Board may contain at most one Entry marker. A Main or Function Board without an Entry marker uses the default entry at (0, 0), facing Right. The default entry does not alter or occupy the cell; execution begins with its actual contents. Explicit Entries suppress the default entry. A Folded Block has no Entry marker and consists of exactly one row.

The source may declare Main explicitly or use the implicit Main form:

```text
@main
@size 5x2
~> _ _ [0 _
_  + #0 ; _
@end main
```

```text
~> + ;
```

The implicit form treats top-level grid rows as the Main Board. After its grid starts, it may define board-local Folded Blocks with `@M<n>` or the equivalent qualified path `@main.M<n>`. A Function or Custom CodeGrid definition requires an explicit `@main` declaration. An explicit Main declaration and implicit Main rows cannot be combined to define two Main Boards. The top-level Main row sequence ends when its first non-size structural definition begins; later top-level grid rows are invalid.

Custom CodeGrid definitions are not nested inside Main or another Custom CodeGrid. Function and Folded Block definitions may be written lexically inside their owning CodeGrid/Board or by an equivalent qualified path. The historical compiler cases establish these qualified forms:

| Definition | Qualified path |
| --- | --- |
| Outer Function | `@main.F<n>` |
| Outer Main Folded Block | `@main.M<n>` |
| Outer Function Folded Block | `@main.F<n>.M<m>` |
| Custom CodeGrid Function | `@C<n>.F<m>` |
| Custom Main Folded Block | `@C<n>.M<m>` |
| Custom Function Folded Block | `@C<n>.F<m>.M<k>` |

Within an open Main or Custom CodeGrid, `@F<n>` defines a Function owned by that CodeGrid and `@M<n>` defines a Folded Block owned by its Main Board. Within an open Function, `@M<n>` defines a Folded Block owned by that Function. `@F<n>.M<m>` is the relative qualified spelling for a Function-owned Folded Block in the current CodeGrid context. Nested and qualified definitions of the same target denote the same structural path; a path may be defined only once.

The structural namespace has ten IDs, `0` through `9`, for each of Custom, Function, and Folded Block. IDs are local to their owner. Reusing an ID in a different CodeGrid or Board scope is valid and denotes a distinct definition. Forms such as `@C10`, `@F10`, `@M10`, `@C00`, and extra or reordered path components are invalid.

## 3. Directives and block delimiters

The directives are:

| Directive | Form | Meaning |
| --- | --- | --- |
| Main | `@main` | Defines the outer Main CodeGrid and its Main Board. |
| Custom | `@C<n>` | Defines Custom CodeGrid `<n>` and its Main Board. |
| Function | One Function path from §2 | Defines a Function Board. |
| Folded Block | One Folded Block path from §2 | Defines a Folded Block row. |
| Size | `@size WIDTHxHEIGHT` | Declares inherited or local dimensions, as specified in §4. |
| End | `@end [NAME]` | Closes the current explicitly delimited definition. |

`@end` accepts zero or one closing name. Names omit the leading `@` and are compared case-insensitively. `main` closes Main; `C<n>` closes a Custom; and `F<n>` or `M<n>` closes the corresponding local Function or Folded Block. For a qualified Function or Folded Block, its local final segment and its complete qualified path are both accepted, such as `F0` or `main.F0`, and `M0` or `C2.F0.M0`. A mismatched name, more than one name, or an `@end` without an open definition is invalid. Explicit Main, Custom, and Function definitions and block-form Folded Block definitions require a matching `@end`. A one-line Folded Block definition whose row follows its path on the same line is a complete shorthand and needs no `@end`:

```text
@main.M0 + > _ ^ _
```

The equivalent block form is:

```text
@main.M0
+ > _ ^ _
@end main.M0
```

Only a Folded Block directive may include cell tokens on its directive line. An `@size` line and comments are not grid rows. Within a definition, the owning board's grid rows must precede its nested definitions; a grid row after a nested definition has begun is invalid. An `@size` declaration may appear between grid rows in its lexical scope; it applies to that complete scope and does not end or split the row sequence.

An unknown directive, malformed path, path with the wrong hierarchy, or definition whose required owner does not exist is invalid. Forward references to definitions are permitted when the referenced definition exists elsewhere in the source.

## 4. Dimensions and grid shape

`@size WIDTHxHEIGHT` contains exactly one argument. Width and height are positive decimal integers written with ASCII digits and separated by lowercase `x`; leading zeroes are allowed. Each dimension must be at most `4,294,967,295`, and the width-by-height cell count must also be at most `4,294,967,295`. These bounds apply identically to native and `wasm32`; zero, malformed values, and values above either bound are invalid.

Size declarations use three lexical scopes, from broadest to narrowest:

1. Program scope: a top-level `@size` applies to the outer CodeGrid and to Custom CodeGrids that do not override it.
2. CodeGrid scope: `@size` within `@main` or `@C<n>` applies to that CodeGrid's Main Board and its Function Boards unless a Function overrides it.
3. Function Board scope: `@size` within a Function applies to that Function Board and overrides inherited dimensions.

The narrowest declared size wins. A size declaration applies to its complete lexical scope rather than only to rows that follow it. Program-scope sizes may appear after Main or Custom definitions; CodeGrid-scope sizes may appear after a Function definition while the owning CodeGrid scope remains open. At most one `@size` may be declared in each lexical scope. `@size` is not valid inside a Folded Block. Size declarations do not add rows or cells. Boards whose dimensions are inferred from source use the same dimension and total-cell bounds.

If no applicable size is declared, a board's dimensions are inferred from its grid. If a size is declared or inherited, the grid must contain exactly the declared number of rows and every row must contain exactly the declared width. No padding rows or cells are implicit. Without a size declaration, all rows must still have equal, nonzero width. A board with no grid rows, zero width, zero height, or a dimension mismatch is invalid. Every cell, including trailing empty cells, is explicit.

Each Custom CodeGrid must define its Main Board. Its Function Boards and Folded Blocks use the same dimension rules as outer definitions. Each Folded Block row must contain exactly as many cells as its owning board's width.

## 5. Cell forms and complete Primary inventory

A cell is one of the following:

| Form | Meaning |
| --- | --- |
| `_` | Empty cell. |
| `~^`, `~v`, `~<`, `~>` | Entry marker with its initial direction. |
| A Primary token from the table below, optionally preceded by one conditional prefix and followed immediately by one suffix Attachment from §6 | One instruction cell. |

An Entry marker is a whole cell and cannot have an Attachment. It is not an instruction. Empty cells and Entry markers do not carry Attachments.

Every Primary token in the canonical language inventory is listed here. Instruction spelling is exact and case-sensitive.

| Primary token(s) | Instruction |
| --- | --- |
| `^`, `v`, `<`, `>` | Set direction. |
| `??` | Choose a random direction. |
| `?=` | Compare thread Data Stack top A without popping with selected register B; write 0 if equal, 1 if A > B, or 2 if A < B. Empty stack preserves the register. |
| `,` | Read a byte and set F=0; on exhaustion retain the register and set F=1. Direction is unchanged. |
| `!` | Clear the current register. |
| `+`, `-` | Add to or subtract from the current register. |
| `$!` | NEG: replace the selected byte with its additive inverse modulo 256. |
| `{`, `}` | Move the register pointer left or right. |
| `.` | Output the current register. |
| `.0`–`.9` | Output the raw byte 0–9 without changing registers or the register pointer. |
| `(`, `)` | Push the current register; pop and add. |
| `&`, `%` | Decode or encode the instruction stack. |
| `[0`–`[9` | Call Function `0`–`9` in the current CodeGrid. |
| `]` | Return from a Function. |
| `$&` | NAND operation. |
| `$(`, `$)` | Load from or store to memory. |
| `$+`, `$-` | Move the Page forward or backward. |
| `$<`, `$>` | Shift the current register left or right. |
| `$0`–`$9` | Enter Folded Block `0`–`9` owned by the current Board. |
| `#0`–`#9` | Call Custom CodeGrid `0`–`9`. |
| `#]` | Return from a Custom invocation. |
| `;` | Halt. |

Prefixes are not tokens by themselves. In particular, standalone `#`, `$`, `,`, `[`, or `x` are invalid. Malformed or unknown atoms (for example `#00`, `++`, `~V`, or `#0x3`) are rejected as complete atoms and never split into valid instructions.

## 6. Attachments

A conditional prefix `?0`, `?1`, `?2`, or `?!` may precede every otherwise valid Primary, requiring equality of the selected register to byte 0, 1, or 2, or active F=1 for `?!`. A cell may have at most one prefix and one suffix, concatenated without whitespace in that order. Initial Empty and Entry cells cannot carry either. Prefixes are allowed inside Folded Blocks; suffixes remain forbidden there. Prefixes do not relax placement or reference validation. Examples: `?0+`, `?1#0`, `?2;`, `?0.3`, `?1?=`, `?0??`, and `?1+x3`. Standalone or repeated prefixes and values outside 0–2 are invalid. The removed directional READ atoms `,^`, `,v`, `,<`, and `,>` are invalid, including prefixed and suffixed forms; they are never split into two cells. `?=` is one complete CMP Primary; `?==` adds WriteCode and `?=*` adds ReadCode.

The rules below describe suffix Attachments independently of prefixes.

An Attachment is concatenated directly to a Primary token in the same cell. The canonical Attachments are:

| Attachment | Meaning |
| --- | --- |
| `*` | ReadCode. |
| `=` | WriteCode. |
| `x2`, `x3`, `x4`, `x5` | Repeat with the stated count. |

Examples of complete attached cells are `+x3`, `,*`, and `[0=`. An Attachment separated by whitespace from its Primary is detached and invalid. A cell cannot contain more than one suffix Attachment. Entry markers and HALT cannot carry a suffix. Folded Block rows cannot contain suffix Attachments.

NEG is an encodable ordinary Primary. Its complete forms include `$!`,
`?0$!`, `$!*`, `$!=`, `$!x2`, and `?1$!x3`. Unicode multiplication signs
are not Repeat syntax; `$!×2` and whitespace-separated `?0 $!` or `$! x2`
are invalid. NEG is allowed on outer and Custom Main and Function Boards,
and in Folded Blocks without a suffix, under the existing placement rules.

ReadCode and WriteCode may be attached to any encodable Primary, defined as a Primary with a normative Instruction Code. Repeat may be attached to any encodable Primary except CALL and RETURN, and to immediate output Primaries `.0`–`.9`. Folded Block calls, Custom calls, Custom returns, and HALT are not encodable and cannot carry suffix Attachments. No other Primary-suffix combination is valid. Conditional prefix eligibility is independent.

## 7. Contextual placement and name resolution

Immediate output Primaries `.0`–`.9` have no Instruction Code and may carry only
Repeat suffixes `x2`–`x5`; conditional prefixes are allowed. `.3x3` executes
three immediate outputs on separate ticks before moving. Forms such as `.3*`, `.3=`, `.3x1`, `.10`, `.00`, and `.-1`
are rejected as whole atoms. Ordinary `.` retains its encoding and Attachments.

Static validation checks each Primary against its owning CodeGrid and Board:

- `]` is valid only on a Function Board.
- `#]` is valid only directly on a Custom Main Board; it is not valid on an outer board or a Function Board.
- A Function CALL `[n` resolves only to Function `n` in the same CodeGrid. Calls may be forward references. A reference to an undefined Function is invalid.
- A Folded Block call `$n` resolves only to Folded Block `n` owned by the current Board. It cannot resolve to a Main Board or Function Board owned Folded Block in another scope.
- A Custom call `#n` resolves to a declared Custom CodeGrid. Custom calls are permitted from outer CodeGrid code, including the outer Main Board, Function Boards, and outer-owned Folded Blocks. They are forbidden from inside a Custom CodeGrid, including its Main Board, Function Boards, and Folded Blocks.
- Function and Folded Block IDs with the same digit in another CodeGrid or Board do not satisfy a reference.
- Folded Blocks do not contain Entry markers, suffix Attachments, Function CALLs, Folded Block calls, Function RETURNs, or Custom RETURNs. Other Primary instructions are allowed subject to the Custom-call rule above.

Function tail-call eligibility is compiler analysis, not a source-validity condition: both navigation-only and side-effecting return paths may contain syntactically valid recursive calls. A compiler may annotate eligible calls according to the VM contract without rejecting other valid calls.

## 8. Static acceptance requirements

Before returning executable output, the shared compiler must reject source that violates any of these requirements:

- Invalid UTF-8 input at a host boundary, bare carriage returns, malformed/nested/unterminated comments, malformed directives, or malformed complete cell atoms.
- Missing or duplicate Main definitions; absent required Main grids; Custom CodeGrids without Main grids; invalidly nested Custom definitions; duplicate structural paths; malformed or out-of-range structural IDs; missing, extra, mismatched, or incorrectly scoped `@end` directives.
- Missing or duplicate size declarations within one lexical scope; malformed, zero, unrepresentable, or overflowing dimensions; size directives in Folded Blocks; empty, ragged, or dimension-mismatched boards; implicit padding; Folded Block width mismatch.
- Missing or invalid Entry counts: at least one on outer and Custom Main Boards, exactly one on each Function Board, and none on Folded Blocks.
- Unknown, undefined, or out-of-scope Function, Custom, or Folded Block references; invalid placement of `]`, `#]`, Custom calls, Folded Block forms, or Attachments.
- Multiple or detached Attachments and any Attachment combination not allowed by the resolved compatibility matrix.

Invalid source never produces executable IR. Diagnostics are structured and identify the offending token, directive, definition, or board where a source span exists. Core spans are UTF-8 byte offsets. Syntax recovery may continue to report independent issues, but no host may execute partially accepted source or independently reproduce these acceptance rules.

Each diagnostic carries a stable `code` assigned at its validation origin. The [error code specification](codegrid-error-codes.md) defines identifiers and compatibility separately from message wording. This additive diagnostic decision is recorded in `docs/decisions.md` on 2026-09-30; acceptance rules are unchanged.

## 9. Complete examples

The repository examples [`simple.cg`](../examples/simple.cg), [`echo.cg`](../examples/echo.cg), [`nested.cg`](../examples/nested.cg), and [`folded-block.cg`](../examples/folded-block.cg) illustrate Main, I/O, qualified and nested definitions, and Folded Blocks. They are explanatory examples; this specification and focused conformance cases define acceptance.

## 10. Evidence reviewed

This contract was reconstructed from the complete instruction and Attachment inventory in [`codegrid-model`](../crates/codegrid-model/src/lib.rs), the source/path types in [`codegrid-syntax`](../crates/codegrid-syntax/src), retained Full compiler cases in [`source_compilation_full_v2.rs`](../crates/codegrid-compiler/tests/historical/source_compilation_full_v2.rs), the repository examples, and explicit decisions in [`decisions.md`](../docs/decisions.md). The complete Primary-Attachment compatibility matrix is specified in §6 and recorded in the resolved source decisions; historical tests provide supporting cases rather than replacing the normative rules.

## 11. Resolution record and implementation gate

The 2026-10-08 NEG decision is implemented in the shared model/compiler, verified IR, level capability validation, editor metadata, and actual native/browser/Node/Wasmtime fixtures. See [implementation evidence](../docs/function-registers-and-neg.md).

The source-language decisions are resolved in the [source decision record](../docs/decisions.md#resolved-source-language-decisions), and the rules in §§1–8 are the normative Full acceptance contract. The implementation gate is covered by focused compiler and IR tests: `compiles_every_resolved_attachment_pair_for_each_encodable_primary`, `verifies_the_complete_full_primary_attachment_compatibility_matrix`, `rejects_invalid_attachment_placement_and_repeat_counts`, `rejects_dimensions_and_total_cells_above_the_portable_source_limit`, `rejects_board_dimensions_and_cell_counts_above_portable_full_bounds`, `size_between_rows_applies_to_the_complete_lexical_scope`, `implicit_main_accepts_qualified_folded_blocks_after_its_grid`, `accepts_named_end_aliases_for_custom_and_function_folded_blocks`, and `rejects_mismatched_custom_and_function_folded_block_end_aliases`. The latter cover explicit Custom Main, outer Function, and Custom Function Folded Blocks with matching local and qualified aliases, plus mismatches. These cases verify implementation coverage; they are not unresolved language decisions.

This Full contract replaces the source restrictions recorded by earlier language profiles. No earlier profile's rejection list narrows Full source acceptance.

## 12. Conditional prefix and CMP migration (2026-10-03)

The approved [decision record](../docs/decisions.md#conditional-prefix-migration-design-2026-10-03) removes the four IfZero forms and standalone random `?`. They are rejected as complete atoms. RandomDirection is `??` and CMP is `?=`. Prefixes and suffixes are separate categories; references and forbidden Primaries remain invalid even when guarded. This migration introduced executable IR format 2. The 2026-10-08 Function/NEG implementation uses format 3 and rejects formats 1 and 2; recompile source.
