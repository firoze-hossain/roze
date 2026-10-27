; ==============================================================================
; Inno Setup Script for Roze Language & Toolchain
; Builds a modern Windows installer wizard (.exe)
; ==============================================================================

#ifndef MyAppVersion
  #define MyAppVersion "1.0.0"
#endif

#define MyAppName "Roze"
#define MyAppPublisher "Roze Ecosystem"
#define MyAppURL "https://github.com/firoze-hossain/roze"
#define MyAppExeName "roze.exe"

[Setup]
AppId={{9E2A8C14-3B5D-4F70-A6E1-83B927D4501F}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
SourceDir=..\..
OutputDir=dist
OutputBaseFilename=Roze-Setup-{#MyAppVersion}
SetupIconFile=packaging\windows\Roze.ico
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
DisableDirPage=no
DisableProgramGroupPage=no
ChangesEnvironment=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "addtopath"; Description: "Add Roze compiler & tools to PATH"; GroupDescription: "Environment Settings:"; Flags: checkedonce
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "dist\roze-win-bin\bin\*"; DestDir: "{app}\bin"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "dist\roze-win-bin\stdlib\*"; DestDir: "{app}\stdlib"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "packaging\windows\Roze.ico"; DestDir: "{app}"; Flags: ignoreversion

[Registry]
; Add {app}\bin to user PATH when task is selected
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}\bin"; Tasks: addtopath; Check: NeedsAddPath(ExpandConstant('{app}\bin'))

[Icons]
Name: "{group}\Roze Documentation"; Filename: "{#MyAppURL}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"

[UninstallDelete]
Type: filesandordirs; Name: "{app}"

[Code]
function NeedsAddPath(Param: string): boolean;
var
  OrigPath: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath)
  then begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + UpperCase(Param) + ';', ';' + UpperCase(OrigPath) + ';') = 0;
end;
