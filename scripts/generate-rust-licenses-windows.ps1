$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectDirectory = Split-Path -Parent $scriptDirectory
$output = Join-Path $projectDirectory "src-tauri/third-party/rust-licenses-windows.html"
$target = "x86_64-pc-windows-msvc"

$version = (& cargo-about --version | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $version -ne "cargo-about 0.9.1") {
    throw "cargo-about 0.9.1 is required for reproducible Windows license output"
}

& cargo-about generate (Join-Path $projectDirectory "about.hbs") `
    --config (Join-Path $projectDirectory "about.toml") `
    --manifest-path (Join-Path $projectDirectory "src-tauri/Cargo.toml") `
    --target $target `
    --locked `
    --offline `
    --fail `
    --output-file $output
if ($LASTEXITCODE -ne 0) {
    throw "Failed to generate Windows Rust license report"
}

$content = [System.IO.File]::ReadAllText($output)
$content = $content.Replace("`r`n", "`n")
$content = [regex]::Replace($content, '[ \t]+(?=\n)', '')
$content = $content.Replace("__ATOGAKI_LICENSE_TARGET__", $target)
[System.IO.File]::WriteAllText($output, $content, [System.Text.UTF8Encoding]::new($false))

Write-Host "Generated Windows Rust license notices: $output"
