#!/usr/bin/env pwsh
# Graphenium — One-line PowerShell installer for Windows
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File install.ps1
#   powershell -ExecutionPolicy Bypass -File install.ps1 -InstallDir ~/.graphenium

param(
    [string]$InstallDir = "$env:USERPROFILE\.graphenium"
)

Write-Host "Graphenium Installer for Windows" -ForegroundColor Cyan
Write-Host "================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Check for Rust/Cargo
$cargoPath = Get-Command "cargo" -ErrorAction SilentlyContinue
if (-not $cargoPath) {
    Write-Host "[!] Rust/Cargo not found." -ForegroundColor Yellow
    Write-Host "    Install Rust from https://rustup.rs and try again."
    Write-Host ""
    Write-Host "    Quick install:"
    Write-Host "      winget install Rustlang.Rustup"
    Write-Host "      rustup default stable"
    exit 1
}
Write-Host "[OK] Cargo found: $($cargoPath.Source)" -ForegroundColor Green

# Step 2: Clone or update repo
if (Test-Path "$InstallDir") {
    Write-Host "[..] Updating existing clone in $InstallDir ..."
    Push-Location "$InstallDir"
    git pull --ff-only
    Pop-Location
} else {
    Write-Host "[..] Cloning Graphenium to $InstallDir ..."
    git clone "https://github.com/lambda-alpha-labs/Graphenium" "$InstallDir"
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[!] Clone failed." -ForegroundColor Red
        exit 1
    }
}
Write-Host "[OK] Repository ready." -ForegroundColor Green

# Step 3: Build and install
Write-Host "[..] Building Graphenium (cargo install --locked --path .) ..."
Push-Location "$InstallDir"
cargo install --locked --path .
if ($LASTEXITCODE -ne 0) {
    Write-Host "[!] Build failed." -ForegroundColor Red
    Pop-Location
    exit 1
}
Pop-Location
Write-Host "[OK] Graphenium installed. Binary: gm.exe" -ForegroundColor Green

# Step 4: Copy SKILL.md for Claude Code
$skillDir = "$env:USERPROFILE\.claude\skills\graphenium"
$skillSrc = "$InstallDir\skills\graphenium\SKILL.md"
if (Test-Path $skillSrc) {
    if (-not (Test-Path $skillDir)) {
        New-Item -ItemType Directory -Path $skillDir -Force | Out-Null
    }
    Copy-Item -Path $skillSrc -Destination "$skillDir\SKILL.md" -Force
    Write-Host "[OK] Claude Code skill installed to $skillDir" -ForegroundColor Green
} else {
    Write-Host "[!] SKILL.md not found at $skillSrc" -ForegroundColor Yellow
}

# Step 5: Verify installation
$gmPath = Get-Command "gm" -ErrorAction SilentlyContinue
if (-not $gmPath) {
    $gmPath = Get-Command "gm.exe" -ErrorAction SilentlyContinue
}
if ($gmPath) {
    $version = & $gmPath.Source --version 2>&1
    Write-Host "[OK] gm available: $($gmPath.Source) ($version)" -ForegroundColor Green
} else {
    Write-Host "[!] gm not found in PATH. Make sure ~/.cargo/bin is in your PATH." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "  1. cd your-project"
Write-Host "  2. gm init"
Write-Host "  3. gm run . --no-semantic"
Write-Host "  4. claude mcp add graphenium --scope user -- gm serve"
Write-Host ""
Write-Host "Done." -ForegroundColor Cyan
