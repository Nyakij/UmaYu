# Builds dist\umaLord-<date>.zip to share: the app, roster, game data and the (empty) notes
# and logs. Leaves out build output, the toolchain hardlinks in app\.cargo, and profiles\
# (saved accounts and backups). -IncludeProfile puts one profile in the zip's profiles\ folder,
# so whoever gets it can load it from the first-run setup with one click.
#   powershell -ExecutionPolicy Bypass -File tools\package.ps1 [-Build] [-IncludeProfile profiles\<name>.umalord.json]
param([switch]$Build, [string]$IncludeProfile)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent

if ($Build) {
    Push-Location (Join-Path $root 'app')
    cargo build --release
    $code = $LASTEXITCODE
    Pop-Location
    if ($code) { throw 'cargo build failed' }
}
$exe = Join-Path $root 'app\target\release\umalord.exe'
if (-not (Test-Path $exe)) { throw 'No release build. Run "cargo build --release" in app\, or pass -Build.' }

$dist = Join-Path $root 'dist'
$stage = Join-Path $dist 'umaLord'
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force $stage | Out-Null

Copy-Item $exe $stage
foreach ($f in 'README.txt', 'LICENSE-GPL-3.0.txt', 'CLAUDE.md', 'AGENTS.md', 'opencode.json', 'sources.md') { Copy-Item (Join-Path $root $f) $stage }
foreach ($d in 'knowledge', 'tools') { Copy-Item (Join-Path $root $d) (Join-Path $stage $d) -Recurse }
# Your account (account\, plans, results, insights.md) never goes in the zip: the app writes
# blank ones from app\templates on first run, and Setup fills them.
New-Item -ItemType Directory -Force (Join-Path $stage 'plans') | Out-Null
Copy-Item (Join-Path $root 'plans\_template.md') (Join-Path $stage 'plans')
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

$zip = Join-Path $dist "umaLord-$(Get-Date -Format yyyy-MM-dd).zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path $stage -DestinationPath $zip
'{0}  ({1:N1} MB, {2} files)' -f $zip, ((Get-Item $zip).Length / 1MB), (Get-ChildItem $stage -Recurse -File).Count
