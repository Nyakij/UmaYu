# Sets up .cargo\ so the app builds with the rustup windows-gnu toolchain alone (no MSVC, no
# full MinGW): hardlinks the toolchain's LLVM tools into .cargo\ and writes .cargo\config.toml
# with this machine's paths. Run once after cloning or unzipping, and again after `rustup update`.
$ErrorActionPreference = 'Stop'
$bin = Join-Path $env:USERPROFILE '.rustup\toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin'
$dest = Join-Path $PSScriptRoot '.cargo'
New-Item -ItemType Directory -Force $dest | Out-Null
$links = @{
    'llvm-dlltool.exe'    = 'llvm-ar.exe'
    'ld.exe'              = 'rust-lld.exe'
    'libgcc_s_seh-1.dll'  = 'libgcc_s_seh-1.dll'
    'libwinpthread-1.dll' = 'libwinpthread-1.dll'
}
foreach ($name in $links.Keys) {
    $target = Join-Path $bin $links[$name]
    if (-not (Test-Path $target)) { throw "Missing $target - install the stable-x86_64-pc-windows-gnu toolchain and run: rustup component add llvm-tools" }
    $path = Join-Path $dest $name
    if (Test-Path $path) { Remove-Item $path -Force }
    New-Item -ItemType HardLink -Path $path -Target $target | Out-Null
    "linked $name -> $($links[$name])"
}

$d = $dest.Replace('\', '/')
@"
# Build setup for the rustup windows-gnu toolchain (no MSVC, no full MinGW). Written by
# setup-toolchain.ps1; rerun it rather than editing paths here.
#
# 1. dlltool: crates using windows-link import Windows APIs via raw-dylib, which needs dlltool.
#    The toolchain's bundled GNU dlltool shells out to an assembler that isn't installed, so use
#    LLVM's instead: llvm-ar behaves as llvm-dlltool when its file name contains "dlltool".
# 2. Linker: GNU ld mislinks llvm-dlltool's short import libraries (an import ends up outside
#    its descriptor and the exe crashes before main), so link with LLD. The bundled gcc is a
#    linker-only driver without collect2, so it ignores -fuse-ld and runs whatever "ld" it finds
#    first; -B points it at .cargo/, where ld.exe is rust-lld (LLD picks GNU/MinGW mode from
#    the name "ld").
#
# Everything in .cargo/ besides this file is a hardlink into
# ~/.rustup/toolchains/stable-x86_64-pc-windows-gnu/lib/rustlib/x86_64-pc-windows-gnu/bin/:
#   llvm-dlltool.exe -> llvm-ar.exe
#   ld.exe           -> rust-lld.exe
#   libgcc_s_seh-1.dll, libwinpthread-1.dll (runtime DLLs those two need)
[target.x86_64-pc-windows-gnu]
rustflags = [
    "-C", "dlltool=$d/llvm-dlltool.exe",
    "-C", "link-arg=-B$d/",
]
"@ | Set-Content -Encoding ascii (Join-Path $dest 'config.toml')
"wrote $dest\config.toml"
