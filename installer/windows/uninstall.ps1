$ErrorActionPreference = "SilentlyContinue"

$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\superCLI"
$DesktopShortcut = Join-Path ([Environment]::GetFolderPath("Desktop")) "superCLI.lnk"
$StartShortcut = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\superCLI.lnk"

if (Test-Path $DesktopShortcut) { Remove-Item $DesktopShortcut -Force }
if (Test-Path $StartShortcut) { Remove-Item $StartShortcut -Force }
if (Test-Path $InstallDir) { Remove-Item $InstallDir -Recurse -Force }

$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not [string]::IsNullOrEmpty($UserPath)) {
    $Filtered = ($UserPath.Split(';') | Where-Object { $_ -and ($_.ToLower() -ne $InstallDir.ToLower()) }) -join ';'
    [Environment]::SetEnvironmentVariable("Path", $Filtered, "User")
}

Write-Host "superCLI desinstalado correctamente." -ForegroundColor Yellow
