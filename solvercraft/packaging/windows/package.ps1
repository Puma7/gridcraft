[CmdletBinding()]
param([string]$TargetDir, [string]$OutputDir, [switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
if (-not $TargetDir) { $TargetDir = Join-Path $repositoryRoot 'target' }
if (-not $OutputDir) { $OutputDir = Join-Path $repositoryRoot 'dist' }
$TargetDir = [IO.Path]::GetFullPath($TargetDir)
$OutputDir = [IO.Path]::GetFullPath($OutputDir)
if (-not $env:CRAFT_FONTS_DIR -or -not (Test-Path -LiteralPath (Join-Path $env:CRAFT_FONTS_DIR 'fonts') -PathType Container)) {
    throw 'Set CRAFT_FONTS_DIR to the documented font checkout for building and for packaging its license notices, including -SkipBuild.'
}
$cargoText = Get-Content -LiteralPath (Join-Path $repositoryRoot 'Cargo.toml') -Raw
$versionMatch = [regex]::Match($cargoText, '(?ms)\[workspace\.package\].*?^version\s*=\s*"([^"]+)"')
if (-not $versionMatch.Success) { throw 'Cannot read workspace application version.' }
$appVersion = $versionMatch.Groups[1].Value
if (-not $SkipBuild) {
    if (-not $env:CRAFT_FONTS_DIR) { throw 'Set CRAFT_FONTS_DIR to the documented Craft font checkout before building the desktop app.' }
    $previousRequiredFonts = $env:CRAFT_FONTS_REQUIRED
    try {
        $env:CRAFT_FONTS_REQUIRED = '1'
        & cargo build --manifest-path (Join-Path $repositoryRoot 'Cargo.toml') --release --locked --target-dir $TargetDir -p solvercraft-app
        if ($LASTEXITCODE -ne 0) { throw 'Solvercraft desktop build failed.' }
        & cargo build --manifest-path (Join-Path $repositoryRoot 'solvercraft/Cargo.toml') --release --locked --target-dir $TargetDir -p solvercraft-cli -p solvercraft-mcp
        if ($LASTEXITCODE -ne 0) { throw 'Solvercraft CLI/MCP build failed.' }
    } finally { $env:CRAFT_FONTS_REQUIRED = $previousRequiredFonts }
}
$executables = @('solvercraft.exe', 'solvercraft-cli.exe', 'solvercraft-mcp.exe')
foreach ($executable in $executables) {
    if (-not (Test-Path -LiteralPath (Join-Path $TargetDir "release/$executable") -PathType Leaf)) { throw "Missing release program: $executable" }
}
$stagingRoot = Join-Path $TargetDir ('solvercraft-package-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stagingRoot -Force | Out-Null
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
foreach ($executable in $executables) { Copy-Item -LiteralPath (Join-Path $TargetDir "release/$executable") -Destination $stagingRoot }
Copy-Item -LiteralPath (Join-Path $repositoryRoot 'solvercraft/examples') -Destination (Join-Path $stagingRoot 'examples') -Recurse
& python (Join-Path $PSScriptRoot '../licenses.py') --repository $repositoryRoot --output (Join-Path $stagingRoot 'licenses')
if ($LASTEXITCODE -ne 0) { throw 'Resolved dependency notice collection failed.' }
$sdkLicenseRoot = Join-Path $stagingRoot 'licenses/solvercraft'
New-Item -ItemType Directory -Path $sdkLicenseRoot -Force | Out-Null
foreach ($notice in @('LICENSE-MIT', 'LICENSE-APACHE')) { Copy-Item -LiteralPath (Join-Path $repositoryRoot "solvercraft/$notice") -Destination $sdkLicenseRoot }
Copy-Item -LiteralPath (Join-Path $repositoryRoot 'solvercraft/crates/math/third_party') -Destination (Join-Path $stagingRoot 'licenses/solvercraft-math-data') -Recurse
foreach ($notice in @('LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE', 'ATTRIBUTION.md')) { Copy-Item -LiteralPath (Join-Path $repositoryRoot $notice) -Destination $stagingRoot }
Copy-Item -LiteralPath (Join-Path $PSScriptRoot '../START-HERE.txt') -Destination $stagingRoot
if ($env:CRAFT_FONTS_DIR) {
    $fontLicenseRoot = Join-Path $stagingRoot 'licenses/craft-fonts'
    New-Item -ItemType Directory -Path $fontLicenseRoot -Force | Out-Null
    $fontSourceRoot = [IO.Path]::GetFullPath($env:CRAFT_FONTS_DIR)
    Get-ChildItem -LiteralPath (Join-Path $fontSourceRoot 'fonts') -File -Recurse | Where-Object { $_.Name -match '^(OFL|LICENSE|COPYING|NOTICE)' } | ForEach-Object {
        $relativeLicense = [IO.Path]::GetRelativePath($fontSourceRoot, $_.FullName)
        $destination = Join-Path $fontLicenseRoot $relativeLicense
        New-Item -ItemType Directory -Path ([IO.Path]::GetDirectoryName($destination)) -Force | Out-Null
        Copy-Item -LiteralPath $_.FullName -Destination $destination
    }
}
$binaryHashes = foreach ($executable in $executables) { '{0} *{1}' -f (Get-FileHash -LiteralPath (Join-Path $stagingRoot $executable) -Algorithm SHA256).Hash.ToLowerInvariant(), $executable }
[IO.File]::WriteAllLines((Join-Path $stagingRoot 'BINARY-SHA256SUMS'), $binaryHashes)
[IO.File]::WriteAllText((Join-Path $stagingRoot 'app-version.txt'), "$appVersion`n")
# Only staging timestamps are clamped to ZIP's supported range.
Get-ChildItem -LiteralPath $stagingRoot -File -Recurse | ForEach-Object { if ($_.LastWriteTimeUtc.Year -lt 1980) { $_.LastWriteTimeUtc = [datetime]::UtcNow } }
$archiveName = "solvercraft-$appVersion-windows-x64-portable.zip"
$archivePath = Join-Path $OutputDir $archiveName
if (Test-Path -LiteralPath $archivePath -PathType Leaf) { Remove-Item -LiteralPath $archivePath }
[IO.Compression.ZipFile]::CreateFromDirectory($stagingRoot, $archivePath, [IO.Compression.CompressionLevel]::Optimal, $false)
$archiveHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
$sumsPath = Join-Path $OutputDir 'SHA256SUMS.txt'
$fallbackSums = Join-Path $OutputDir 'SHA256SUMS'
$checksumRows = [Collections.Generic.List[string]]::new()
$existingSums = if (Test-Path -LiteralPath $sumsPath -PathType Leaf) { $sumsPath } elseif (Test-Path -LiteralPath $fallbackSums -PathType Leaf) { $fallbackSums } else { $null }
if ($existingSums) {
    foreach ($row in [IO.File]::ReadAllLines($existingSums)) {
        $parts = $row.Trim() -split '\s+', 2
        if ($parts.Length -eq 2 -and $parts[1].TrimStart('*') -eq $archiveName) { continue }
        if ($row.Trim()) { $checksumRows.Add($row) }
    }
}
$checksumRows.Add("$archiveHash *$archiveName")
[IO.File]::WriteAllLines($sumsPath, $checksumRows)
[IO.File]::WriteAllLines($fallbackSums, $checksumRows)
Write-Output $archivePath
