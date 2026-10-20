#define AppVersion GetEnv("PANDA_APP_VERSION")
#define AppExePath GetEnv("PANDA_EXE")
#define InstallerOutput GetEnv("PANDA_INSTALLER_OUTPUT")

[Setup]
AppId={{12D7D8FD-A86A-4EA7-AB00-5DE5C1F8C5A1}
AppName=Panda Note
AppVersion={#AppVersion}
AppPublisher=Panda Note
DefaultDirName={localappdata}\Programs\Panda Note
DefaultGroupName=Panda Note
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir={#InstallerOutput}
OutputBaseFilename=PandaNoteSetup-{#AppVersion}-windows-x64
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\panda.exe
ChangesEnvironment=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts"; Flags: unchecked
Name: "addtopath"; Description: "Add Panda Note CLI to your user PATH"; GroupDescription: "Command-line integration"; Flags: checkedonce

[Files]
Source: "{#AppExePath}"; DestDir: "{app}"; DestName: "panda.exe"; Flags: ignoreversion

[Icons]
Name: "{group}\Panda Note"; Filename: "{app}\panda.exe"
Name: "{autodesktop}\Panda Note"; Filename: "{app}\panda.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\panda.exe"; Description: "Launch Panda Note"; Flags: nowait postinstall skipifsilent

[Code]
procedure AddInstallDirToPath();
var
  UserPath, InstallDir: string;
begin
  InstallDir := ExpandConstant('{app}');
  UserPath := '';
  RegQueryStringValue(HKCU, 'Environment', 'Path', UserPath);
  if Pos(';' + Lowercase(InstallDir) + ';', ';' + Lowercase(UserPath) + ';') = 0 then begin
    if UserPath <> '' then
      UserPath := UserPath + ';';
    UserPath := UserPath + InstallDir;
    RegWriteExpandStringValue(HKCU, 'Environment', 'Path', UserPath);
  end;
end;

procedure RemoveInstallDirFromPath();
var
  UserPath, InstallDir, WrappedPath: string;
begin
  UserPath := '';
  InstallDir := ExpandConstant('{app}');
  if RegQueryStringValue(HKCU, 'Environment', 'Path', UserPath) then begin
    WrappedPath := ';' + UserPath + ';';
    while StringChangeEx(WrappedPath, ';' + InstallDir + ';', ';', True) > 0 do;
    if (Length(WrappedPath) > 0) and (WrappedPath[1] = ';') then
      Delete(WrappedPath, 1, 1);
    if (Length(WrappedPath) > 0) and (WrappedPath[Length(WrappedPath)] = ';') then
      Delete(WrappedPath, Length(WrappedPath), 1);
    RegWriteExpandStringValue(HKCU, 'Environment', 'Path', WrappedPath);
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
    AddInstallDirToPath();
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    RemoveInstallDirFromPath();
end;
