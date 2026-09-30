param([string]$MaximumMemoryBytes = '67108864')
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
Push-Location $taskRoot
try {
    node scripts/check-level-toolchain.mjs
    if ($LASTEXITCODE) { throw 'Pinned level toolchain mismatch' }
    $env:CODEGRID_WASM_MAX_MEMORY_BYTES = $MaximumMemoryBytes
    cargo test --locked -p codegrid-level-wasm-browser -p codegrid-level-wasm-server
    if ($LASTEXITCODE) { throw 'Native adapter tests failed' }
    cargo build --locked --release --target wasm32-unknown-unknown -p codegrid-level-wasm-browser -p codegrid-level-wasm-server
    if ($LASTEXITCODE) { throw 'WASM build failed' }
    wasm-bindgen --target web --out-dir target/level-browser-bindings target/wasm32-unknown-unknown/release/codegrid_level_wasm_browser.wasm
    if ($LASTEXITCODE) { throw 'Browser binding generation failed' }
    cargo build --locked -p codegrid-cli
    if ($LASTEXITCODE) { throw 'Native CLI build failed' }
    node crates/codegrid-level-wasm-server/tests/server-smoke.mjs
    if ($LASTEXITCODE) { throw 'Portable host comparison failed' }
    node crates/codegrid-level-wasm-browser/tests/run-browser-smoke.mjs
    if ($LASTEXITCODE) { throw 'Browser worker comparison failed' }
    cargo run --locked --manifest-path tools/wasmtime-host/Cargo.toml --bin level-parity -- $taskRoot (Join-Path $taskRoot 'target/wasm32-unknown-unknown/release/codegrid_level_wasm_server.wasm')
    if ($LASTEXITCODE) { throw 'Wasmtime portable comparison failed' }
    node scripts/compare-level-hosts.mjs
    if ($LASTEXITCODE) { throw 'Full semantic host comparison failed' }
    node scripts/record-level-build.mjs
    if ($LASTEXITCODE) { throw 'Build provenance recording failed' }
} finally { Pop-Location }
