param([string]$MaximumMemoryBytes = '67108864')
$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'test-level-wasm.ps1') -MaximumMemoryBytes $MaximumMemoryBytes
if ($LASTEXITCODE) { throw 'Scene host verification failed' }
