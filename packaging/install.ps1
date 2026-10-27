# ==============================================================================
# 🌹 Official Roze Programming Language Installer for Windows (v1.0.0)
# Usage in PowerShell:
#     iwr -useb https://roze-lang.org/install.ps1 | iex
# ==============================================================================

$ErrorActionPreference = "Stop"

$RozeVersion = "1.0.0"
$InstallDir = Join-Path $env:LOCALAPPDATA "Roze"
$BinDir = Join-Path $InstallDir "bin"
$StdlibDir = Join-Path $InstallDir "stdlib"

Write-Host "==============================================================================" -ForegroundColor Magenta
Write-Host "🌹 Installing Roze Programming Language v$RozeVersion for Windows" -ForegroundColor Magenta
Write-Host "==============================================================================" -ForegroundColor Magenta

# Create target directories
if (!(Test-Path $BinDir)) { New-Item -ItemType Directory -Force -Path $BinDir | Out-Null }
if (!(Test-Path $StdlibDir)) { New-Item -ItemType Directory -Force -Path $StdlibDir | Out-Null }

# Locate script root
$ScriptDir = Split-Path -Parent $PSScriptRoot
if (!$ScriptDir) { $ScriptDir = Get-Location }

# Copy prebuilt binaries if present
$SourceBin = Join-Path $ScriptDir "target\release"
if (Test-Path (Join-Path $SourceBin "roze.exe")) {
    Write-Host "📦 Copying binaries from $SourceBin..." -ForegroundColor Cyan
    Copy-Item -Path (Join-Path $SourceBin "roze*.exe") -Destination $BinDir -Force
} else {
    Write-Host "⚡ Building Roze toolchain via Cargo..." -ForegroundColor Cyan
    cargo build --release --workspace
    Copy-Item -Path (Join-Path $SourceBin "roze*.exe") -Destination $BinDir -Force
}

# Copy standard library
$SourceStdlib = Join-Path $ScriptDir "stdlib"
if (Test-Path $SourceStdlib) {
    Copy-Item -Path "$SourceStdlib\*" -Destination $StdlibDir -Recurse -Force
}

# Add BinDir to User PATH environment variable
$UserPath = [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::User)
if ($UserPath -notlike "*$BinDir*") {
    $NewPath = "$UserPath;$BinDir"
    [Environment]::SetEnvironmentVariable("Path", $NewPath, [EnvironmentVariableTarget]::User)
    Write-Host "✅ Added $BinDir to User PATH" -ForegroundColor Green
}

[Environment]::SetEnvironmentVariable("ROZE_STDLIB_PATH", $StdlibDir, [EnvironmentVariableTarget]::User)

Write-Host ""
Write-Host "==============================================================================" -ForegroundColor Green
Write-Host "🎉 Roze v$RozeVersion installed successfully!" -ForegroundColor Green
Write-Host "Location: $InstallDir" -ForegroundColor Green
Write-Host "==============================================================================" -ForegroundColor Green
Write-Host ""
Write-Host "Restart your PowerShell terminal, then run:" -ForegroundColor Yellow
Write-Host "    roze --version" -ForegroundColor White
Write-Host "    roze-pkg --version" -ForegroundColor White
Write-Host ""
