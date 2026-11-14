; Inno Setup script for Codelab. Lives in installer-i\, so paths go up one level (..\).
; Build first:  cargo build --release   then compile this script (ISCC.exe codelab.iss).

#define MyAppName "Codelab"
#define MyAppVersion "0.1.0"
#define MyAppExe "Codelab.exe"

[Setup]
; Keep this GUID forever: it is how upgrades find the previous install.
AppId={{426D9821-9106-478C-BC95-5035FE6AD1DB}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher=Tinnth7
AppPublisherURL=https://github.com/Tinnth7/codelab
AppSupportURL=https://github.com/Tinnth7/codelab/issues
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputDir=..\installer-o
OutputBaseFilename=CodeLab
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#MyAppExe}
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional shortcuts:"

[Files]
; The only file you need: fonts, icons and the tree-sitter grammars are compiled into the exe.
Source: "..\target\release\codelab.exe"; DestDir: "{app}"; DestName: "{#MyAppExe}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExe}"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent
