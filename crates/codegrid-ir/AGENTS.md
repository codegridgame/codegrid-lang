# Module Rules: `codegrid-ir`

## Responsibility

Own the versioned executable representation, its construction invariants, and verification required before execution.

Expose stable `IrError.code` categories from `spec/codegrid-error-codes.md` at
verification origins; the compiler forwards them without message classification.

## Allowed dependency and call direction

- This crate may depend only on `codegrid-model` among workspace crates.
- `codegrid-compiler` lowers validated source into verified IR; `codegrid-vm` accepts verified IR for execution.
- `codegrid-runtime-api` may expose public IR identity types in its host data contract, but executable programs must still enter through compiler-produced verified handles.
- Any decoded or deserialized representation must pass the verifier before it can produce an executable program.

## Prohibited

- Do not parse `.cg` source, add a runtime interpreter, access files/network, or depend on VM/host/editor crates.
- Do not allow external callers to construct or execute unchecked IR.
- Do not conflate source-language version, IR format version, and host API version.

## Validation

Add verifier tests for malformed identities, references, layouts, and version boundaries. Check native and supported `wasm32` builds when available.
