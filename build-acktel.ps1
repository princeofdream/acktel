#!/usr/bin/env pwsh
# Build acktel for AckShell on Windows
# Pure Rust project — just needs cargo, no CMake/Clang/Ninja required

param(
    [Alias("a")]
    [Parameter(Position=0)]
    [ValidateSet("x86_64", "x86", "aarch64")]
    [string]$Arch = "x86_64",

    [Alias("o")]
    [string]$OutputDir = "",

    [Alias("b")]
    [string]$BuildDir = "",

    [Alias("t")]
    [string]$TargetTriple = "",

    [Alias("c")]
    [switch]$Clean,

    [Alias("d")]
    [ValidateSet("Release", "Debug", "RelWithDebInfo")]
    [string]$BuildType = "Release",

    [Alias("h")]
    [switch]$Help
)

function Show-Usage {
    Write-Host @"
Usage: $(Split-Path -Leaf $PSCommandPath) [-a <arch>] [-o <output_dir>] [-t <triple>] [-d <build_type>] [-c] [-h]

Build acktel for AckShell on Windows using cargo.

Arguments:
  -a, --arch <arch>        Architecture: x86_64, x86, aarch64 (default: x86_64)
  -o, --output <dir>       Output prefix for installed binary (default: <repo>/output/target/windows/<arch>)
  -t, --target <triple>    Rust target triple (e.g. aarch64-pc-windows-msvc)
  -d, --build-type <type>  Build type: Release, Debug, RelWithDebInfo (default: Release)
  -c, --clean             Clean build artifacts before building
  -h, --help              Show this help message

Examples:
  $(Split-Path -Leaf $PSCommandPath) -a x86_64
  $(Split-Path -Leaf $PSCommandPath) -a x86_64 -d Debug
  $(Split-Path -Leaf $PSCommandPath) -a aarch64 -t aarch64-pc-windows-msvc -o C:\install
  $(Split-Path -Leaf $PSCommandPath) -c
"@
    exit 0
}

if ($Help) {
    Show-Usage
}

$ErrorActionPreference = "Stop"

$scriptDir   = $PSScriptRoot
$repoRoot    = (Resolve-Path (Join-Path $scriptDir "..\..")).Path
$platformOS  = "windows"

$nativeArch = $env:PROCESSOR_ARCHITECTURE
if ($nativeArch -eq "AMD64") { $nativeArch = "x86_64" }
elseif ($nativeArch -eq "x86") { $nativeArch = "x86" }
elseif ($nativeArch -eq "ARM64") { $nativeArch = "aarch64" }

$defaultInstallDir = Join-Path $repoRoot "output\target\$platformOS\$Arch"
$installDir = if ($OutputDir) { $OutputDir } else { $defaultInstallDir }

Write-Host ""
Write-Host "========================================"
Write-Host "Building acktel (Windows)"
Write-Host "========================================"
Write-Host "Architecture : $Arch"
Write-Host "Native arch  : $nativeArch"
Write-Host "Source       : $scriptDir"
Write-Host "Install dir  : $installDir"
Write-Host ""

# ---------------------------------------------------------------------------
# Check Rust toolchain
# ---------------------------------------------------------------------------
$rustcVer = & rustc --version 2>$null
if (-not $?) {
    Write-Error "rustc not found. Install Rust: https://rustup.rs"
    exit 1
}
$cargoVer = & cargo --version 2>$null
if (-not $?) {
    Write-Error "cargo not found. Install Rust: https://rustup.rs"
    exit 1
}
Write-Host "rustc : $rustcVer"
Write-Host "cargo : $cargoVer"
Write-Host ""

# ---------------------------------------------------------------------------
# Determine Rust target triple
# ---------------------------------------------------------------------------
if (-not $TargetTriple) {
    $tripleMap = @{
        "x86_64"  = "x86_64-pc-windows-msvc"
        "x86"     = "i686-pc-windows-msvc"
        "aarch64" = "aarch64-pc-windows-msvc"
    }
    $TargetTriple = $tripleMap[$Arch]
}

$isCross = ($Arch -ne $nativeArch)

$isRelease = ($BuildType -ne "Debug")
$cargoArgs = @(
    "--manifest-path", (Join-Path $scriptDir "Cargo.toml")
)
if ($isRelease) {
    $cargoArgs += "--release"
}

if ($isCross) {
    Write-Host "Cross-compiling: $nativeArch -> $Arch ($TargetTriple)"
    $installedTargets = & rustup target list --installed 2>$null
    if ($installedTargets -notmatch [regex]::Escape($TargetTriple)) {
        Write-Host "Installing Rust target: $TargetTriple"
        & rustup target add $TargetTriple
        if (-not $?) { Write-Error "Failed to install target $TargetTriple"; exit 1 }
    }
    $cargoArgs += "--target"
    $cargoArgs += $TargetTriple
} else {
    Write-Host "Compiling native: $Arch"
}

# ---------------------------------------------------------------------------
# Clean
# ---------------------------------------------------------------------------
if ($Clean) {
    Write-Host "Cleaning..."
    & cargo clean --manifest-path (Join-Path $scriptDir "Cargo.toml")
}

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "Building $BuildType binary..."
& cargo build @cargoArgs
if (-not $?) { Write-Error "Build failed!"; exit 1 }

# ---------------------------------------------------------------------------
# Install binary + library
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "Installing to $installDir\bin\"

$profileDir = if ($isRelease) { "release" } else { "debug" }
if ($isCross) {
    $srcBinary = Join-Path $scriptDir "target\$TargetTriple\$profileDir\acktel.exe"
    $srcLib = Join-Path $scriptDir "target\$TargetTriple\$profileDir\libacktel.rlib"
} else {
    $srcBinary = Join-Path $scriptDir "target\$profileDir\acktel.exe"
    $srcLib = Join-Path $scriptDir "target\$profileDir\libacktel.rlib"
}

$binDir = Join-Path $installDir "bin"
New-Item -ItemType Directory -Force -Path $binDir | Out-Null
Copy-Item $srcBinary (Join-Path $binDir "acktel.exe") -Force
Write-Host "  -> $binDir\acktel.exe"

# Copy library for nshell integration
$libDir = Join-Path $installDir "lib"
New-Item -ItemType Directory -Force -Path $libDir | Out-Null
Copy-Item $srcLib (Join-Path $libDir "libacktel.rlib") -Force
Write-Host "  -> $libDir\libacktel.rlib"

Write-Host ""
Write-Host "Done!"
