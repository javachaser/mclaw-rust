$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = Split-Path -Parent $PSScriptRoot
$cargoToml = Join-Path $repoRoot 'Cargo.toml'
$targetTriple = 'x86_64-pc-windows-gnu'
$releaseDir = Join-Path $repoRoot "target\$targetTriple\release"
$exePath = Join-Path $releaseDir 'mclaw.exe'
$dllPath = Join-Path $releaseDir 'WebView2Loader.dll'
$payloadDir = Join-Path $PSScriptRoot 'payload'
$bootstrapperPath = Join-Path $payloadDir 'MicrosoftEdgeWebview2Setup.exe'
$bootstrapperUrl = 'https://go.microsoft.com/fwlink/p/?LinkId=2124703'
$issPath = Join-Path $PSScriptRoot 'MClaw.iss'

if (-not (Test-Path $cargoToml)) {
    throw "Cargo.toml not found: $cargoToml"
}

$version = [regex]::Match((Get-Content $cargoToml -Raw), '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
if (-not $version) {
    throw 'Failed to parse version from Cargo.toml'
}

if (-not (Test-Path $payloadDir)) {
    New-Item -ItemType Directory -Path $payloadDir | Out-Null
}

if (-not (Test-Path $bootstrapperPath)) {
    Write-Host "Downloading WebView2 bootstrapper..."
    Invoke-WebRequest -Uri $bootstrapperUrl -OutFile $bootstrapperPath
}

if (-not (Test-Path $exePath) -or -not (Test-Path $dllPath)) {
    Write-Host "Release artifacts missing, building MClaw..."
    $env:PATH = "C:\msys64\mingw64\bin;" + $env:PATH
    Push-Location $repoRoot
    try {
        cargo +stable-x86_64-pc-windows-gnu build --release --target $targetTriple
    }
    finally {
        Pop-Location
    }
}

$iscc = (Get-Command iscc.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -First 1)
if (-not $iscc) {
    $candidatePaths = @(
        'C:\Program Files (x86)\Inno Setup 6\ISCC.exe',
        'C:\Program Files\Inno Setup 6\ISCC.exe',
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe')
    )
    $iscc = $candidatePaths | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $iscc) {
    throw 'ISCC.exe not found. Please install Inno Setup 6 first.'
}


Push-Location $PSScriptRoot
try {
    & $iscc "/DMyAppVersion=$version" $issPath
}
finally {
    Pop-Location
}
