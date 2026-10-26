# 產生 Windows 發行壓縮檔（MadEdit-<版本>-windows-<架構>.zip）。
#
# 用法：
#   powershell -ExecutionPolicy Bypass -File .\scripts\make_windows_dist.ps1 [輸出目錄]
#   powershell -ExecutionPolicy Bypass -File .\scripts\make_windows_dist.ps1 -Arch aarch64
#
# 架構為 x86_64 或 aarch64。省略時使用本機架構。
# 與本機不同的架構會以 cargo --target 交叉編譯（需已安裝對應的 MSVC 目標）。

param(
    [string]$OutDir = "",
    [string]$Arch = ""
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

function Normalize-Arch([string]$Value) {
    switch ($Value.ToLower()) {
        "x86_64" { return "x86_64" }
        "amd64" { return "x86_64" }
        "x64" { return "x86_64" }
        "aarch64" { return "aarch64" }
        "arm64" { return "aarch64" }
        default {
            Write-Error "不支援的架構：$Value（請用 x86_64 或 aarch64）"
            exit 1
        }
    }
}

$HostArch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "aarch64" } else { "x86_64" }
if (-not $Arch) { $Arch = $HostArch }
$Arch = Normalize-Arch $Arch
$Target = if ($Arch -eq "aarch64") { "aarch64-pc-windows-msvc" } else { "x86_64-pc-windows-msvc" }

$AppName = "MadEdit"
$DistName = "$AppName-$Version-windows-$Arch"
$ZipFile = Join-Path $OutDir "$DistName.zip"
$StageDir = Join-Path $OutDir "staging-$DistName"

Write-Host "==> 建置 Release 執行檔（$Arch）..."
Push-Location $Root
try {
    if ($Arch -eq $HostArch) {
        cargo build --release --locked
        $Bin = Join-Path $Root "target\release\$AppName.exe"
    } else {
        rustup target add $Target
        cargo build --release --locked --target $Target
        $Bin = Join-Path $Root "target\$Target\release\$AppName.exe"
    }
} finally {
    Pop-Location
}
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
$Hash = (Get-FileHash $ZipFile -Algorithm SHA256).Hash.ToLower()
$ShaFile = "$ZipFile.sha256"
$Utf8 = New-Object System.Text.UTF8Encoding $false
[System.IO.File]::WriteAllText($ShaFile, "$Hash  $DistName.zip`n", $Utf8)
Write-Host "==> 打包完成！"
Write-Host "    檔案：$ZipFile"
Write-Host "    大小：$($ZipItem.Length) bytes"
Write-Host "    SHA256: $Hash"
