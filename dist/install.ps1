param(
    [string]$DataPath = "",
    [string]$InstallDir = "$env:LOCALAPPDATA\mcp-chinese-dict-search"
)

$ErrorActionPreference = "Stop"
$Here = Split-Path -Parent $MyInvocation.MyCommand.Path
$SourceExe = Join-Path $Here "mcp-chinese-dict-search.exe"

if (-not (Test-Path -LiteralPath $SourceExe)) {
    throw "mcp-chinese-dict-search.exe was not found beside install.ps1."
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item -LiteralPath $SourceExe -Destination $InstallDir -Force
$SourceEval = Join-Path $Here "evaluate-semantic.exe"
if (Test-Path -LiteralPath $SourceEval) {
    Copy-Item -LiteralPath $SourceEval -Destination $InstallDir -Force
}
$License = Join-Path $Here "LICENSE"
if (Test-Path -LiteralPath $License) {
    Copy-Item -LiteralPath $License -Destination $InstallDir -Force
}

$InstalledExe = Join-Path $InstallDir "mcp-chinese-dict-search.exe"

if ([string]::IsNullOrWhiteSpace($DataPath)) {
    $DataPath = Read-Host "Path to the dictionary .zip (or its extracted directory)"
}
$DataPath = $DataPath.Trim().Trim('"')

if (-not (Test-Path -LiteralPath $DataPath)) {
    Write-Warning "The data path does not exist yet: $DataPath"
    Write-Warning "The client entry will be written anyway; correct the path before starting the client."
}

$Config = [ordered]@{
    mcpServers = [ordered]@{
        "chinese-medicine-dictionary" = [ordered]@{
            command = $InstalledExe
            args    = @("--data", $DataPath)
        }
    }
}

$ConfigPath = Join-Path $InstallDir "mcp-config.json"
$Config | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $ConfigPath -Encoding UTF8

Write-Host ""
Write-Host "Installed to: $InstallDir"
Write-Host "MCP config:   $ConfigPath"
Write-Host ""
Write-Host "Add the mcpServers entry from that file to your Claude Code, OpenCode, or DeepSeek Harness configuration."
Write-Host "The dictionary itself was not copied; it remains at: $DataPath"
