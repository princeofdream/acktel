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
# Prefer a rustup-managed toolchain (Windows)
# ---------------------------------------------------------------------------
# A standalone/chocolatey cargo on PATH may point at a different toolchain than
# the one `rustup target add` populates, causing E0463 "can't find crate for
# core/std" for the cross target. If rustup is available, resolve its cargo and
# remember its directory so it is placed first on PATH below (same sysroot that
# `rustup target add` uses).
# Locate rustup (even if it is not on PATH) and resolve its cargo directory so it
# can be placed first on PATH below. A standalone/chocolatey cargo on PATH may
# point at a different toolchain than the one `rustup target add` populates,
# causing E0463 "can't find crate for core/std" for the cross target.
$rustupExePath = $null
$rustupCmd = Get-Command rustup.exe -ErrorAction SilentlyContinue
if ($rustupCmd) {
    $rustupExePath = $rustupCmd.Source
} else {
    $rustupCandidates = @()
    if ($env:USERPROFILE) { $rustupCandidates += Join-Path $env:USERPROFILE ".cargo\bin\rustup.exe" }
    if ($env:CARGO_HOME)  { $rustupCandidates += Join-Path $env:CARGO_HOME "bin\rustup.exe" }
    foreach ($c in $rustupCandidates) {
        if (Test-Path $c) { $rustupExePath = $c; break }
    }
}

$rustupBin = $null
if ($rustupExePath) {
    $rustupCargo = & $rustupExePath which cargo 2>$null
    if ($rustupCargo -and (Test-Path $rustupCargo)) {
        $rustupBin = Split-Path $rustupCargo
    }
}

# ---------------------------------------------------------------------------
# Set up MSVC environment for the target architecture (Windows)
# ---------------------------------------------------------------------------
# Rust MSVC targets (e.g. aarch64-pc-windows-msvc) link with the MSVC `link.exe`.
# For cross-compilation the target-arch linker must be on PATH, so initialize the
# matching VS environment — same approach as the C/C++ module scripts.
if ($env:OS -eq "Windows_NT") {
    function Get-VSPath {
        $vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
        if (Test-Path $vsWhere) {
            $allVS = @(& $vsWhere -property installationPath 2>$null) | Where-Object { $_ }
            foreach ($vsPath in $allVS) {
                if ($vsPath -and (Test-Path (Join-Path $vsPath "VC\Auxiliary\Build\vcvarsall.bat"))) {
                    return $vsPath
                }
            }
        }
        $fallbacks = @(
            "D:\Programs\MicrosoftVisualStudio\18\Community",
            "C:\Program Files\Microsoft Visual Studio\2022\Community",
            "C:\Program Files\Microsoft Visual Studio\2022\Professional",
            "C:\Program Files\Microsoft Visual Studio\2022\Enterprise"
        )
        foreach ($f in $fallbacks) {
            if (Test-Path (Join-Path $f "VC\Auxiliary\Build\vcvarsall.bat")) { return $f }
        }
        return $null
    }

    $vcArchMap       = @{ "x86_64" = "x64"; "x86" = "x86"; "aarch64" = "x64_arm64" }
    $vcHostArchMap   = @{ "x86_64" = "x64"; "x86" = "x86"; "aarch64" = "x64" }
    $vcTargetArchMap = @{ "x86_64" = "x64"; "x86" = "x86"; "aarch64" = "arm64" }
    $vcArch       = $vcArchMap[$Arch]
    $vcHostArch   = $vcHostArchMap[$nativeArch]
    $vcTargetArch = $vcTargetArchMap[$Arch]

    $vsPath = Get-VSPath
    if ($vsPath) {
        $vcvarsall = Join-Path $vsPath "VC\Auxiliary\Build\vcvarsall.bat"
        $msvcPath  = Join-Path $vsPath "VC\Tools\MSVC"

        # Pick the newest MSVC toolset that actually ships the required target
        # linker (bin\Host{host}\{target}\link.exe). Newer toolsets may be
        # installed without the ARM64/x64 build tools, yielding a missing/fallback
        # link.exe (which would link the wrong architecture).
        $targetLink  = "bin\Host$vcHostArch\$vcTargetArch\link.exe"
        $msvcVersion = Get-ChildItem $msvcPath -Directory -ErrorAction SilentlyContinue |
            Sort-Object Name -Descending |
            Where-Object { Test-Path (Join-Path $_.FullName $targetLink) } |
            Select-Object -First 1

        if (-not $msvcVersion) {
            Write-Warning "No MSVC toolset with '$targetLink' found; relying on vcvarsall default for linking."
        }

        $vcTargetBin = if ($msvcVersion) {
            Join-Path $msvcVersion.FullName "bin\Host$vcHostArch\$vcTargetArch"
        } else { $null }

        $vcArgs = $vcArch
        if ($msvcVersion) { $vcArgs += " -vcvars_ver=$($msvcVersion.Name)" }

        $envFile    = "$env:TEMP\acktel_vcvars_$PID.env"
        $setupScript = @"
@echo off
call "$vcvarsall" $vcArgs
(set PATH)> "$envFile"
(set INCLUDE)>> "$envFile"
(set LIB)>> "$envFile"
"@
        $setupScript | Out-File -FilePath "$env:TEMP\acktel_setup_$PID.bat" -Encoding ASCII
        & cmd.exe /c "$env:TEMP\acktel_setup_$PID.bat"
        Remove-Item "$env:TEMP\acktel_setup_$PID.bat" -ErrorAction SilentlyContinue

        $afterVcvars = $null
        $afterInclude = $null
        $afterLib = $null
        if (Test-Path $envFile) {
            Get-Content $envFile | ForEach-Object {
                if ($_ -match '^PATH=(.*)')    { $script:afterVcvars  = $matches[1] }
                if ($_ -match '^INCLUDE=(.*)') { $script:afterInclude = $matches[1] }
                if ($_ -match '^LIB=(.*)')      { $script:afterLib     = $matches[1] }
            }
            Remove-Item $envFile -ErrorAction SilentlyContinue
        }

        if ($afterVcvars) {
            # rustup's bin first (consistent cargo/rustc sysroot), then the target
            # linker bin, then vcvarsall's PATH.
            $finalPath = @($rustupBin, $vcTargetBin, $afterVcvars) | Where-Object { $_ }
            $env:PATH = $finalPath -join ';'
            if ($afterInclude) { $env:INCLUDE = $afterInclude }
            if ($afterLib)     { $env:LIB     = $afterLib }
        } else {
            Write-Warning "vcvarsall did not produce a PATH; MSVC linker for $Arch may be missing."
        }
    } else {
        Write-Warning "Visual Studio not found; the MSVC linker for $Arch must already be on PATH."
    }
}

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
    if (-not $rustupExePath) {
        Write-Error "rustup not found; cannot install/verify cross target '$TargetTriple'. Install Rust via https://rustup.rs so rustup is available."
        exit 1
    }
    $installedTargets = & $rustupExePath target list --installed 2>$null
    if ($installedTargets -notmatch [regex]::Escape($TargetTriple)) {
        Write-Host "Installing Rust target: $TargetTriple"
        & $rustupExePath target add $TargetTriple
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
