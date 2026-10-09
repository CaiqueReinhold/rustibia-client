; Inno Setup script for the Rustibia client.
;
; Not meant to be run by hand -- packaging/build-windows.sh stages the files and
; invokes ISCC with the /D defines below. If you do run it manually, make sure
; `stage/` sits next to this file and holds the exe plus the assets/ tree.

#define AppName "Rustibia"
#define AppExeName "rustibia-client.exe"
#define AppPublisher "Caique Reinhold"

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef StageDir
  #define StageDir "stage"
#endif
#ifndef OutputDir
  #define OutputDir "."
#endif

[Setup]
; Keep this GUID stable forever -- it is how Windows recognises an upgrade of an
; existing install instead of a second, parallel one.
AppId={{83D14E74-0D29-40A9-AAF7-53698EAB357E}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
UninstallDisplayIcon={app}\{#AppExeName}
OutputDir={#OutputDir}
OutputBaseFilename={#AppName}Setup-{#AppVersion}
Compression=lzma2/max
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Default to a per-user install so testers without an admin account can still
; install; the dialog lets them pick machine-wide if they can elevate.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
DisableProgramGroupPage=yes
WizardStyle=modern
#if FileExists(AddBackslash(SourcePath) + "icon.ico")
SetupIconFile={#AddBackslash(SourcePath)}icon.ico
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "{#StageDir}\{#AppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\assets\*"; DestDir: "{app}\assets"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"; WorkingDir: "{app}"
Name: "{group}\{cm:UninstallProgram,{#AppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExeName}"; WorkingDir: "{app}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
