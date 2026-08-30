# Install Pulse on Windows from a prebuilt release.
#
# Windows has no setuid; elevation is handled by UAC. Run an elevated
# (Administrator) PowerShell for a system install into "Program Files\Pulse"
# added to the system PATH; otherwise Pulse installs into
# %LOCALAPPDATA%\Pulse\bin on your user PATH.
#
#   ./install.ps1 [-Channel stable|beta|dev] [-User]
#
param(
    [ValidateSet("stable", "beta", "dev")]
    [string]$Channel = "stable",
    [switch]$User
)

$ErrorActionPreference = "Stop"
$Owner = "Hexadecimall"
$Repo = "Pulse-Package-Manager"
$Asset = "pulse-windows-x64.zip"

# Resolve the download URL for the channel.
switch ($Channel) {
    "stable" { $Url = "https://github.com/$Owner/$Repo/releases/latest/download/$Asset" }
    "dev"    { $Url = "https://github.com/$Owner/$Repo/releases/download/dev/$Asset" }
    "beta" {
        $releases = Invoke-RestMethod "https://api.github.com/repos/$Owner/$Repo/releases?per_page=30"
        $tag = ($releases | Where-Object { $_.prerelease -and $_.tag_name -match "beta" } | Select-Object -First 1).tag_name
        if (-not $tag) { throw "no beta release is available yet" }
        $Url = "https://github.com/$Owner/$Repo/releases/download/$tag/$Asset"
    }
}

# Elevated + not --User => system install; otherwise user install.
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltinRole]::Administrator)
if ($isAdmin -and -not $User) {
    $InstallDir = Join-Path $env:ProgramFiles "Pulse"
    $PathScope = "Machine"
    $ModeName = "system"
} else {
    $InstallDir = Join-Path $env:LOCALAPPDATA "Pulse\bin"
    $PathScope = "User"
    $ModeName = "user"
}

$Tmp = Join-Path $env:TEMP ("pulse-" + [System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Force -Path $Tmp | Out-Null
try {
    Write-Host "pulse: downloading $Asset ($Channel)..."
    $zip = Join-Path $Tmp $Asset
    Invoke-WebRequest -Uri $Url -OutFile $zip
    Expand-Archive -Path $zip -DestinationPath $Tmp -Force

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item -Force (Join-Path $Tmp "pulse.exe") (Join-Path $InstallDir "pulse.exe")

    # Add the install dir to the appropriate PATH if it isn't already there.
    $current = [Environment]::GetEnvironmentVariable("Path", $PathScope)
    if ($current -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$InstallDir;$current", $PathScope)
    }

    # Record the install mode for the running binary's default.
    $pulseHome = Join-Path $env:USERPROFILE ".pulse"
    New-Item -ItemType Directory -Force -Path $pulseHome | Out-Null
    "install_mode = `"$ModeName`"" | Set-Content -Path (Join-Path $pulseHome "config")
} finally {
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "Installed $(Join-Path $InstallDir 'pulse.exe') ($ModeName mode)"
Write-Host "Open a new terminal for the updated PATH to take effect."
