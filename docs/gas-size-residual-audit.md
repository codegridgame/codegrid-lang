# Gas, Size and Custom residual-contract audit

Date: 2026-10-11. Scope: Rust implementation, current specifications, usage
documentation and downstream consumers of Rust execution. This audit follows
the [implementation record](gas-size-implementation.md).

## Findings and corrections

| Area | Residual claim | Correction |
| --- | --- | --- |
| Browser README | Constructor and create example omitted required Gas inputs | Added trusted Gas ceiling, instance budget and exact metric fields |
| Server README | Examples omitted Gas; Wasmtime claimed to use obsolete ABI | Added Gas and current ABI 4/API 3 evidence; ceiling accepts positive u64 |
| CLI contract | Gas option, debug setting and result fields absent; Custom described as running | Added Gas configuration, breakdown, schedule and disabled dispatch |
| Runtime API spec | Main record omitted Gas despite amendment; Custom limits presented as active | Updated records and limits directly, distinguishing retained semantics |
| VM spec | Custom implied production availability; NEG said no special cost | Explicit disabled dispatch and retained-body scope; NEG Gas fee |
| Level/scene specs | Obsolete Custom errors, custom stack metric and aggregation; loader described as awaiting migration | Current errors, vocabulary, Gas sums/static Size and loader status |
| Indexes | IR 2 and unimplemented Status Flag claims | Current IR 3, F/READ status and implementation links |
| Synchronization guidance | Unpublished old readers/artifacts appeared mandatory | Applied published-baseline rule directly |

## Implementation review

Production Rust contains no `cost`, `max_cost`, `non_empty_cells` or
`max_non_empty_cells` metric readers, aliases or scoring logic. Remaining
literals occur in rejection tests proving those fields are absent or invalid.
The VM calculates Gas; level aggregation uses checked Gas/breakdown sums and
static Size once. Hosts forward those values.

Production `VmConfig::custom_execution_enabled()` always returns false.
Custom dispatch checks it before charging the invocation fee or entering the
body. The enabling method is crate-private and compiled only with `cfg(test)`;
it preserves retained semantic coverage and cannot be called by a host.
Custom validation and capability identifiers deliberately remain. Neither
a capability whitelist nor a Custom tick limit enables execution.

No executable source changed in this cleanup, so WASM bytes and build identities
remain valid. Downstream Rust DTOs, host requests, metric readers and manifests
were inspected: current Gas/Size names and limits are present with actual
artifact identities. The downstream sync record was updated; these wording
corrections require no artifact rebuild.

## Deliberately retained references

- The original Rust review is clearly historical and describes preimplementation
  code. It is not the current acceptance contract.
- Dated decisions and acceptance records retain their original fixture counts
  and hashes. They do not create compatibility duties.
- `prefix-operation-cost.json` fixture paths/IDs name operation-accounting
  scenarios. Their payloads use current Gas/Size fields; paths are not aliases.
- Retained Custom body rules and test-only execution are intentional. Future
  Custom Scene/CustomJudge architecture is separate from VM Custom execution.

Existing downstream TypeScript type errors and broad editor E2E limits remain
as recorded in the implementation report; this audit does not resolve them.

## Verification of this cleanup

- `cargo test -p codegrid-level-core --test scene_schema_v2`: 10 passed,
  including rejection of former author metric names.
- `cargo test -p codegrid-vm --test gas`: 6 passed, including exact-limit
  commit, attempted-Gas rollback, warm memory and conditional/Repeat charging.
- `git diff --check` passed. Targeted rescans found old metric literals only
  in rejection tests or explicitly historical/explanatory text.
