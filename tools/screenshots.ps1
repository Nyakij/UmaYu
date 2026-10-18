<#
.SYNOPSIS
  Screenshots of every page of the app, in each theme and window size, for checking the UI.

.DESCRIPTION
  Runs app\target\release\umalord.exe once per page x theme x size with the UMALORD_* screenshot
  variables (see CLAUDE.md) and saves <page>-<theme>-<size>.png into -Out, which must be outside
  the repo. Pages that need a roster show the friend's profile in memory (UMALORD_PROFILE), so
  your own account files are never read into a screenshot or changed.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File tools\screenshots.ps1 -Out $env:TEMP\shots
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File tools\screenshots.ps1 -Out $env:TEMP\shots -Pages home,umas-table -Themes dark -Sizes 960x600
#>
param(
    [Parameter(Mandatory = $true)][string]$Out,
    # Page names from the list below; empty means all of them.
    [string[]]$Pages = @(),
    # mode-look pairs: dark or light, then sky, sakura, sunset or night ("dark" alone means dark-sky).
    [string[]]$Themes = @('dark-sky', 'light-sky', 'light-sakura'),
    [string[]]$Sizes = @('1400x900', '960x600'),
    # How many copies of the app run at once.
    [int]$Parallel = 4,
    [string]$Exe = '',
    [string]$ProfileFile = '',
    # Where the mascot goes: everywhere, home or off. Empty uses your saved setting.
    [string]$Chibi = '',
    # The live CM schedule: cache (what was fetched last, no network), fetch, or off (bundled data).
    [string]$Live = 'cache'
)

$ErrorActionPreference = 'Stop'
# `powershell -File` passes "a,b" as one string: split lists on commas either way.
$split = { param($list) @($list | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ }) }
$Pages = & $split $Pages
$Themes = & $split $Themes
$Sizes = & $split $Sizes
$repo = Split-Path -Parent $PSScriptRoot
if (-not $Exe) { $Exe = Join-Path $repo 'app\target\release\umalord.exe' }
if (-not (Test-Path $Exe)) { throw "Build the app first (cargo build --release in app\): $Exe is missing." }

$outFull = [System.IO.Path]::GetFullPath($Out)
$repoFull = [System.IO.Path]::GetFullPath($repo).TrimEnd('\') + '\'
if ($outFull.StartsWith($repoFull, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Save screenshots outside the repo (it's public): $outFull is inside $repoFull."
}
New-Item -ItemType Directory -Force $outFull | Out-Null

if (-not $ProfileFile) {
    $friend = Get-ChildItem (Join-Path $repo 'profiles') -Filter 'friend-*.umalord.json' -ErrorAction SilentlyContinue | Sort-Object Name | Select-Object -Last 1
    if ($friend) { $ProfileFile = $friend.FullName }
}
if (-not $ProfileFile) { Write-Warning 'No friend profile in profiles\; pages show whatever account\ holds.' }

# Each page is a set of UMALORD_* variables (without the prefix).
$all = [ordered]@{
    'setup-welcome'  = @{ SETUP = 'welcome' }
    'setup-import'   = @{ SETUP = 'import' }
    'setup-umas'     = @{ SETUP = 'umas' }
    'setup-cards'    = @{ SETUP = 'cards' }
    'setup-done'     = @{ SETUP = 'done' }
    'home'           = @{ TAB = 'home' }
    'umas-grid'      = @{ TAB = 'umas' }
    'umas-table'     = @{ TAB = 'umas'; VIEW = 'table' }
    'uma-detail'     = @{ TAB = 'umas'; SELECT = '100601' }
    'cards-grid'     = @{ TAB = 'cards' }
    'cards-table'    = @{ TAB = 'cards'; VIEW = 'table' }
    'card-detail'    = @{ TAB = 'cards'; SELECT = '30076' }
    'cards-add'      = @{ ADD = '30076' }
    'skills'         = @{ TAB = 'skills' }
    'skill-detail'   = @{ TAB = 'skills'; SELECT = '200512' }
    'planner-empty'  = @{ TAB = 'planner' }
    'planner-demo'   = @{ TAB = 'planner'; SELECT = '100601' }
    'planner-skills' = @{ TAB = 'planner'; SELECT = '100601'; RUN = '1'; SCROLL = '1500' }
    'sim-empty'      = @{ TAB = 'sim' }
    'sim-run'        = @{ TAB = 'sim'; SELECT = '100601'; RUN = '1' }
    'results'        = @{ TAB = 'results' }
    'ask'            = @{ TAB = 'ask' }
    'notes'          = @{ TAB = 'notes' }
    'settings'       = @{ SETTINGS = '1'; SETTINGS_PAGE = 'profile' }
    'settings-look'  = @{ SETTINGS = '1'; SETTINGS_PAGE = 'appearance' }
    'settings-data'  = @{ SETTINGS = '1'; SETTINGS_PAGE = 'data' }
    'settings-ai'    = @{ SETTINGS = '1'; SETTINGS_PAGE = 'ai' }
    'settings-about' = @{ SETTINGS = '1'; SETTINGS_PAGE = 'about' }
    'help'           = @{ HELP = '1' }
}
$names = if ($Pages.Count) { $Pages } else { @($all.Keys) }
foreach ($n in $names) { if (-not $all.Contains($n)) { throw "Unknown page '$n'. Pages: $($all.Keys -join ', ')" } }

$vars = 'SCREENSHOT', 'TAB', 'VIEW', 'SELECT', 'RUN', 'SCROLL', 'SETUP', 'SETTINGS', 'SETTINGS_PAGE', 'HELP', 'ADD', 'PICKER',
    'WINDOW', 'THEME', 'LOOK', 'MASCOT', 'CHIBI', 'PROFILE', 'ASK', 'INSTALL', 'ROOT', 'LIVE'
$running = New-Object System.Collections.ArrayList
$count = 0
foreach ($name in $names) {
    foreach ($theme in $Themes) {
        $mode, $look = $theme.Split('-', 2)
        if (-not $look) { $look = 'sky' }
        foreach ($size in $Sizes) {
            foreach ($v in $vars) { Remove-Item "Env:UMALORD_$v" -ErrorAction SilentlyContinue }
            $file = Join-Path $outFull "$name-$mode-$look-$size.png"
            $env:UMALORD_SCREENSHOT = $file
            $env:UMALORD_WINDOW = $size
            $env:UMALORD_THEME = $mode
            $env:UMALORD_LOOK = $look
            $env:UMALORD_MASCOT = '1'
            if ($Chibi) { $env:UMALORD_CHIBI = $Chibi }
            $env:UMALORD_LIVE = $Live
            if ($ProfileFile) { $env:UMALORD_PROFILE = $ProfileFile }
            foreach ($kv in $all[$name].GetEnumerator()) { Set-Item "Env:UMALORD_$($kv.Key)" $kv.Value }
            # Setup pages show over an empty roster unless a step is named; others keep it closed.
            if (-not $all[$name].ContainsKey('SETUP')) { $env:UMALORD_SETUP = 'off' }
            while ($running.Count -ge $Parallel) {
                $done = @($running | Where-Object { $_.HasExited })
                foreach ($d in $done) { $running.Remove($d) }
                if ($running.Count -ge $Parallel) { Start-Sleep -Milliseconds 200 }
            }
            [void]$running.Add((Start-Process -FilePath $Exe -WorkingDirectory $repo -PassThru))
            $count++
        }
    }
}
foreach ($v in $vars) { Remove-Item "Env:UMALORD_$v" -ErrorAction SilentlyContinue }
$running | Wait-Process -ErrorAction SilentlyContinue
$saved = @(Get-ChildItem $outFull -Filter '*.png').Count
Write-Output "Took $count screenshots into $outFull ($saved PNG files there)."
