# 產生 Windows x86_64 發行壓縮檔（MadEdit-<版本>-windows-x86_64.zip）
#
# 用法：
#   powershell -ExecutionPolicy Bypass -File .\scripts\make_windows_dist.ps1 [輸出目錄]

param(
    [string]$OutDir = ""
)

$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$CargoToml = Get-Content (Join-Path $Root "Cargo.toml") -Raw
if ($CargoToml -match 'version\s*=\s*"([^"]+)"') {
    $Version = $Matches[1]
} else {
    Write-Error "無法自 Cargo.toml 解析版本號"
    exit 1
}

if (-not $OutDir) {
    $OutDir = Join-Path $Root "dist"
}

if (-not (Test-Path $OutDir)) {
    New-Item -ItemType Directory -Path $OutDir | Out-Null
}

$AppName = "MadEdit"
$DistName = "$AppName-$Version-windows-x86_64"
$ZipFile = Join-Path $OutDir "$DistName.zip"
$StageDir = Join-Path $OutDir "staging-$DistName"

Write-Host "==> 建置 Release 執行檔..."
Push-Location $Root
try {
    cargo build --release
} finally {
    Pop-Location
}

$Bin = Join-Path $Root "target\release\$AppName.exe"
if (-not (Test-Path $Bin)) {
    Write-Error "找不到執行檔：$Bin"
    exit 1
}

Write-Host "==> 準備打包目錄：$StageDir"
if (Test-Path $StageDir) {
    Remove-Item $StageDir -Recurse -Force
}
if (Test-Path $ZipFile) {
    Remove-Item $ZipFile -Force
}

New-Item -ItemType Directory -Path $StageDir | Out-Null
Copy-Item $Bin -Destination (Join-Path $StageDir "$AppName.exe")

$DictSrc = Join-Path $Root "assets\dictionaries"
if (Test-Path $DictSrc) {
    Copy-Item $DictSrc -Destination (Join-Path $StageDir "dictionaries") -Recurse
}

Write-Host "==> 壓縮為 $ZipFile..."
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($StageDir, $ZipFile, [System.IO.Compression.CompressionLevel]::Optimal, $false)

Remove-Item $StageDir -Recurse -Force

$ZipItem = Get-Item $ZipFile
$Hash = (Get-FileHash $ZipFile -Algorithm SHA256).Hash
Write-Host "==> 打包完成！"
Write-Host "    檔案：$ZipFile"
Write-Host "    大小：$($ZipItem.Length) bytes"
Write-Host "    SHA256: $Hash"
