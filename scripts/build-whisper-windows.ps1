$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectDirectory = Split-Path -Parent $scriptDirectory
$versionsFile = Join-Path $scriptDirectory "sidecar-versions.zsh"
$targetTriple = "x86_64-pc-windows-msvc"
$buildRoot = Join-Path $projectDirectory "target/windows-sidecars"
$sourceCache = Join-Path $projectDirectory "target/sidecar-source-cache"
$binariesDirectory = Join-Path $projectDirectory "src-tauri/binaries"

function Read-PinnedVersions {
    $values = @{}
    foreach ($line in Get-Content $versionsFile) {
        if ($line -match '^([A-Z0-9_]+)=(.*)$') {
            $value = $Matches[2].Trim()
            if ($value.StartsWith('"') -and $value.EndsWith('"')) {
                $value = $value.Substring(1, $value.Length - 2)
            }
            $values[$Matches[1]] = $value
        }
    }
    return $values
}

function Get-VerifiedSource([string]$url, [string]$output, [string]$expectedHash) {
    if (Test-Path $output) {
        $actualHash = (Get-FileHash -Algorithm SHA256 $output).Hash.ToLowerInvariant()
        if ($actualHash -eq $expectedHash) {
            Write-Host "Using cached source: $output"
            return
        }
        Remove-Item $output -Force
    }

    Invoke-WebRequest -Uri $url -OutFile $output
    $actualHash = (Get-FileHash -Algorithm SHA256 $output).Hash.ToLowerInvariant()
    if ($actualHash -ne $expectedHash) {
        throw "SHA-256 mismatch for $output`: expected $expectedHash, found $actualHash"
    }
}

$versions = Read-PinnedVersions
$archive = Join-Path $sourceCache "whisper.cpp-$($versions['WHISPER_COMMIT']).tar.gz"
$sourceDirectory = Join-Path $buildRoot "whisper.cpp-$($versions['WHISPER_COMMIT'])"
$cmakeBuildDirectory = Join-Path $buildRoot "whisper-msvc-build"
$outputBinary = Join-Path $binariesDirectory "whisper-cli-$targetTriple.exe"

New-Item -ItemType Directory -Force $buildRoot, $sourceCache, $binariesDirectory | Out-Null
Get-VerifiedSource $versions["WHISPER_SOURCE_URL"] $archive $versions["WHISPER_SOURCE_SHA256"]

if (Test-Path $sourceDirectory) {
    Remove-Item $sourceDirectory -Recurse -Force
}
if (Test-Path $cmakeBuildDirectory) {
    Remove-Item $cmakeBuildDirectory -Recurse -Force
}

tar -xf $archive -C $buildRoot
if ($LASTEXITCODE -ne 0) {
    throw "Failed to extract whisper.cpp source archive"
}

cmake -S $sourceDirectory -B $cmakeBuildDirectory -A x64 `
    -DBUILD_SHARED_LIBS=OFF `
    -DGGML_STATIC=ON `
    -DGGML_BACKEND_DL=OFF `
    -DGGML_NATIVE=OFF `
    -DGGML_OPENMP=OFF `
    -DWHISPER_BUILD_EXAMPLES=ON `
    -DWHISPER_BUILD_SERVER=OFF `
    -DWHISPER_BUILD_TESTS=OFF `
    -DWHISPER_SDL2=OFF
if ($LASTEXITCODE -ne 0) {
    throw "Failed to configure whisper.cpp"
}

cmake --build $cmakeBuildDirectory --config Release --target whisper-cli --parallel
if ($LASTEXITCODE -ne 0) {
    throw "Failed to build whisper-cli"
}

$builtBinary = Join-Path $cmakeBuildDirectory "bin/Release/whisper-cli.exe"
if (-not (Test-Path $builtBinary)) {
    throw "Missing built whisper-cli: $builtBinary"
}
Copy-Item $builtBinary $outputBinary -Force

& $outputBinary --help | Select-Object -First 8
if ($LASTEXITCODE -ne 0) {
    throw "Built whisper-cli did not start successfully"
}

Write-Host "Built and verified $outputBinary"
