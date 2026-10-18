param([string]$Dir, [string]$Tsv, [string]$OutDir, [int]$Cols = 7, [int]$W = 150)
Add-Type -AssemblyName PresentationCore, PresentationFramework, WindowsBase
$H = [int]($W * 346 / 260)
$rows = Get-Content $Tsv -Encoding UTF8 | ForEach-Object { $p = $_ -split '\|'; [pscustomobject]@{ Id = $p[0]; Type = $p[1]; Name = $p[2] } }
$groups = $rows | Group-Object { $_.Type + '_' + $(if ($_.Id.StartsWith('3')) { 'SSR' } else { 'SR' }) }
foreach ($g in $groups) {
  $items = @($g.Group)
  $n = $items.Count
  $r = [math]::Ceiling($n / $Cols)
  $cellH = $H + 34
  $dv = New-Object System.Windows.Media.DrawingVisual
  $dc = $dv.RenderOpen()
  $dc.DrawRectangle([System.Windows.Media.Brushes]::White, $null, (New-Object System.Windows.Rect 0, 0, ($Cols * ($W + 6)), ($r * $cellH)))
  for ($i = 0; $i -lt $n; $i++) {
    $it = $items[$i]
    $x = ($i % $Cols) * ($W + 6); $y = [math]::Floor($i / $Cols) * $cellH
    $f = Join-Path $Dir ($it.Id + '.webp')
    $bmp = New-Object System.Windows.Media.Imaging.BitmapImage
    $bmp.BeginInit(); $bmp.UriSource = New-Object System.Uri $f; $bmp.CacheOption = 'OnLoad'; $bmp.EndInit()
    $dc.DrawImage($bmp, (New-Object System.Windows.Rect $x, $y, $W, $H))
    $label = $it.Id + ' ' + $it.Name
    $ft = New-Object System.Windows.Media.FormattedText($label, [System.Globalization.CultureInfo]::InvariantCulture, 'LeftToRight', (New-Object System.Windows.Media.Typeface 'Segoe UI'), 11, [System.Windows.Media.Brushes]::Black, 1.0)
    $ft.MaxTextWidth = $W; $ft.MaxTextHeight = 32
    $dc.DrawText($ft, (New-Object System.Windows.Point $x, ($y + $H + 1)))
  }
  $dc.Close()
  $rtb = New-Object System.Windows.Media.Imaging.RenderTargetBitmap(($Cols * ($W + 6)), ($r * $cellH), 96, 96, [System.Windows.Media.PixelFormats]::Pbgra32)
  $rtb.Render($dv)
  $enc = New-Object System.Windows.Media.Imaging.PngBitmapEncoder
  $enc.Frames.Add([System.Windows.Media.Imaging.BitmapFrame]::Create($rtb))
  $path = Join-Path $OutDir ($g.Name + '.png')
  $fs = [System.IO.File]::Create($path); $enc.Save($fs); $fs.Close()
  "$path  ($n cards)"
}
