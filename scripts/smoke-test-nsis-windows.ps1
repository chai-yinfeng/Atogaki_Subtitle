param(
    [Parameter(Mandatory = $true)]
    [string]$Installer
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$installerPath = (Resolve-Path $Installer).Path
$installProcess = Start-Process -FilePath $installerPath -ArgumentList "/S" -Wait -PassThru
if ($installProcess.ExitCode -ne 0) {
    throw "NSIS installer exited with code $($installProcess.ExitCode)"
}

$uninstallRoot = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall"
$entry = Get-ChildItem $uninstallRoot |
    ForEach-Object { Get-ItemProperty $_.PSPath } |
    Where-Object { $_.DisplayName -eq "Atogaki" } |
    Select-Object -First 1
if (-not $entry) {
    throw "Atogaki did not register a current-user uninstaller"
}

$uninstallCommand = $entry.UninstallString
if (-not $uninstallCommand) {
    throw "Atogaki uninstall entry has no UninstallString"
}
$uninstallCommand = $uninstallCommand.Trim()
if ($uninstallCommand -match '^"([^"]+)"') {
    $uninstaller = $Matches[1]
} else {
    $uninstaller = $uninstallCommand.Split(' ')[0]
}
if (-not (Test-Path $uninstaller)) {
    throw "Registered Atogaki uninstaller does not exist: $uninstaller"
}
$installDirectory = Split-Path -Parent $uninstaller

$app = Join-Path $installDirectory "atogaki-desktop.exe"
if (-not (Test-Path $app)) {
    throw "Installed application executable is missing: $app"
}

$appStream = [System.IO.File]::OpenRead($app)
$appReader = $null
try {
    $appReader = [System.IO.BinaryReader]::new($appStream)
    $appStream.Position = 0x3c
    $peHeaderOffset = $appReader.ReadInt32()
    $appStream.Position = $peHeaderOffset + 92
    $subsystem = $appReader.ReadUInt16()
} finally {
    if ($appReader) { $appReader.Dispose() }
    $appStream.Dispose()
}
if ($subsystem -ne 2) {
    throw "Installed application is not a Windows GUI executable (PE subsystem: $subsystem)"
}

$ffmpeg = Get-ChildItem $installDirectory -Filter "ffmpeg*.exe" -Recurse | Select-Object -First 1
$ffprobe = Get-ChildItem $installDirectory -Filter "ffprobe*.exe" -Recurse | Select-Object -First 1
$whisper = Get-ChildItem $installDirectory -Filter "whisper-cli*.exe" -Recurse | Select-Object -First 1
foreach ($sidecar in @($ffmpeg, $ffprobe, $whisper)) {
    if (-not $sidecar) {
        throw "Installed application is missing one or more sidecars"
    }
}

$filters = & $ffmpeg.FullName -hide_banner -filters 2>&1 | Out-String
if ($LASTEXITCODE -ne 0 -or $filters -notmatch '(?m)^\s*\S+\s+ass\s+') {
    throw "Installed FFmpeg cannot report the libass filter"
}
$encoders = & $ffmpeg.FullName -hide_banner -encoders 2>&1 | Out-String
if ($LASTEXITCODE -ne 0 -or $encoders -notmatch '(?m)^\s*\S+\s+mpeg4\s+') {
    throw "Installed FFmpeg cannot report the MPEG-4 encoder"
}
& $ffprobe.FullName -version | Select-Object -First 1
if ($LASTEXITCODE -ne 0) {
    throw "Installed ffprobe did not start"
}
& $whisper.FullName --help | Select-Object -First 4
if ($LASTEXITCODE -ne 0) {
    throw "Installed whisper-cli did not start"
}

$manifest = Get-ChildItem $installDirectory -Filter "build-manifest.txt" -Recurse | Select-Object -First 1
if (-not $manifest) {
    throw "Installed application is missing the sidecar build manifest"
}
$manifestText = Get-Content $manifest.FullName -Raw
if ($manifestText -notmatch '(?m)^target=x86_64-pc-windows-msvc$' -or $manifestText -notmatch '(?m)^windows_toolchain=msvc\+msys2-ucrt64$') {
    throw "Installed sidecar manifest does not describe the Windows build"
}

$uninstallProcess = Start-Process -FilePath $uninstaller -ArgumentList "/S" -Wait -PassThru
if ($uninstallProcess.ExitCode -ne 0) {
    throw "NSIS uninstaller exited with code $($uninstallProcess.ExitCode)"
}
if (Test-Path $app) {
    throw "Atogaki executable remains after silent uninstall: $app"
}

Write-Host "NSIS install, bundled sidecars, compliance resources and uninstall smoke passed"
