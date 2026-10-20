[CmdletBinding()]
Param(
    [Parameter()][switch]$Install,
    [Parameter()][switch]$Help
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

if ($Help) {
    Write-Output "Build the Windows x64 release-size installer for Panda Note."
    Write-Output "Usage: script/build-panda-installer-windows.ps1 [-Install] [-Help]"
    exit 0
}

$workspace = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$target = 'x86_64-pc-windows-msvc'
$targetRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $workspace 'target' }
$exe = Join-Path $targetRoot "$target\release-size\panda.exe"
$outputDir = Join-Path $workspace 'dist\windows'
$iss = Join-Path $workspace 'crates\panda\resources\windows\panda.iss'
$manifest = Get-Content (Join-Path $workspace 'crates\panda\Cargo.toml') -Raw
if ($manifest -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
    throw 'Could not read Panda version from crates/panda/Cargo.toml.'
}
$version = $Matches[1]

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'cargo was not found on PATH.'
}

rustup target add $target
cargo build --locked -p panda --profile release-size --target $target
if (-not (Test-Path -LiteralPath $exe)) {
    throw "Panda executable was not produced at $exe"
}

$iscc = Get-Command ISCC.exe -ErrorAction SilentlyContinue
if (-not $iscc) {
    $candidates = @(
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
        'C:\Program Files (x86)\Inno Setup 6\ISCC.exe',
        'C:\Program Files\Inno Setup 6\ISCC.exe'
    )
    $isccPath = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if (-not $isccPath) {
        throw 'Inno Setup 6 is required. Install it or add ISCC.exe to PATH, then rerun this script.'
    }
} else {
    $isccPath = $iscc.Source
}

New-Item -Path $outputDir -ItemType Directory -Force | Out-Null
$env:PANDA_EXE = $exe
$env:PANDA_INSTALLER_OUTPUT = $outputDir
$env:PANDA_APP_VERSION = $version
& $isccPath $iss

$installer = Join-Path $outputDir "PandaNoteSetup-$version-windows-x64.exe"
if (-not (Test-Path -LiteralPath $installer)) {
    throw "Installer compilation did not produce $installer"
}
Write-Output "Installer created: $installer"

if ($Install) {
    Start-Process -FilePath $installer -Wait
}
