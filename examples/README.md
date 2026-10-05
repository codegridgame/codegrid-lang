# Full Language Examples

These programs illustrate the intended Full CodeGrid language. The [Full task book](../tasks/spec-completion.md) and completed source/VM specifications define acceptance; examples are not normative by themselves.

- [`simple.cg`](simple.cg): Main Board movement, arithmetic, and output.
- [`echo.cg`](echo.cg): input, output, and termination.
- [`nested.cg`](nested.cg): nested Function and Custom CodeGrid definitions.
- [`folded-block.cg`](folded-block.cg): Folded Block execution.

Review each program against the completed Full source specification before using it as conformance evidence.

## Scene author documents

[Scene Level JSON v2 examples](scene-level-v2/README.md) provide six complete valid author documents and six invalid counterparts. They illustrate the decided author schema, are currently unsupported by the format-1 loader, and are not execution conformance evidence.

[Scene host v2 examples](scene-host-v2/README.md) illustrate the trusted profile, Debug feedback request/data, and visible case failure. Fragments and profile examples are clearly distinguished from executable author levels and existing API support.

## Executable scene conformance fixtures

The [scene-v2 fixtures](../fixtures/scene-v2/README.md) pair author definitions
with compiled Full programs and a cross-host execution manifest. These are
separate from the document-shape examples above. See [WASM transports v2](../docs/level-wasm-v2.md)
for commands and the actual-host verification scope.
