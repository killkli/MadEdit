# 下載固定版本的 dprint 執行檔與 Wasm 插件（Windows「帶 DPRINT」發行版用）。
#
# 版本、網址與 SHA-256 一律取自 assets/formatters/manifest.json；
# 已存在且雜湊正確的檔案不會重新下載。
#
# 用法：
#   powershell -ExecutionPolicy Bypass -File .\scripts\fetch_formatter.ps1 `
#       [-OutDir target\formatters] [-Target x86_64-pc-windows-msvc]

param(
    [string]$OutDir = "",
    [string]$Target = ""
)

$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$ManifestPath = Join-Path $Root "assets\formatters\manifest.json"
if (-not $OutDir) { $OutDir = Join-Path $Root "target\formatters" }
if (-not $Target) {
    $Arch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "aarch64" } else { "x86_64" }
    $Target = "$Arch-pc-windows-msvc"
}

New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
$PluginsDir = Join-Path $OutDir "plugins"
New-Item -ItemType Directory -Path $PluginsDir -Force | Out-Null

function Get-FileSha256([string]$Path) {
    (Get-FileHash -Path $Path -Algorithm SHA256).Hash.ToLower()
}

function Fetch-Verified([string]$Url, [string]$Expected, [string]$Dest) {
    if ((Test-Path $Dest) -and ((Get-FileSha256 $Dest) -eq $Expected)) {
        Write-Host "  已存在且驗證通過：$(Split-Path -Leaf $Dest)"
        return
    }
    Write-Host "  下載 $Url"
    $tmp = "$Dest.tmp"
    Invoke-WebRequest -Uri $Url -OutFile $tmp -UseBasicParsing
    if ((Get-FileSha256 $tmp) -ne $Expected) {
        Remove-Item $tmp -Force
        throw "SHA-256 不符：$Url"
    }
    Move-Item -Force $tmp $Dest
}

$Manifest = Get-Content $ManifestPath -Raw | ConvertFrom-Json

# --- dprint 執行檔 ---
$Entry = $Manifest.dprint.targets.$Target
if (-not $Entry) { throw "manifest.json 沒有 $Target 的 dprint。" }
Write-Host "dprint $($Manifest.dprint.version)（$Target）"
$Binary = Join-Path $OutDir "dprint.exe"
# sidecar 記下壓縮檔雜湊；一致時連 dprint 本體都不用重抓
$Sidecar = Join-Path $OutDir ".dprint-zip-sha256"
if ((Test-Path $Binary) -and (Test-Path $Sidecar) -and ((Get-Content $Sidecar -Raw).Trim() -eq $Entry.sha256)) {
    Write-Host "  執行檔已存在且驗證通過：dprint.exe"
} else {
    $tmpZip = Join-Path $OutDir "dprint.zip"
    Fetch-Verified $Entry.url $Entry.sha256 $tmpZip
    $tmpExtract = Join-Path $OutDir "dprint-extract"
    if (Test-Path $tmpExtract) { Remove-Item $tmpExtract -Recurse -Force }
    Expand-Archive -Path $tmpZip -DestinationPath $tmpExtract -Force
    $Member = Get-ChildItem $tmpExtract -Recurse -Filter "dprint.exe" | Select-Object -First 1
    if (-not $Member) { throw "壓縮檔內找不到 dprint.exe" }
    Move-Item -Force $Member.FullName $Binary
    Remove-Item $tmpExtract -Recurse -Force
    Remove-Item $tmpZip -Force
    Set-Content -Path $Sidecar -Value $Entry.sha256 -NoNewline
}

# --- Wasm 插件 ---
foreach ($Prop in $Manifest.plugins.PSObject.Properties) {
    $Name = $Prop.Name
    $Info = $Prop.Value
    Write-Host "插件 $Name $($Info.version)"
    Fetch-Verified $Info.url $Info.sha256 (Join-Path $PluginsDir "$Name.wasm")
}

# 清掉 manifest 以外的殘留插件，避免舊檔被一起打包
$Keep = @($Manifest.plugins.PSObject.Properties.Name | ForEach-Object { "$_.wasm" })
Get-ChildItem $PluginsDir -Filter "*.wasm" | Where-Object { $Keep -notcontains $_.Name } | ForEach-Object {
    Write-Host "  移除非 manifest 插件：$($_.Name)"
    Remove-Item $_.FullName -Force
}

# --- 設定檔（跟著 repo 走，永遠覆寫以保持同步） ---
Copy-Item (Join-Path $Root "assets\formatters\dprint.json") (Join-Path $OutDir "dprint.json") -Force
Write-Host "完成：$OutDir"
