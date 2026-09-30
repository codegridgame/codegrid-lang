# Historical WASM Distribution

This guide documents the existing restricted-profile package and release
process. It is not the Full WASM distribution contract. Update versions,
manifest contents, API/ABI compatibility, and fixture coverage after the Full
host specifications are approved.

The `wasm-mvp-0.1.0.zip` package is the versioned handoff for game hosts that
consume CodeGrid MVP 0.1. It contains generated browser bindings for web and
Node.js, the no-import server ABI module, a manifest with SHA-256 hashes, and
the license. The manifest records the source profile, Runtime API version,
server ABI version, configured linear-memory maximum, and `wasm-bindgen` CLI
version.

## Build the package

Install the `wasm32-unknown-unknown` Rust target, `wasm-bindgen-cli` version
`0.2.129`, Node.js, and Python 3.11 or later. Set
`CODEGRID_WASM_MAX_MEMORY_BYTES` to a deployment-selected multiple of 65,536
bytes within the wasm32 address-space ceiling. The value below matches the
current CI smoke configuration; it is not a production process-memory budget.

```powershell
$env:CODEGRID_WASM_MAX_MEMORY_BYTES = "67108864"
cargo build --locked --release --target wasm32-unknown-unknown -p codegrid-wasm-browser -p codegrid-wasm-server
wasm-bindgen target/wasm32-unknown-unknown/release/codegrid_wasm_browser.wasm --target web --out-dir target/browser-bindings
wasm-bindgen target/wasm32-unknown-unknown/release/codegrid_wasm_browser.wasm --target nodejs --out-dir target/browser-node-bindings
python scripts/package_wasm_mvp.py --version 0.1.0
```

The package is written to `dist/wasm-mvp-0.1.0.zip`. A matching
`wasm-mvp-v0.1.0` Git tag causes the Rust CI workflow to build, validate, and
attach that package to a GitHub Release. Consumers should pin a release tag and
verify the package manifest hashes instead of relying on a moving branch.
Before packaging, the script checks that both generated browser modules and the
server module encode the configured linear-memory maximum.

## Package contents

- `browser/web/` contains the browser-target `wasm-bindgen` JavaScript and WASM
  files.
- `browser/nodejs/` contains the Node.js-target binding and WASM files.
- `server/codegrid_wasm_server.wasm` is the no-import server ABI v3 module. A
  host must follow the allocation, request write, process, response copy, and
  buffer release protocol in the [server adapter contract](../crates/codegrid-wasm-server/README.md).
- `manifest.json` records compatibility versions, the linear-memory maximum,
  and SHA-256 hashes for every packaged payload file.
- `README.md` and `LICENSE` describe package use and licensing.

The browser and server artifacts share the same Rust compiler, verified IR,
Runtime API, and VM. The server module recompiles submitted source; clients must
not submit unchecked IR or treat a browser result as authoritative. Linear
memory and serialized-payload ceilings do not bound the embedding host's
process memory, CPU time, cancellation, or concurrency. Verify those budgets in
the selected runtime before production deployment.
