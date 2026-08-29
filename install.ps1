# Build Pulse and install it on Windows.
#
# Windows has no setuid. Elevation is handled by UAC instead: winget requests
# Administrator itself when a package needs it, and per-user installs need no
# elevation at all. So this script just builds and drops the binary somewhere on
# PATH — no privileged install step.
#
# Usage:  ./install.ps1

$ErrorActionPreference = "Stop"

$RepoDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\pulse"
$PulseBin = Join-Path $env:USERPROFILE ".pulse\bin"

Write-Host "pulse: building release..."
Push-Location $RepoDir
try {
    cargo build --release
} finally {
    Pop-Location
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Copy-Item -Force (Join-Path $RepoDir "target\release\pulse.exe") (Join-Path $InstallDir "pulse.exe")

# Put both the install dir and Pulse's own bin dir on the user's PATH.
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
foreach ($dir in @($InstallDir, $PulseBin)) {
    if ($userPath -notlike "*$dir*") {
        $userPath = "$dir;$userPath"
    }
}
[Environment]::SetEnvironmentVariable("Path", $userPath, "User")

Write-Host ""
Write-Host "Installed $(Join-Path $InstallDir 'pulse.exe')"
Write-Host "Open a new terminal for the updated PATH to take effect."
