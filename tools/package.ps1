param([switch]$SkipDependencyInstall)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
function Copy-IfDifferent([string]$Source, [string]$Destination) {
    if (Test-Path -LiteralPath $Destination -PathType Leaf) {
        if ((Get-FileHash -LiteralPath $Source).Hash -eq (Get-FileHash -LiteralPath $Destination).Hash) { return }
    }
    Copy-Item -LiteralPath $Source -Destination $Destination -Force
}
Push-Location -LiteralPath $projectRoot
try {
    if (-not (Test-Path 'vendor\ffmpeg.exe') -or -not (Test-Path 'vendor\ffprobe.exe') -or -not (Test-Path 'vendor\LICENSE') -or -not (Test-Path 'vendor\SOURCE.txt')) {
        python -X utf8 tools/fetch_ffmpeg.py
        if ($LASTEXITCODE -ne 0) { throw 'FFmpeg preparation failed.' }
    }
    python -X utf8 tools/prepare_concat.py
    if ($LASTEXITCODE -ne 0) { throw 'Concat preparation failed.' }
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Rust build failed.' }
    $outputDir = Join-Path $projectRoot 'dist\ScreenRecorder'
    New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
    Copy-IfDifferent (Join-Path $projectRoot 'target\release\screen-recorder.exe') (Join-Path $outputDir 'ScreenRecorder.exe')
    foreach ($name in @('ffmpeg.exe', 'ffprobe.exe')) {
        Copy-IfDifferent (Join-Path $projectRoot "vendor\$name") (Join-Path $outputDir $name)
    }
    foreach ($name in @('README.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md', 'DESIGN_REFERENCES.md', 'VALIDATION.md', 'CONCAT.md')) {
        $sourcePath = Join-Path $projectRoot $name
        if (Test-Path -LiteralPath $sourcePath) { Copy-Item -LiteralPath $sourcePath -Destination $outputDir -Force }
    }
    Copy-Item -LiteralPath (Join-Path $projectRoot 'vendor\LICENSE') -Destination (Join-Path $outputDir 'FFMPEG_LICENSE.txt') -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot 'vendor\SOURCE.txt') -Destination (Join-Path $outputDir 'FFMPEG_SOURCE.txt') -Force
    python tools\collect_licenses.py
    if ($LASTEXITCODE -ne 0) { throw 'License collection failed.' }
    $archivePath = Join-Path $projectRoot 'dist\ScreenRecorder-portable.zip'
    python tools\package_archive.py
    if ($LASTEXITCODE -ne 0) { throw 'Archive creation failed.' }
    Get-FileHash -LiteralPath $archivePath -Algorithm SHA256 | Format-List
} finally { Pop-Location }
