# Builds dist\UmaYu-<date>.zip or dist\UmaYu-<date>-Setup.exe to share: the app, roster, game data
# and the (empty) notes and logs. Leaves out build output, the toolchain hardlinks in app\.cargo,
# and profiles\ (saved accounts and backups).
# -Format Zip (default): generates standalone portable .zip archive.
# -Format Setup: calls Inno Setup (ISCC.exe) to compile per-user Windows installer .exe.
# -IncludeProfile puts one profile in the zip's profiles\ folder (Zip format only).
# -Version v0.1.1 names the artifact UmaYu-v0.1.1.zip / UmaYu-v0.1.1-Setup.exe;
# without it the artifact is named by date.
#   powershell -ExecutionPolicy Bypass -File tools\package.ps1 [-Build] [-Version v0.1.1] [-Format Zip|Setup] [-IncludeProfile profiles\<name>.umalord.json]
param(
    [switch]$Build,
    [string]$Version,
    [string]$IncludeProfile,
    [ValidateSet('Zip', 'Setup')][string]$Format = 'Zip'
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent

if ($Format -eq 'Setup' -and $IncludeProfile) {
    throw 'Setup installer format does not support -IncludeProfile to prevent bundling personal user data.'
}

if ($Build) {
    Push-Location (Join-Path $root 'app')
    cargo build --release --locked
    $code = $LASTEXITCODE
    Pop-Location
    if ($code) { throw 'cargo build failed' }
}
$exe = Join-Path $root 'app\target\release\umayu.exe'
if (-not (Test-Path $exe)) { throw 'No release build. Run "cargo build --release" in app\, or pass -Build.' }

$requiredInputs = @(
    (Join-Path $root 'LICENSE'),
    (Join-Path $root 'sources.md'),
    (Join-Path $root 'UmaYu-reset.bat'),
    (Join-Path $root 'docs\guide.txt'),
    (Join-Path $root 'knowledge'),
    (Join-Path $root 'tools'),
    (Join-Path $root 'app\Cargo.toml'),
    (Join-Path $root 'app\Cargo.lock'),
    (Join-Path $root 'app\setup-toolchain.ps1'),
    (Join-Path $root 'app\src'),
    (Join-Path $root 'app\assets'),
    (Join-Path $root 'app\templates')
)
foreach ($req in $requiredInputs) {
    if (-not (Test-Path $req)) {
        throw "Required staging input not found: $req"
    }
}

$dist = Join-Path $root 'dist'
if (-not (Test-Path $dist)) { New-Item -ItemType Directory -Force $dist | Out-Null }
$stage = Join-Path $dist 'UmaYu'
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force $stage | Out-Null

Copy-Item $exe $stage
foreach ($f in 'LICENSE', 'sources.md', 'UmaYu-reset.bat') { Copy-Item (Join-Path $root $f) $stage }
# The user guide sits at the zip's root as README.txt, where people look first.
Copy-Item (Join-Path $root 'docs\guide.txt') (Join-Path $stage 'README.txt')
foreach ($d in 'knowledge', 'tools') { Copy-Item (Join-Path $root $d) (Join-Path $stage $d) -Recurse }
# Your account (account\, plans, results, insights.md) never goes in the zip: the app writes
# blank ones from app\templates on first run, and Setup fills them.
New-Item -ItemType Directory -Force (Join-Path $stage 'app') | Out-Null
foreach ($d in 'src', 'assets', 'templates', 'tests') { Copy-Item (Join-Path $root "app\$d") (Join-Path $stage "app\$d") -Recurse }
foreach ($f in 'Cargo.toml', 'Cargo.lock', 'setup-toolchain.ps1') { Copy-Item (Join-Path $root "app\$f") (Join-Path $stage 'app') }
Get-ChildItem $stage -Recurse -Directory -Filter '__pycache__' | Remove-Item -Recurse -Force
# OpenCode (~170 MB) isn't shipped: the Ask tab downloads the pinned release on first use.
Remove-Item (Join-Path $stage 'tools\opencode') -Recurse -Force -ErrorAction SilentlyContinue
# The roster digest is regenerated when the app starts.
Remove-Item (Join-Path $stage 'knowledge\digest') -Recurse -Force -ErrorAction SilentlyContinue
if ($IncludeProfile) {
    $src = Resolve-Path (Join-Path $root $IncludeProfile) -ErrorAction SilentlyContinue
    if (-not $src) { $src = Resolve-Path $IncludeProfile }
    New-Item -ItemType Directory -Force (Join-Path $stage 'profiles') | Out-Null
    Copy-Item $src (Join-Path $stage 'profiles')
}

$label = if ($Version) { $Version } else { Get-Date -Format yyyy-MM-dd }

if ($Format -eq 'Zip') {
    $zip = Join-Path $dist "UmaYu-$label.zip"
    if (Test-Path $zip) { Remove-Item $zip -Force }
    Compress-Archive -Path $stage -DestinationPath $zip
    '{0}  ({1:N1} MB, {2} files)' -f $zip, ((Get-Item $zip).Length / 1MB), (Get-ChildItem $stage -Recurse -File).Count
} elseif ($Format -eq 'Setup') {
    $iscc = $null
    if ($env:ISCC_PATH -and (Test-Path $env:ISCC_PATH)) {
        $iscc = $env:ISCC_PATH
    } else {
        $cmd = Get-Command iscc.exe -ErrorAction SilentlyContinue
        if ($cmd) {
            $iscc = $cmd.Source
        } else {
            $candidates = @(
                (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
                (Join-Path ${env:ProgramFiles} 'Inno Setup 6\ISCC.exe'),
                (Join-Path ${env:LOCALAPPDATA} 'Programs\Inno Setup 6\ISCC.exe'),
                (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 5\ISCC.exe'),
                (Join-Path ${env:ProgramFiles} 'Inno Setup 5\ISCC.exe'),
                (Join-Path ${env:LOCALAPPDATA} 'Programs\Inno Setup 5\ISCC.exe')
            )
            foreach ($c in $candidates) {
                if ($c -and (Test-Path $c)) {
                    $iscc = $c
                    break
                }
            }
        }
    }
    if (-not $iscc) {
        throw 'Inno Setup Compiler (ISCC.exe) not found. To build setup.exe, install Inno Setup 6 (https://jrsoftware.org/isdl.php) and ensure ISCC.exe is in your PATH or set the ISCC_PATH environment variable.'
    }

    $issFile = Join-Path $root 'UmaYu-setup.iss'
    if (-not (Test-Path $issFile)) {
        throw "Inno Setup definition file not found: $issFile"
    }

    $setupBase = "UmaYu-$label-Setup"
    $setupExe = Join-Path $dist "$setupBase.exe"
    if (Test-Path $setupExe) { Remove-Item $setupExe -Force }

    $isccArgs = @(
        "/O$dist",
        "/F$setupBase",
        "/DMyAppVersion=$label",
        "/DSourceDir=$stage",
        $issFile
    )
    & $iscc $isccArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Inno Setup compilation failed with exit code $LASTEXITCODE."
    }
    if (-not (Test-Path $setupExe)) {
        throw "Setup executable was not created at expected path: $setupExe"
    }
    '{0}  ({1:N1} MB)' -f $setupExe, ((Get-Item $setupExe).Length / 1MB)
}
