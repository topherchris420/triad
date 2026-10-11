param([string]$Action = 'build')
$ErrorActionPreference = 'Stop'
$rustBin = Join-Path $env:USERPROFILE '.rustup\toolchains\stable-x86_64-pc-windows-gnu\bin'
$linkerBin = Join-Path $env:USERPROFILE '.local\triad-toolchain\w64devkit\bin'
$env:PATH = "$rustBin;$linkerBin;$env:PATH"
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = 'x86_64-w64-mingw32-gcc.exe'
$env:CC = 'x86_64-w64-mingw32-gcc.exe'
$env:AR = 'ar.exe'
$env:RUSTC = Join-Path $rustBin 'rustc.exe'
$env:RUSTFLAGS = '-C link-self-contained=yes'
$cargoArgs = switch ($Action) {
    'build' { @('build', '--release', '--locked') }
    'test' { @('test', '--release', '--locked') }
    'fmt' { @('fmt', '--all') }
    default { throw 'Expected build, test, or fmt' }
}
if (-not (Test-Path -LiteralPath (Join-Path $rustBin 'cargo.exe'))) { throw 'Rust toolchain is missing' }
# Windows PowerShell otherwise treats redirected native stderr progress as a
# terminating error. Preserve text and use Cargo's actual exit status.
$ErrorActionPreference = 'Continue'
& (Join-Path $rustBin 'cargo.exe') @cargoArgs 2>&1 | ForEach-Object { $_.ToString() }
$cargoExit = $LASTEXITCODE
exit $cargoExit
