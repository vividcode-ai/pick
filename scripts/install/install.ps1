# Pick standalone installer for Windows
# Usage: irm https://github.com/vividcode-ai/pick/releases/latest/download/install.ps1 | iex

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

# Force TLS 1.2 (required for GitHub API on older PowerShell)
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Repo = "vividcode-ai/pick"
$PickHome = "$env:USERPROFILE\.pick"
$PackagesDir = "$PickHome\packages\standalone"
$ReleasesDir = "$PackagesDir\releases"
$BinDir = "$PickHome\bin"

# Detect platform
$Arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { "i686" }
$Target = "windows-${Arch}"
$Archive = "pick-package-${Target}.tar.gz"

# Fetch latest release
Write-Host "Fetching latest release info..." -ForegroundColor Cyan
try {
    $Latest = Invoke-RestMethod -Uri "https://api.github.com/repos/${Repo}/releases/latest" -Headers @{
        "User-Agent" = "Pick-Installer"
        "Accept" = "application/vnd.github+json"
    }
} catch {
    Write-Host "Error: Could not fetch release info from GitHub." -ForegroundColor Red
    exit 1
}

$Version = $Latest.tag_name -replace '^v', ''
if (-not $Version) {
    Write-Host "Error: Could not parse version from release info." -ForegroundColor Red
    exit 1
}

Write-Host "Latest version: ${Version}" -ForegroundColor Green

# Download
$DownloadUrl = "https://github.com/${Repo}/releases/download/v${Version}/${Archive}"
$TempArchive = Join-Path $env:TEMP $Archive
Write-Host "Downloading ${DownloadUrl}..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempArchive

# Download and verify checksum
$ChecksumsUrl = "https://github.com/${Repo}/releases/download/v${Version}/pick-package_SHA256SUMS"
$TempChecksums = Join-Path $env:TEMP "pick-package_SHA256SUMS"
try {
    Invoke-WebRequest -Uri $ChecksumsUrl -OutFile $TempChecksums -ErrorAction Stop
    $Checksums = Get-Content $TempChecksums
    $ExpectedLine = $Checksums | Where-Object { $_ -match [regex]::Escape($Archive) } | Select-Object -First 1
    if ($ExpectedLine) {
        $ExpectedHash = ($ExpectedLine -split '\s+')[0]
        $ActualHash = (Get-FileHash $TempArchive -Algorithm SHA256).Hash.ToLower()
        if ($ActualHash -ne $ExpectedHash.ToLower()) {
            Write-Host "Error: Checksum mismatch!" -ForegroundColor Red
            Write-Host "  Expected: $ExpectedHash"
            Write-Host "  Actual:   $ActualHash"
            exit 1
        }
        Write-Host "Checksum verified." -ForegroundColor Green
    }
    Remove-Item $TempChecksums -Force -ErrorAction SilentlyContinue
} catch {
    Write-Host "Warning: Could not verify checksums (${ChecksumsUrl})." -ForegroundColor Yellow
}

# Extract
$ExtractDir = Join-Path $ReleasesDir "${Version}-${Target}"
New-Item -ItemType Directory -Path $ExtractDir -Force | Out-Null
Write-Host "Extracting to ${ExtractDir}..." -ForegroundColor Cyan

# Use tar (built-in on Windows 10 1803+)
if (Get-Command tar -ErrorAction SilentlyContinue) {
    tar -xzf $TempArchive -C $ExtractDir
} else {
    # Fallback to .NET
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [System.IO.Compression.ZipFile]::ExtractToDirectory($TempArchive, $ExtractDir)
}

Remove-Item $TempArchive -Force -ErrorAction SilentlyContinue

# Update current symlink
$CurrentLink = Join-Path $PackagesDir "current"
if (Test-Path $CurrentLink) { Remove-Item $CurrentLink -Force -Recurse }
New-Item -ItemType Junction -Path $CurrentLink -Target $ExtractDir | Out-Null

# Copy binary to bin dir
New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
Copy-Item "$ExtractDir\pick.exe" "$BinDir\pick.exe" -Force

Write-Host ""
Write-Host "Adding ${BinDir} to PATH..."
$currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($currentPath -notlike "*${BinDir}*") {
    [Environment]::SetEnvironmentVariable("PATH", "${currentPath};${BinDir}", "User")
    # Also update current session
    $env:PATH = "${env:PATH};${BinDir}"
    Write-Host "  Added ${BinDir} to User PATH." -ForegroundColor Green
} else {
    Write-Host "  ${BinDir} already in PATH." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Pick v${Version} installed successfully!" -ForegroundColor Green
Write-Host "Run 'pick update' to check for future updates." -ForegroundColor Cyan
Write-Host "Run 'pick' to start a new session." -ForegroundColor Cyan
