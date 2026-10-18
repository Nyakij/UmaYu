param([string]$In, [string]$Out, [int]$X, [int]$Y, [int]$W, [int]$H, [double]$Scale = 3)
Add-Type -AssemblyName PresentationCore, WindowsBase
$bmp = New-Object System.Windows.Media.Imaging.BitmapImage
$bmp.BeginInit(); $bmp.UriSource = New-Object System.Uri $In; $bmp.CacheOption = 'OnLoad'; $bmp.EndInit()
$crop = New-Object System.Windows.Media.Imaging.CroppedBitmap($bmp, (New-Object System.Windows.Int32Rect $X, $Y, $W, $H))
$scaled = New-Object System.Windows.Media.Imaging.TransformedBitmap($crop, (New-Object System.Windows.Media.ScaleTransform $Scale, $Scale))
$enc = New-Object System.Windows.Media.Imaging.PngBitmapEncoder
$enc.Frames.Add([System.Windows.Media.Imaging.BitmapFrame]::Create($scaled))
$fs = [System.IO.File]::Create($Out); $enc.Save($fs); $fs.Close()
"$Out ($($bmp.PixelWidth)x$($bmp.PixelHeight) source)"
