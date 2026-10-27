# Builds dist\UmaYu-<date>.zip to share: the app, roster, game data and the (empty) notes
# and logs. Leaves out build output, the toolchain hardlinks in app\.cargo, and profiles\
# (saved accounts and backups). -IncludeProfile puts one profile in the zip's profiles\ folder,
# so whoever gets it can load it from the first-run setup with one click.
# -Version v0.1.1 names the zip UmaYu-v0.1.1.zip (the release workflow passes the tag);
# without it the zip is named by date.
#   powershell -ExecutionPolicy Bypass -File tools\package.ps1 [-Build] [-Version v0.1.1] [-IncludeProfile profiles\<name>.umalord.json]
param([switch]$Build, [string]$Version, [string]$IncludeProfile)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent

if ($Build) {
    Push-Location (Join-Path $root 'app')
    cargo build --release
    $code = $LASTEXITCODE
    Pop-Location
    if ($code) { throw 'cargo build failed' }
}
$exe = Join-Path $root 'app\target\release\umayu.exe'
if (-not (Test-Path $exe)) { throw 'No release build. Run "cargo build --release" in app\, or pass -Build.' }

$dist = Join-Path $root 'dist'
$stage = Join-Path $dist 'UmaYu'
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force $stage | Out-Null

Copy-Item $exe $stage
foreach ($f in 'LICENSE', 'CLAUDE.md', 'AGENTS.md', 'opencode.json', 'sources.md') { Copy-Item (Join-Path $root $f) $stage }
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
$zip = Join-Path $dist "UmaYu-$label.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path $stage -DestinationPath $zip
'{0}  ({1:N1} MB, {2} files)' -f $zip, ((Get-Item $zip).Length / 1MB), (Get-ChildItem $stage -Recurse -File).Count
