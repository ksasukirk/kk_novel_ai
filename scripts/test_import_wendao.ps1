# 问道红尘：知识库导入 / 迁移 / 可选蒸馏
# 代码路径: kk_novel_ai/scripts/test_import_wendao.ps1
param(
    [switch]$Distill,
    [switch]$MigrateOnly,
    [int]$From = 1,
    [int]$To = 20,
    [string]$Apply = "auto",
    [string]$WorkRoot = ""
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot
if (-not $WorkRoot) {
    $WorkRoot = Join-Path $RepoRoot "test_files\_imported\wendao_hongchen"
}
$TxtDir = Join-Path $RepoRoot "test_files"
$Txt = Get-ChildItem -LiteralPath $TxtDir -Filter "*.txt" |
    Sort-Object Length -Descending |
    Select-Object -First 1 -ExpandProperty FullName
$Cli = Join-Path $RepoRoot "src-tauri\target\debug\kk_novel_cli.exe"
if (-not (Test-Path -LiteralPath $Cli)) {
    Write-Host "Building kk_novel_cli..."
    Push-Location (Join-Path $RepoRoot "src-tauri")
    $env:CARGO_TARGET_DIR = Join-Path $RepoRoot "src-tauri\target"
    cargo build --bin kk_novel_cli
    Pop-Location
}

Write-Host "CLI: $Cli"
Write-Host "WorkRoot: $WorkRoot"

if ($MigrateOnly -or (Test-Path (Join-Path $WorkRoot "project.json"))) {
    Write-Host "Migrating existing project to knowledge_base..."
    & $Cli kb migrate $WorkRoot --sync
} else {
    if (-not $Txt) { throw "No TXT under $TxtDir" }
    New-Item -ItemType Directory -Force -Path $WorkRoot | Out-Null
    Write-Host "Importing as knowledge_base: $Txt"
    & $Cli kb import-txt $WorkRoot --file $Txt --title "问道红尘"
}

$proj = Get-Content (Join-Path $WorkRoot "project.json") -Raw -Encoding UTF8 | ConvertFrom-Json
Write-Host "kind=$($proj.kind) chapters=$($proj.chapters.Count) title=$($proj.title)"
if ($proj.kind -ne "knowledge_base") { throw "Expected kind=knowledge_base" }

& $Cli kb list

if ($Distill) {
    Write-Host "Distilling $From..$To apply=$Apply"
    & $Cli kb distill $WorkRoot --from $From --to $To --apply $Apply --resume
    & $Cli kb sync $WorkRoot
}

Write-Host "OK"
