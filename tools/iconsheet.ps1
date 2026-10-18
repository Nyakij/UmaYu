param([string]$Dir, [string]$Tsv, [string]$OutPrefix, [int]$Cols = 10, [int]$W = 110, [int]$Chunk = 50)
Add-Type -AssemblyName PresentationCore, PresentationFramework, WindowsBase
$rows = Get-Content $Tsv -Encoding UTF8 | ForEach-Object { $p = $_ -split '\|'; [pscustomobject]@{ Id = $p[0]; Name = $p[1] } }
$cellH = $W + 30
for ($start = 0; $start -lt $rows.Count; $start += $Chunk) {
  $items = @($rows[$start..([math]::Min($start + $Chunk, $rows.Count) - 1)])
  $n = $items.Count; $r = [math]::Ceiling($n / $Cols)
  $dv = New-Object System.Windows.Media.DrawingVisual; $dc = $dv.RenderOpen()
  $dc.DrawRectangle([System.Windows.Media.Brushes]::White, $null, (New-Object System.Windows.Rect 0, 0, ($Cols * ($W + 4)), ($r * $cellH)))
  for ($i = 0; $i -lt $n; $i++) {
    $it = $items[$i]; $x = ($i % $Cols) * ($W + 4); $y = [math]::Floor($i / $Cols) * $cellH
    $bmp = New-Object System.Windows.Media.Imaging.BitmapImage
    $bmp.BeginInit(); $bmp.UriSource = New-Object System.Uri (Join-Path $Dir ($it.Id + '.webp')); $bmp.CacheOption = 'OnLoad'; $bmp.EndInit()
    $dc.DrawImage($bmp, (New-Object System.Windows.Rect $x, $y, $W, $W))
    $ft = New-Object System.Windows.Media.FormattedText(($it.Id + ' ' + $it.Name), [System.Globalization.CultureInfo]::InvariantCulture, 'LeftToRight', (New-Object System.Windows.Media.Typeface 'Segoe UI'), 10, [System.Windows.Media.Brushes]::Black, 1.0)
    $ft.MaxTextWidth = $W; $ft.MaxTextHeight = 28
    $dc.DrawText($ft, (New-Object System.Windows.Point $x, ($y + $W + 1)))
  }
  $dc.Close()
  $rtb = New-Object System.Windows.Media.Imaging.RenderTargetBitmap(($Cols * ($W + 4)), ($r * $cellH), 96, 96, [System.Windows.Media.PixelFormats]::Pbgra32)
  $rtb.Render($dv)
  $enc = New-Object System.Windows.Media.Imaging.PngBitmapEncoder
  $enc.Frames.Add([System.Windows.Media.Imaging.BitmapFrame]::Create([System.Windows.Media.Imaging.BitmapSource]$rtb))
  $path = "$OutPrefix$start.png"; $fs = [System.IO.File]::Create($path); $enc.Save($fs); $fs.Close(); "$path ($n)"
}
