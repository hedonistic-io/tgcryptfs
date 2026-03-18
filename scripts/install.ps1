#Requires -Version 5.1
<#
.SYNOPSIS
    tgcryptfs installer for Windows.

.DESCRIPTION
    Downloads and installs a pre-built tgcryptfs binary from GitHub releases.
    Windows FUSE support requires WinFsp (https://winfsp.dev/).

    NOTE: Windows support for tgcryptfs is experimental. The primary platforms
    are Linux and macOS where native FUSE is available.

.PARAMETER Version
    Specific version to install (e.g., "0.3.0"). Defaults to latest.

.EXAMPLE
    .\install.ps1
    .\install.ps1 -Version 0.3.0
#>

[CmdletBinding()]
param(
    [string]$Version = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# -- Constants ----------------------------------------------------------------

$Repo = "hedonistic-io/tgcryptfs"
$BinaryName = "tgcryptfs.exe"
$GitHubApi = "https://api.github.com/repos/$Repo"
$GitHubReleases = "https://github.com/$Repo/releases"
$ConfigDir = Join-Path $env:USERPROFILE ".config\tgcryptfs"
$InstallDir = Join-Path $env:LOCALAPPDATA "tgcryptfs\bin"

# -- Output helpers -----------------------------------------------------------

function Write-Info    { param([string]$Msg) Write-Host "[info]  $Msg" -ForegroundColor Blue }
function Write-Ok      { param([string]$Msg) Write-Host "[ok]    $Msg" -ForegroundColor Green }
function Write-Warn    { param([string]$Msg) Write-Host "[warn]  $Msg" -ForegroundColor Yellow }
function Write-Err     { param([string]$Msg) Write-Host "[error] $Msg" -ForegroundColor Red }
function Write-Header  { param([string]$Msg) Write-Host "`n$Msg" -ForegroundColor White -BackgroundColor DarkGray }

function Exit-Fatal {
    param([string]$Msg)
    Write-Err $Msg
    exit 1
}

# -- Architecture detection ---------------------------------------------------

function Get-Arch {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    switch ($arch) {
        "X64"   { return "x86_64" }
        "Arm64" { return "aarch64" }
        default {
            # Fallback for older PowerShell
            if ($env:PROCESSOR_ARCHITECTURE -eq "AMD64") { return "x86_64" }
            if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { return "aarch64" }
            Exit-Fatal "Unsupported architecture: $arch"
        }
    }
}

$Arch = Get-Arch

Write-Header "tgcryptfs installer (Windows)"
Write-Info "Detected architecture: $Arch"
Write-Warn "Windows support is experimental. Linux and macOS are the primary platforms."

# -- WinFsp check -------------------------------------------------------------

Write-Header "Checking FUSE dependencies"

$WinFspInstalled = $false
$winfspPaths = @(
    "C:\Program Files (x86)\WinFsp",
    "C:\Program Files\WinFsp"
)

foreach ($p in $winfspPaths) {
    if (Test-Path $p) {
        $WinFspInstalled = $true
        break
    }
}

if ($WinFspInstalled) {
    Write-Ok "WinFsp is installed."
} else {
    Write-Warn "WinFsp is not installed. FUSE operations will not work without it."
    Write-Warn "Download WinFsp from: https://winfsp.dev/rel/"
    Write-Warn "After installing WinFsp, FUSE mount operations will be available."
    Write-Info "Continuing with binary installation..."
}

# -- Resolve version ----------------------------------------------------------

if ([string]::IsNullOrEmpty($Version)) {
    Write-Info "Fetching latest release..."
    try {
        # GitHub API requires TLS 1.2+
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        $release = Invoke-RestMethod -Uri "$GitHubApi/releases/latest" -UseBasicParsing
        $Version = $release.tag_name -replace "^v", ""
    } catch {
        Exit-Fatal "Failed to query GitHub API: $_"
    }
}

$Version = $Version -replace "^v", ""
Write-Info "Version: $Version"

# -- Download -----------------------------------------------------------------

Write-Header "Downloading tgcryptfs v$Version"

$ArchiveName = "tgcryptfs-v${Version}-windows-${Arch}.zip"
$ArchiveUrl = "$GitHubReleases/download/v${Version}/$ArchiveName"
$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) "tgcryptfs-install-$([guid]::NewGuid().ToString('N').Substring(0,8))"
New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
$ArchivePath = Join-Path $TempDir $ArchiveName

try {
    Write-Info "Downloading $ArchiveUrl"
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -Uri $ArchiveUrl -OutFile $ArchivePath -UseBasicParsing
    Write-Ok "Download complete."
} catch {
    Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
    Exit-Fatal "Download failed: $_"
}

# -- Verify checksum ----------------------------------------------------------

$ChecksumUrl = "$GitHubReleases/download/v${Version}/checksums-sha256.txt"
$ChecksumPath = Join-Path $TempDir "checksums-sha256.txt"

try {
    Invoke-WebRequest -Uri $ChecksumUrl -OutFile $ChecksumPath -UseBasicParsing
    $checksumLines = Get-Content $ChecksumPath
    $expectedLine = $checksumLines | Where-Object { $_ -match $ArchiveName }
    if ($expectedLine) {
        $expectedHash = ($expectedLine -split "\s+")[0]
        $actualHash = (Get-FileHash -Path $ArchivePath -Algorithm SHA256).Hash.ToLower()
        if ($actualHash -ne $expectedHash.ToLower()) {
            Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
            Exit-Fatal "Checksum mismatch! Expected: $expectedHash, Got: $actualHash"
        }
        Write-Ok "Checksum verified."
    } else {
        Write-Warn "Archive not found in checksum file; skipping verification."
    }
} catch {
    Write-Warn "Checksum file not available; skipping verification."
}

# -- Extract ------------------------------------------------------------------

Write-Info "Extracting archive..."
$ExtractDir = Join-Path $TempDir "extracted"
Expand-Archive -Path $ArchivePath -DestinationPath $ExtractDir -Force

$ExtractedBin = Get-ChildItem -Path $ExtractDir -Recurse -Filter $BinaryName | Select-Object -First 1
if (-not $ExtractedBin) {
    Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue
    Exit-Fatal "Binary '$BinaryName' not found in archive."
}

# -- Install binary -----------------------------------------------------------

Write-Header "Installing binary"

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$DestBin = Join-Path $InstallDir $BinaryName
Copy-Item -Path $ExtractedBin.FullName -Destination $DestBin -Force
Write-Ok "Binary installed to $DestBin"

# -- Add to PATH --------------------------------------------------------------

Write-Header "Configuring PATH"

$userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($userPath -split ";" | Where-Object { $_ -eq $InstallDir }) {
    Write-Ok "$InstallDir is already on PATH."
} else {
    Write-Info "Adding $InstallDir to user PATH..."
    $newPath = "$InstallDir;$userPath"
    [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
    # Also update current session
    $env:PATH = "$InstallDir;$env:PATH"
    Write-Ok "Added $InstallDir to user PATH."
    Write-Info "Restart your terminal for PATH changes to take full effect."
}

# -- Config directory ---------------------------------------------------------

Write-Header "Setting up configuration"

if (-not (Test-Path $ConfigDir)) {
    New-Item -ItemType Directory -Path $ConfigDir -Force | Out-Null
}
Write-Ok "Config directory: $ConfigDir"

# -- Cleanup ------------------------------------------------------------------

Remove-Item -Path $TempDir -Recurse -Force -ErrorAction SilentlyContinue

# -- Verify -------------------------------------------------------------------

Write-Header "Verifying installation"

try {
    $versionOutput = & $DestBin --version 2>&1
    Write-Ok "tgcryptfs is installed."
    Write-Info "Version: $versionOutput"
} catch {
    Write-Warn "Could not verify installation. The binary may require a terminal restart."
}

# -- Done ---------------------------------------------------------------------

Write-Header "Installation complete"
Write-Host ""
Write-Info "Quick start:"
Write-Info "  tgcryptfs setup-telegram     # Configure Telegram API credentials"
Write-Info "  tgcryptfs auth login          # Authenticate with Telegram"
Write-Info "  tgcryptfs volume create myfs  # Create an encrypted volume"
Write-Info "  tgcryptfs mount myfs X:\      # Mount the volume (requires WinFsp)"
Write-Host ""
Write-Info "Documentation: https://github.com/$Repo#readme"
Write-Host ""
