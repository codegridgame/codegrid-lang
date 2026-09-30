param(
    [string]$OutputPath = '',
    [string]$Target = 'win32-x64'
)
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$editorRoot = Join-Path $repositoryRoot 'editors/vscode'
$hostPlatform = if ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)) { 'win32' } elseif ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::OSX)) { 'darwin' } else { 'linux' }
$hostArchitecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLower()
if ($Target -ne "$hostPlatform-$hostArchitecture") {
    throw "Target $Target does not match the native build host $hostPlatform-$hostArchitecture."
}
if (!$OutputPath) {
    $OutputPath = Join-Path $repositoryRoot 'target/packages/codegrid-vscode-0.4.2-win32-x64.vsix'
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $OutputPath) | Out-Null
Push-Location $repositoryRoot
try {
    & cargo build --release -p codegrid-cli
    if ($LASTEXITCODE -ne 0) { throw 'Native runtime build failed.' }
    $runtimeDirectory = Join-Path $editorRoot 'runtime'
    New-Item -ItemType Directory -Force -Path $runtimeDirectory | Out-Null
    $executableName = if ($env:OS -eq 'Windows_NT') { 'codegrid.exe' } else { 'codegrid' }
    Copy-Item -LiteralPath (Join-Path $repositoryRoot "target/release/$executableName") -Destination $runtimeDirectory -Force
    Copy-Item -LiteralPath (Join-Path $repositoryRoot 'spec/codegrid-error-codes.md'), (Join-Path $repositoryRoot 'spec/codegrid-error-codes.json') -Destination $runtimeDirectory -Force
    Push-Location $editorRoot
    try {
        if ($env:OS -eq 'Windows_NT') {
            & './node_modules/.bin/vsce.cmd' package --allow-missing-repository --target $Target --out $OutputPath
        } else {
            & './node_modules/.bin/vsce' package --allow-missing-repository --target $Target --out $OutputPath
        }
        if ($LASTEXITCODE -ne 0) { throw 'VS Code extension packaging failed.' }
    } finally { Pop-Location }
} finally { Pop-Location }
Write-Output "Packaged extension: $OutputPath"
