<#
.SYNOPSIS
  The README's art, in the style of the social preview: a hero banner, feature cards, the
  themes board, a "get started" banner, and the social preview itself.

.DESCRIPTION
  Composes docs\readme\*.png (and docs\social-preview.png) from app screenshots with WPF, so run
  it with powershell -STA. -Shots is a folder of 1400x900 screenshots named as in $need below.
  -Take takes them first with the friend's profile on a scratch copy of knowledge\ and
  app\assets\ in %TEMP%, so no real account file is read; ask.png needs a live model run, so
  take it yourself (UMALORD_ASK, see CLAUDE.md) or keep the one already in -Shots.

.EXAMPLE
  powershell -STA -ExecutionPolicy Bypass -File tools\readme_art.ps1 -Shots $env:TEMP\readme-shots -Take
#>
param(
    [Parameter(Mandatory = $true)][string]$Shots,
    [switch]$Take,
    [string]$ProfileFile = ''
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$out = Join-Path $repo 'docs\readme'
New-Item -ItemType Directory -Force $out, $Shots | Out-Null

# name = the UMALORD_* variables for it (1400x900, Seiun Sky dark unless it says otherwise).
$need = [ordered]@{
    'home'        = @{ TAB = 'home' }
    'plan'        = @{ TAB = 'planner'; SELECT = '100601'; RUN = '1' }
    'skills'      = @{ TAB = 'planner'; SELECT = '100601'; RUN = '1'; SCROLL = '1500' }
    'race'        = @{ TAB = 'sim'; SELECT = '100601'; RUN = '1'; RESULTS = 'race'; SCROLL = '430'; LOOK = 'night' }
    'collection'  = @{ TAB = 'cards'; SELECT = '30076'; THEME = 'light' }
    'setup'       = @{ SETUP = 'cards'; LOOK = 'sunset' }
    'theme-sky'   = @{ TAB = 'home' }
    'theme-sakura' = @{ TAB = 'home'; THEME = 'light'; LOOK = 'sakura' }
    'theme-sunset' = @{ TAB = 'home'; LOOK = 'sunset' }
    'theme-night' = @{ TAB = 'home'; LOOK = 'night' }
    'theme-track' = @{ TAB = 'home'; THEME = 'light'; LOOK = 'track' }
    'theme-ink'   = @{ TAB = 'home'; LOOK = 'ink' }
}

if ($Take) {
    $exe = Join-Path $repo 'app\target\release\umalord.exe'
    if (-not (Test-Path $exe)) { throw "Build the app first: $exe is missing." }
    if (-not $ProfileFile) {
        $ProfileFile = Get-ChildItem (Join-Path $repo 'profiles') -Filter 'friend-*.umalord.json' -ErrorAction SilentlyContinue | Sort-Object Name | Select-Object -Last 1 -ExpandProperty FullName
    }
    if (-not $ProfileFile) { throw 'No friend profile in profiles\: pass -ProfileFile.' }
    $root = Join-Path $env:TEMP 'umalord-readme-root'
    if (-not (Test-Path (Join-Path $root 'knowledge\data'))) {
        robocopy (Join-Path $repo 'knowledge') (Join-Path $root 'knowledge') /E /XD digest /NFL /NDL /NJH /NJS | Out-Null
        robocopy (Join-Path $repo 'app\assets') (Join-Path $root 'app\assets') /E /NFL /NDL /NJH /NJS | Out-Null
    }
    $vars = 'SCREENSHOT', 'TAB', 'VIEW', 'SELECT', 'RUN', 'SCROLL', 'SETUP', 'SETTINGS', 'SETTINGS_PAGE', 'HELP', 'ADD', 'PICKER',
        'WINDOW', 'THEME', 'LOOK', 'MASCOT', 'CHIBI', 'PROFILE', 'ASK', 'INSTALL', 'ROOT', 'LIVE', 'SHOP', 'EXPLAIN', 'COMPARE',
        'COURSE', 'RAIL', 'RESULTS', 'EXTRA', 'REPLAY', 'SCALE', 'DENSITY', 'HOVER', 'APPROVE', 'AGENT_DIR'
    foreach ($name in $need.Keys) {
        foreach ($v in $vars) { Remove-Item "Env:UMALORD_$v" -ErrorAction SilentlyContinue }
        $env:UMALORD_ROOT = $root
        $env:UMALORD_PROFILE = $ProfileFile
        $env:UMALORD_LIVE = 'cache'
        $env:UMALORD_WINDOW = '1400x900'
        $env:UMALORD_THEME = 'dark'
        $env:UMALORD_LOOK = 'sky'
        $env:UMALORD_MASCOT = '1'
        $env:UMALORD_CHIBI = 'home'
        if (-not $need[$name].ContainsKey('SETUP')) { $env:UMALORD_SETUP = 'off' }
        foreach ($kv in $need[$name].GetEnumerator()) { Set-Item "Env:UMALORD_$($kv.Key)" $kv.Value }
        $env:UMALORD_SCREENSHOT = Join-Path $Shots "$name.png"
        Start-Process -FilePath $exe -Wait -WorkingDirectory $root
    }
    foreach ($v in $vars) { Remove-Item "Env:UMALORD_$v" -ErrorAction SilentlyContinue }
}
foreach ($name in @($need.Keys) + 'ask') {
    if (-not (Test-Path (Join-Path $Shots "$name.png"))) { throw "Missing $name.png in $Shots." }
}

Add-Type -AssemblyName PresentationCore, PresentationFramework, WindowsBase

# --- Drawing kit -------------------------------------------------------------------------------

function C([string]$hex, [int]$a = 255) {
    $c = [System.Windows.Media.ColorConverter]::ConvertFromString($hex)
    [System.Windows.Media.Color]::FromArgb($a, $c.R, $c.G, $c.B)
}
function Fill($color) { $b = New-Object System.Windows.Media.SolidColorBrush $color; $b.Freeze(); $b }
function Pt([double]$x, [double]$y) { New-Object System.Windows.Point($x, $y) }
function Box([double]$x, [double]$y, [double]$w, [double]$h) { New-Object System.Windows.Rect($x, $y, $w, $h) }
function Pen($color, [double]$width) { $p = New-Object System.Windows.Media.Pen((Fill $color), $width); $p.Freeze(); $p }

function Picture([string]$path) {
    $b = New-Object System.Windows.Media.Imaging.BitmapImage
    $b.BeginInit()
    $b.CacheOption = [System.Windows.Media.Imaging.BitmapCacheOption]::OnLoad
    $b.UriSource = New-Object Uri((Resolve-Path $path).Path)
    $b.EndInit()
    $b.Freeze()
    $b
}
function Crop($bmp, [int]$x, [int]$y, [int]$w, [int]$h) {
    $c = New-Object System.Windows.Media.Imaging.CroppedBitmap($bmp, (New-Object System.Windows.Int32Rect($x, $y, $w, $h)))
    $c.Freeze()
    $c
}
function Shot([string]$name) { Picture (Join-Path $Shots "$name.png") }

function Words([string]$s, [double]$size, $color, [string]$weight = 'Normal', [double]$maxW = 0) {
    $tf = New-Object System.Windows.Media.Typeface((New-Object System.Windows.Media.FontFamily('Segoe UI')),
        [System.Windows.FontStyles]::Normal, [System.Windows.FontWeights]::$weight, [System.Windows.FontStretches]::Normal)
    $ft = New-Object System.Windows.Media.FormattedText($s, [Globalization.CultureInfo]::InvariantCulture,
        [System.Windows.FlowDirection]::LeftToRight, $tf, $size, (Fill $color), 1.0)
    if ($maxW -gt 0) { $ft.MaxTextWidth = $maxW }
    $ft
}
# Small caps-style kicker: letters spread with thin spaces.
function Spread([string]$s) { ($s.ToUpper().ToCharArray() | ForEach-Object { [string]$_ }) -join [char]0x2009 }

function New-Art([int]$w, [int]$h, [double]$radius = 0) {
    $root = New-Object System.Windows.Media.ContainerVisual
    if ($radius -gt 0) { $root.Clip = New-Object System.Windows.Media.RectangleGeometry((Box 0 0 $w $h), $radius, $radius) }
    @{ W = $w; H = $h; Root = $root }
}
function Layer($art, [scriptblock]$draw, $effect = $null, [double]$opacity = 1) {
    $v = New-Object System.Windows.Media.DrawingVisual
    [System.Windows.Media.RenderOptions]::SetBitmapScalingMode($v, [System.Windows.Media.BitmapScalingMode]::HighQuality)
    [System.Windows.Media.TextOptions]::SetTextFormattingMode($v, [System.Windows.Media.TextFormattingMode]::Ideal)
    $dc = $v.RenderOpen()
    & $draw $dc
    $dc.Close()
    if ($effect) { $v.Effect = $effect }
    $v.Opacity = $opacity
    [void]$art.Root.Children.Add($v)
}
function Save-Art($art, [string]$path) {
    $rtb = New-Object System.Windows.Media.Imaging.RenderTargetBitmap($art.W, $art.H, 96, 96, [System.Windows.Media.PixelFormats]::Pbgra32)
    $rtb.Render($art.Root)
    $enc = New-Object System.Windows.Media.Imaging.PngBitmapEncoder
    $enc.Frames.Add([System.Windows.Media.Imaging.BitmapFrame]::Create($rtb))
    $fs = [IO.File]::Create($path)
    $enc.Save($fs)
    $fs.Close()
    '{0,-24} {1,5:n0} KB' -f (Split-Path $path -Leaf), ((Get-Item $path).Length / 1KB)
}

# Seiun Sky's dark page: the page colour top left, the sky colour bottom right, soft clouds, and a
# glow in the art's accent colour behind where the screenshot goes.
$bg = '#111318'; $sky = '#1e3454'
$text = '#f1f5fb'; $soft = '#b4c2d8'; $dim = '#7d8aa0'
function Backdrop($art, [string]$accent, [double]$gx = 0.72, [double]$gy = 0.6) {
    $w = $art.W; $h = $art.H
    Layer $art {
        param($dc)
        $g = New-Object System.Windows.Media.LinearGradientBrush
        $g.StartPoint = Pt 0 0; $g.EndPoint = Pt 1 1
        $g.GradientStops.Add((New-Object System.Windows.Media.GradientStop((C $bg), 0)))
        $g.GradientStops.Add((New-Object System.Windows.Media.GradientStop((C '#151b27'), 0.45)))
        $g.GradientStops.Add((New-Object System.Windows.Media.GradientStop((C $sky), 1)))
        $dc.DrawRectangle($g, $null, (Box 0 0 $w $h))
        $r = New-Object System.Windows.Media.RadialGradientBrush
        $r.Center = Pt $gx $gy; $r.GradientOrigin = Pt $gx $gy; $r.RadiusX = 0.55; $r.RadiusY = 0.75
        $r.GradientStops.Add((New-Object System.Windows.Media.GradientStop((C $accent 70), 0)))
        $r.GradientStops.Add((New-Object System.Windows.Media.GradientStop((C $accent 0), 1)))
        $dc.DrawRectangle($r, $null, (Box 0 0 $w $h))
        # The app's cloud cluster, bottom right, and smaller ones top right and bottom left.
        $cloud = Fill (C '#ffffff' 11)
        $size = $h * 0.46
        $base = Pt ($w - $size * 0.5) ($h - 10)
        foreach ($c in @(@(-0.95, -0.12, 0.30), @(-0.62, -0.22, 0.38), @(-0.25, -0.12, 0.30))) {
            $dc.DrawEllipse($cloud, $null, (Pt ($base.X + $c[0] * $size) ($base.Y + $c[1] * $size)), $c[2] * $size, $c[2] * $size)
        }
        $small = $h * 0.22
        foreach ($c in @(@(-0.5, 0.35, 0.34), @(-0.12, 0.22, 0.46), @(0.3, 0.38, 0.34))) {
            $dc.DrawEllipse($cloud, $null, (Pt ($w * 0.8 + $c[0] * $small) ($c[1] * $small)), $c[2] * $small, $c[2] * $small)
        }
        foreach ($c in @(@(0.1, -0.1, 0.42), @(0.55, -0.02, 0.32))) {
            $dc.DrawEllipse($cloud, $null, (Pt ($c[0] * $size) ($h + $c[1] * $size)), $c[2] * $size, $c[2] * $size)
        }
    }
}

# A screenshot in a rounded window frame with a glow in the accent colour and a soft shadow.
function Window($art, $img, [double]$x, [double]$y, [double]$w, [double]$h, [string]$accent, [double]$opacity = 1) {
    $frame = Box $x $y $w $h
    $glow = New-Object System.Windows.Media.Effects.BlurEffect
    $glow.Radius = 46
    Layer $art { param($dc) $dc.DrawRoundedRectangle((Fill (C $accent 120)), $null, (Box ($x + 10) ($y + 18) ($w - 20) ($h - 10)), 18, 18) } $glow $opacity
    $shade = New-Object System.Windows.Media.Effects.BlurEffect
    $shade.Radius = 30
    Layer $art { param($dc) $dc.DrawRoundedRectangle((Fill (C '#000000' 150)), $null, (Box ($x + 6) ($y + 22) $w $h), 16, 16) } $shade $opacity
    Layer $art {
        param($dc)
        $dc.PushClip((New-Object System.Windows.Media.RectangleGeometry($frame, 14, 14)))
        $dc.DrawImage($img, $frame)
        $dc.Pop()
        $dc.DrawRoundedRectangle($null, (Pen (C $accent 170) 2), $frame, 14, 14)
    } $null $opacity
}

function Pill($dc, [string]$s, [double]$x, [double]$y, [string]$accent) {
    $t = Words $s 19 (C '#dbe7f7') 'SemiBold'
    $w = $t.Width + 36
    $dc.DrawRoundedRectangle((Fill (C $accent 30)), (Pen (C $accent 110) 1.5), (Box $x $y $w 42), 21, 21)
    $dc.DrawText($t, (Pt ($x + 18) ($y + 21 - $t.Height / 2)))
    $w
}

$chibi = Picture (Join-Path $repo 'app\assets\chibi\seiun_102001_0011.png')
$blue = '#60a5fa'; $pink = '#f48fb1'; $orange = '#fb9d4b'; $violet = '#a78bfa'; $gold = '#eab308'; $cyan = '#7dd3fc'

# --- Hero ---------------------------------------------------------------------------------------

$hero = New-Art 1600 760 28
Backdrop $hero $blue 0.74 0.5
$race = Shot 'race'
Window $hero (Crop $race 318 205 1080 690) 930 64 620 396 $violet 0.9
$homeShot = Shot 'home'
Window $hero (Crop $homeShot 0 0 1300 836) 700 196 800 514 $blue
Layer $hero {
    param($dc)
    $dc.DrawImage($chibi, (Box 70 40 210 210))
    $dc.DrawText((Words 'umaLord' 128 (C $text) 'SemiBold'), (Pt 74 236))
    $dc.DrawText((Words 'Your Uma Musume (Global) companion. Plan careers on the community meta, race them in a real simulator, and keep your whole collection at hand.' 29 (C $soft) 'Normal' 580), (Pt 82 420))
    $x = 82
    foreach ($p in 'Free and open source', 'Windows 10 and 11', 'Runs offline') { $x += (Pill $dc $p $x 600 $blue) + 12 }
}
Save-Art $hero (Join-Path $out 'hero.png')

# --- Feature cards ------------------------------------------------------------------------------

function Card([string]$file, [string]$kicker, [string]$title, [string]$line, $img, [string]$accent) {
    $art = New-Art 1000 640 28
    Backdrop $art $accent 0.8 0.85
    Layer $art {
        param($dc)
        $dc.DrawText((Words (Spread $kicker) 21 (C $accent) 'SemiBold'), (Pt 56 46))
        $dc.DrawText((Words $title 46 (C $text) 'SemiBold' 890), (Pt 54 76))
        $dc.DrawText((Words $line 25 (C $soft) 'Normal' 860), (Pt 56 142))
    }
    $w = 1000
    $h = $w * $img.PixelHeight / $img.PixelWidth
    Window $art $img 56 240 $w $h $accent
    Save-Art $art (Join-Path $out $file)
}

Card 'home.png' '01 · Home' 'The next Champions Meeting, at a glance' 'Live from the game''s news: the course drawn, what the meta recommends, and your best umas for it.' (Crop $homeShot 0 52 1260 700) $blue
Card 'plan.png' '02 · Plan' 'Careers built on the community meta' 'Pick a CM and a trainee. Decks start from the PvP meta sheet; simulated careers rank the rest.' (Crop (Shot 'plan') 240 112 1160 640) $cyan
Card 'autopick.png' '03 · Auto-pick' 'Skills that win together' 'Races every skill on offer, keeps the set that gains the most, and checks it in 1,000 races.' (Crop (Shot 'skills') 240 128 1160 600) $blue
Card 'race.png' '04 · Race sim' 'Moomoolator''s engine, in Rust' 'Race two builds 1,000 times: where the lengths are won, speed and HP, what each skill is worth.' (Crop $race 300 190 1100 660) $violet
Card 'collection.png' '05 · Collection' 'Everything you own, at your level' 'Effects at your limit break, the skills each card teaches, and every skill that can trigger on a course.' (Crop (Shot 'collection') 240 136 1160 640) $cyan
Card 'ask.png' '06 · Ask' 'An assistant that works the app' 'Ask in plain English: it plans, picks and races a career, and asks before it saves. Free models built in.' (Crop (Shot 'ask') 0 52 1100 640) $pink

# --- Themes -------------------------------------------------------------------------------------

$themes = New-Art 1600 980 28
Backdrop $themes $violet 0.5 0.55
$looks = @(
    @('sky', 'Seiun Sky', 'dark', $blue), @('sakura', 'Sakura', 'light', $pink), @('sunset', 'Sunset', 'dark', $orange),
    @('night', 'Starry night', 'dark', $violet), @('track', 'Track day', 'light', $gold), @('ink', 'Ink', 'high contrast', '#eceef2')
)
Layer $themes {
    param($dc)
    $k = Words (Spread 'Six themes · dark or light') 21 (C $blue) 'SemiBold'
    $dc.DrawText($k, (Pt ((1600 - $k.Width) / 2) 54))
    $t = Words 'Make it yours' 56 (C $text) 'SemiBold'
    $dc.DrawText($t, (Pt ((1600 - $t.Width) / 2) 86))
    $s = Words 'Follow Windows or pick a mode, and give the sky to Seiun Sky, Sakura, Sunset, Starry night, Track day or Ink.' 25 (C $soft)
    $dc.DrawText($s, (Pt ((1600 - $s.Width) / 2) 166))
}
for ($i = 0; $i -lt 6; $i++) {
    $l = $looks[$i]
    $x = 70 + ($i % 3) * 500
    $y = 250 + [math]::Floor($i / 3) * 360
    Window $themes (Crop (Shot "theme-$($l[0])") 0 0 1400 900) $x $y 460 296 $l[3]
    Layer $themes {
        param($dc)
        $dc.DrawEllipse((Fill (C $l[3])), $null, (Pt ($x + 8) ($y + 324)), 7, 7)
        $n = Words $l[1] 24 (C $text) 'SemiBold'
        $dc.DrawText($n, (Pt ($x + 24) ($y + 324 - $n.Height / 2)))
        $m = Words $l[2] 20 (C $dim)
        $dc.DrawText($m, (Pt ($x + 32 + $n.Width) ($y + 324 - $m.Height / 2)))
    }
}
Save-Art $themes (Join-Path $out 'themes.png')

# --- Get started --------------------------------------------------------------------------------

$start = New-Art 1600 640 28
Backdrop $start $orange 0.78 0.55
Window $start (Crop (Shot 'setup') 140 52 1120 720) 820 70 720 463 $orange
Layer $start {
    param($dc)
    $dc.DrawText((Words (Spread 'Get started') 21 (C $orange) 'SemiBold'), (Pt 70 70))
    $dc.DrawText((Words 'Up and running in a minute' 50 (C $text) 'SemiBold' 680), (Pt 68 100))
    $steps = @(
        @('Download', 'Grab the zip from Releases and unzip it anywhere.'),
        @('Run umalord.exe', 'Windows may ask first: More info, then Run anyway.'),
        @('Add your account', 'Load a profile file, or click the umas and cards you own.')
    )
    for ($i = 0; $i -lt 3; $i++) {
        $y = 232 + $i * 116
        $dc.DrawEllipse((Fill (C $orange 40)), (Pen (C $orange 200) 2), (Pt 96 ($y + 30)), 26, 26)
        $n = Words ([string]($i + 1)) 26 (C $orange) 'SemiBold'
        $dc.DrawText($n, (Pt (96 - $n.Width / 2) ($y + 30 - $n.Height / 2)))
        $dc.DrawText((Words $steps[$i][0] 28 (C $text) 'SemiBold'), (Pt 144 ($y + 2)))
        $dc.DrawText((Words $steps[$i][1] 22 (C $soft) 'Normal' 560), (Pt 144 ($y + 42)))
    }
}
Save-Art $start (Join-Path $out 'start.png')

# --- Social preview (GitHub's link card, 1280x640; upload it by hand in Settings) ----------------

$social = New-Art 1280 640
Backdrop $social $blue 0.72 0.5
Window $social (Crop (Shot 'plan') 0 0 1400 900) 560 120 680 437 $blue
Layer $social {
    param($dc)
    $dc.DrawImage($chibi, (Box 70 50 190 190))
    $dc.DrawText((Words 'umaLord' 100 (C $text) 'SemiBold'), (Pt 58 250))
    $dc.DrawText((Words 'Your Uma Musume (Global) account: collection, career plans built on the community meta, and a race simulator. All offline, on Windows.' 27 (C $soft) 'Normal' 440), (Pt 62 386))
    $dc.DrawText((Words 'github.com/brickc4/umaLord' 24 (C $blue) 'SemiBold'), (Pt 62 560))
}
Save-Art $social (Join-Path $repo 'docs\social-preview.png')
