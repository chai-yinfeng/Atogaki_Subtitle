param(
    [string]$OutputDirectory
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectDirectory = Split-Path -Parent $scriptDirectory
$versionsFile = Join-Path $scriptDirectory "sidecar-versions.zsh"
$manifest = Join-Path $projectDirectory "src-tauri/third-party/build-manifest.txt"
$sourceCache = Join-Path $projectDirectory "target/sidecar-source-cache"
$targetTriple = "x86_64-pc-windows-msvc"

if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $projectDirectory "target/release/bundle/sources"
}

function Read-KeyValueFile([string]$path) {
    $values = @{}
    foreach ($line in Get-Content $path) {
        if ($line -match '^([A-Za-z0-9_]+)=(.*)$') {
            $value = $Matches[2].Trim()
            if ($value.StartsWith('"') -and $value.EndsWith('"')) {
                $value = $value.Substring(1, $value.Length - 2)
            }
            $values[$Matches[1]] = $value
        }
    }
    return $values
}

function Require-Value($values, [string]$key, [string]$expected) {
    if (-not $values.ContainsKey($key) -or $values[$key] -ne $expected) {
        $actual = if ($values.ContainsKey($key)) { $values[$key] } else { "missing" }
        throw "Manifest mismatch for $key`: expected $expected, found $actual"
    }
}

function Require-FileHash([string]$path, [string]$expected) {
    if (-not (Test-Path $path)) {
        throw "Missing required file: $path"
    }
    $actual = (Get-FileHash -Algorithm SHA256 $path).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
        throw "SHA-256 mismatch for $path`: expected $expected, found $actual"
    }
}

if (-not (Test-Path $manifest)) {
    throw "Missing Windows sidecar manifest: $manifest"
}

$versions = Read-KeyValueFile $versionsFile
$manifestValues = Read-KeyValueFile $manifest
Require-Value $manifestValues "target" $targetTriple
Require-Value $manifestValues "windows_toolchain" "msvc+msys2-ucrt64"

$manifestVersionMap = @{
    "whisper_version" = "WHISPER_VERSION"
    "whisper_commit" = "WHISPER_COMMIT"
    "whisper_source_sha256" = "WHISPER_SOURCE_SHA256"
    "ffmpeg_version" = "FFMPEG_VERSION"
    "ffmpeg_source_sha256" = "FFMPEG_SHA256"
    "libass_version" = "LIBASS_VERSION"
    "libass_source_sha256" = "LIBASS_SHA256"
    "libunibreak_version" = "LIBUNIBREAK_VERSION"
    "libunibreak_source_sha256" = "LIBUNIBREAK_SHA256"
    "fribidi_version" = "FRIBIDI_VERSION"
    "fribidi_source_sha256" = "FRIBIDI_SHA256"
    "freetype_version" = "FREETYPE_VERSION"
    "freetype_source_sha256" = "FREETYPE_SHA256"
    "harfbuzz_version" = "HARFBUZZ_VERSION"
    "harfbuzz_source_sha256" = "HARFBUZZ_SHA256"
}
foreach ($manifestKey in $manifestVersionMap.Keys) {
    Require-Value $manifestValues $manifestKey $versions[$manifestVersionMap[$manifestKey]]
}

$configuration = $manifestValues["ffmpeg_configuration"]
if (-not $configuration.Contains("--disable-gpl") -or -not $configuration.Contains("--disable-nonfree") -or $configuration.Contains("--enable-gpl") -or $configuration.Contains("--enable-nonfree") -or $configuration.Contains("libx264")) {
    throw "FFmpeg manifest does not describe the required LGPL-only build"
}

Require-FileHash (Join-Path $projectDirectory "src-tauri/binaries/ffmpeg-$targetTriple.exe") $manifestValues["prebundle_ffmpeg_binary_sha256"]
Require-FileHash (Join-Path $projectDirectory "src-tauri/binaries/ffprobe-$targetTriple.exe") $manifestValues["prebundle_ffprobe_binary_sha256"]
Require-FileHash (Join-Path $projectDirectory "src-tauri/binaries/whisper-cli-$targetTriple.exe") $manifestValues["prebundle_whisper_binary_sha256"]

$sourceFiles = @(
    @{ Name = "whisper.cpp-$($versions['WHISPER_COMMIT']).tar.gz"; Hash = $versions["WHISPER_SOURCE_SHA256"] },
    @{ Name = "ffmpeg-$($versions['FFMPEG_VERSION']).tar.xz"; Hash = $versions["FFMPEG_SHA256"] },
    @{ Name = "libass-$($versions['LIBASS_VERSION']).tar.xz"; Hash = $versions["LIBASS_SHA256"] },
    @{ Name = "libunibreak-$($versions['LIBUNIBREAK_VERSION']).tar.gz"; Hash = $versions["LIBUNIBREAK_SHA256"] },
    @{ Name = "fribidi-$($versions['FRIBIDI_VERSION']).tar.xz"; Hash = $versions["FRIBIDI_SHA256"] },
    @{ Name = "freetype-$($versions['FREETYPE_VERSION']).tar.xz"; Hash = $versions["FREETYPE_SHA256"] },
    @{ Name = "harfbuzz-$($versions['HARFBUZZ_VERSION']).tar.xz"; Hash = $versions["HARFBUZZ_SHA256"] }
)

$appVersion = (Get-Content (Join-Path $projectDirectory "src-tauri/tauri.conf.json") -Raw | ConvertFrom-Json).version
$packageName = "Atogaki-$appVersion-windows-x86_64-third-party-sources"
$stagingParent = Join-Path $projectDirectory "target/windows-source-package"
$packageRoot = Join-Path $stagingParent $packageName
$sourcesDirectory = Join-Path $packageRoot "sources"
$buildDirectory = Join-Path $packageRoot "build"
$licensesDirectory = Join-Path $packageRoot "licenses"

if (Test-Path $stagingParent) {
    Remove-Item $stagingParent -Recurse -Force
}
New-Item -ItemType Directory -Force $sourcesDirectory, $buildDirectory, $licensesDirectory, $OutputDirectory | Out-Null

foreach ($source in $sourceFiles) {
    $cached = Join-Path $sourceCache $source.Name
    Require-FileHash $cached $source.Hash
    Copy-Item $cached (Join-Path $sourcesDirectory $source.Name)
}

Copy-Item (Join-Path $projectDirectory "LICENSE") (Join-Path $packageRoot "ATOGAKI-LICENSE")
Copy-Item (Join-Path $scriptDirectory "build-whisper-windows.ps1") $buildDirectory
Copy-Item (Join-Path $scriptDirectory "build-ffmpeg-windows.sh") $buildDirectory
Copy-Item $versionsFile $buildDirectory
Copy-Item $manifest $buildDirectory
Copy-Item (Join-Path $projectDirectory "src-tauri/third-party/licenses/*") $licensesDirectory -Recurse

$checksumLines = foreach ($source in $sourceFiles) {
    "$($source.Hash)  $($source.Name)"
}
Set-Content -Path (Join-Path $sourcesDirectory "SHA256SUMS") -Value $checksumLines -Encoding utf8NoBOM

$readme = @"
# Atogaki $appVersion Windows corresponding sidecar sources

This archive contains the exact upstream sources, checksums, license texts,
build scripts and binary manifest for the Windows x86_64 sidecars.

whisper-cli is built with MSVC. FFmpeg and its statically linked libass subtitle
stack are built with MSYS2 UCRT64/MinGW-w64. The FFmpeg configuration is LGPL
v2.1-or-later without GPL, nonfree or libx264 components.

Run build/build-whisper-windows.ps1 in a Visual Studio Build Tools environment,
then run build/build-ffmpeg-windows.sh from an MSYS2 UCRT64 shell. The repository
workflow records the exact CI packages and invocation used for this release.
"@
Set-Content -Path (Join-Path $packageRoot "SOURCES.md") -Value $readme -Encoding utf8NoBOM

$archiveName = "$packageName.tar.gz"
$archivePath = Join-Path $OutputDirectory $archiveName
if (Test-Path $archivePath) {
    Remove-Item $archivePath -Force
}
tar -czf $archivePath -C $stagingParent $packageName
if ($LASTEXITCODE -ne 0) {
    throw "Failed to create corresponding-source archive"
}

$archiveHash = (Get-FileHash -Algorithm SHA256 $archivePath).Hash.ToLowerInvariant()
Set-Content -Path "$archivePath.sha256" -Value "$archiveHash  $archiveName" -Encoding ascii
Write-Host "Packaged corresponding sources: $archivePath"
