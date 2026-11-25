param(
    [string]$SourceExe = ""
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($SourceExe)) {
    $candidates = @(
        (Join-Path $PSScriptRoot "supercli.exe"),
        (Join-Path $PSScriptRoot "..\..\target\release\supercli.exe")
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) {
            $SourceExe = (Resolve-Path $c).Path
            break
        }
    }
}

if ([string]::IsNullOrWhiteSpace($SourceExe) -or -not (Test-Path $SourceExe)) {
    Write-Error "No se encontro supercli.exe. Compila primero con 'cargo build --release' o coloca supercli.exe junto a este script."
    exit 1
}

$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\superCLI"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

$DestExe = Join-Path $InstallDir "supercli.exe"
Copy-Item -Path $SourceExe -Destination $DestExe -Force

$ReadmeCandidates = @(
    (Join-Path $PSScriptRoot "README.md"),
    (Join-Path $PSScriptRoot "..\..\README.md")
)
foreach ($r in $ReadmeCandidates) {
    if (Test-Path $r) {
        Copy-Item -Path $r -Destination (Join-Path $InstallDir "README.md") -Force
        break
    }
}

# Create Desktop and Start Menu shortcuts
$WshShell = New-Object -ComObject WScript.Shell

$DesktopPath = [Environment]::GetFolderPath("Desktop")
$DesktopShortcut = $WshShell.CreateShortcut((Join-Path $DesktopPath "superCLI.lnk"))
$DesktopShortcut.TargetPath = $DestExe
$DesktopShortcut.WorkingDirectory = $env:USERPROFILE
$DesktopShortcut.Description = "superCLI - Editor de codigo ligero para Agentes CLI"
$DesktopShortcut.IconLocation = "$DestExe,0"
$DesktopShortcut.Save()

$StartMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
$StartShortcut = $WshShell.CreateShortcut((Join-Path $StartMenuDir "superCLI.lnk"))
$StartShortcut.TargetPath = $DestExe
$StartShortcut.WorkingDirectory = $env:USERPROFILE
$StartShortcut.Description = "superCLI - Editor de codigo ligero para Agentes CLI"
$StartShortcut.IconLocation = "$DestExe,0"
$StartShortcut.Save()

# Add InstallDir to User PATH if not present
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ([string]::IsNullOrEmpty($UserPath)) {
    [Environment]::SetEnvironmentVariable("Path", $InstallDir, "User")
} elseif ($UserPath.ToLower().Split(';') -notcontains $InstallDir.ToLower()) {
    $NewPath = ($UserPath.TrimEnd(';') + ";" + $InstallDir)
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
}

Write-Host "superCLI instalado correctamente en: $InstallDir" -ForegroundColor Green
Write-Host "Acceso directo creado en el Escritorio: $(Join-Path $DesktopPath 'superCLI.lnk')" -ForegroundColor Cyan
Write-Host "Acceso directo creado en el Menu Inicio." -ForegroundColor Cyan
Write-Host "Tambien puedes ejecutar 'supercli .' desde cualquier nueva terminal." -ForegroundColor Cyan
