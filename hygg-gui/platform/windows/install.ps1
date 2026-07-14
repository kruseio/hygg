# Install hygg-gui on Windows and register it as a document handler for PDF /
# EPUB / text, so it shows up under "Open with" and can be set as the default
# reader. Writes to HKCU (per-user, no admin needed).
#
# Usage:  powershell -ExecutionPolicy Bypass -File platform\windows\install.ps1
param([switch]$Default)

$ErrorActionPreference = "Stop"
$here  = Split-Path -Parent $MyInvocation.MyCommand.Definition
$crate = Resolve-Path (Join-Path $here "..\..")
$root  = Resolve-Path (Join-Path $crate "..")

Write-Host "> building hygg-gui (release)..."
Push-Location $root
cargo build -p hygg-gui --release
Pop-Location

# Install the binary under %LOCALAPPDATA%\Programs\hygg.
$installDir = Join-Path $env:LOCALAPPDATA "Programs\hygg"
New-Item -ItemType Directory -Force -Path $installDir | Out-Null
$exe = Join-Path $installDir "hygg-gui.exe"
Copy-Item (Join-Path $root "target\release\hygg-gui.exe") $exe -Force
Write-Host "  installed -> $exe"

# Register a ProgID that opens the document passed as argv[1].
$progId = "hygg.Document"
$cmd    = "`"$exe`" `"%1`""
New-Item -Path "HKCU:\Software\Classes\$progId\shell\open\command" -Force | Out-Null
Set-ItemProperty -Path "HKCU:\Software\Classes\$progId" -Name "(default)" -Value "hygg document"
Set-ItemProperty -Path "HKCU:\Software\Classes\$progId\shell\open\command" -Name "(default)" -Value $cmd

# Advertise support for each extension under "Open with" (OpenWithProgIds).
$exts = @(".pdf", ".epub", ".txt", ".md", ".markdown")
foreach ($ext in $exts) {
  $key = "HKCU:\Software\Classes\$ext\OpenWithProgIds"
  New-Item -Path $key -Force | Out-Null
  Set-ItemProperty -Path $key -Name $progId -Value ([byte[]]@()) -Type Binary
  if ($Default) {
    # Best-effort default association (Windows may still prompt the user to
    # confirm the change via the Settings UI).
    Set-ItemProperty -Path "HKCU:\Software\Classes\$ext" -Name "(default)" -Value $progId
  }
}

Write-Host "OK. hygg is now available under 'Open with' for: $($exts -join ', ')"
Write-Host "Set it as the default: right-click a PDF -> Open with -> Choose another app -> hygg -> Always."
# The version, publisher, copyright, and commit are embedded in the exe by
# build.rs (VERSIONINFO resource) — Windows' equivalent of the macOS About
# panel. The full, interactive About / Credits screens are in Settings -> About.
Write-Host "About this build: right-click hygg-gui.exe -> Properties -> Details, or open Settings -> About in the app."
