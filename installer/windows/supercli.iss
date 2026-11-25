#ifndef MyAppVersion
  #define MyAppVersion "0.1.1"
#endif

[Setup]
AppId={{8E2D7A4F-6F9C-4B28-9E15-2C8B6A9D4F11}
AppName=superCLI
AppVersion={#MyAppVersion}
AppPublisher=WillyBarrios
AppPublisherURL=https://github.com/WillyBarrios/superCLI
AppSupportURL=https://github.com/WillyBarrios/superCLI/issues
DefaultDirName={localappdata}\Programs\superCLI
DefaultGroupName=superCLI
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=..\..\dist
OutputBaseFilename=supercli-{#MyAppVersion}-windows-x86_64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ChangesEnvironment=yes

[Languages]
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Crear un acceso directo en el Escritorio"; GroupDescription: "Accesos directos:"
Name: "addtopath"; Description: "Agregar superCLI al PATH del usuario (para usar 'supercli .' en la terminal)"; GroupDescription: "Integración con terminal:"

[Files]
Source: "..\..\target\release\supercli.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\superCLI"; Filename: "{app}\supercli.exe"; WorkingDir: "{userdocs}"
Name: "{autodesktop}\superCLI"; Filename: "{app}\supercli.exe"; WorkingDir: "{userdocs}"; Tasks: desktopicon

[Run]
Filename: "{app}\supercli.exe"; Description: "Ejecutar superCLI al finalizar"; Flags: nowait postinstall skipifsilent

[Code]
procedure CurStepChanged(CurStep: TSetupStep);
var
  PathVal: string;
  AppDir: string;
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
  begin
    AppDir := ExpandConstant('{app}');
    if RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', PathVal) then
    begin
      if Pos(Lowercase(AppDir), Lowercase(PathVal)) = 0 then
      begin
        if (Length(PathVal) > 0) and (PathVal[Length(PathVal)] <> ';') then
          PathVal := PathVal + ';';
        PathVal := PathVal + AppDir;
        RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', PathVal);
      end;
    end
    else
    begin
      RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', AppDir);
    end;
  end;
end;
