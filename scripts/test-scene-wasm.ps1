param([string]$MaximumMemoryBytes = '67108864')
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
Push-Location $taskRoot
try {
    & (Join-Path $PSScriptRoot 'test-level-wasm.ps1') -MaximumMemoryBytes $MaximumMemoryBytes
    $env:CODEGRID_WASM_MAX_MEMORY_BYTES = $MaximumMemoryBytes
    node scripts/test-scene-hosts.mjs
    if ($LASTEXITCODE) { throw 'Native/portable scene comparison failed' }
    node crates/codegrid-level-wasm-browser/tests/run-browser-smoke.mjs --scene-v2
    if ($LASTEXITCODE) { throw 'Scene browser worker comparison failed' }
    cargo run --locked --manifest-path tools/wasmtime-host/Cargo.toml --bin level-parity -- $taskRoot (Join-Path $taskRoot 'target/wasm32-unknown-unknown/release/codegrid_level_wasm_server.wasm') --scene-v2
    if ($LASTEXITCODE) { throw 'Scene Wasmtime comparison failed' }
    node scripts/compare-scene-hosts.mjs
    if ($LASTEXITCODE) { throw 'Complete scene result/event comparison failed' }
} finally { Pop-Location }
