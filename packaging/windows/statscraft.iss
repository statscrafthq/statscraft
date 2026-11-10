; Per-user Windows installer for StatsCraft, built with Inno Setup 6.
; From the repository root, after cargo build --release -p statscraft -p statscraft-gui:
;   iscc /DAppVersion=0.1.0 packaging\windows\statscraft.iss
; The installer lands in target\packages\.
;
; No administrator rights: everything goes to the user's own program folder and registry.
;
; The updater in StatsCraft runs this installer as
;   setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS /RELAUNCH=1
; It installs over the earlier version in the same folder (the AppId below and
; UsePreviousAppDir), rewrites the same file types, and with /RELAUNCH=1 starts the window again.

#ifndef AppVersion
  #error Pass the version, for example: iscc /DAppVersion=0.1.0 packaging\windows\statscraft.iss
#endif

[Setup]
; Keep this id: Windows uses it to recognise upgrades and the uninstaller.
AppId={{249E004F-824E-428E-87A6-28EB9450BDEA}
AppName=StatsCraft
AppVersion={#AppVersion}
AppPublisher=StatsCraft
AppPublisherURL=https://github.com/statscrafthq/statscraft
; paths below are relative to the repository root
SourceDir=..\..
OutputDir=target\packages
OutputBaseFilename=statscraft-{#AppVersion}-windows-x86_64-setup
; with lowest privileges {autopf} is %LOCALAPPDATA%\Programs and {autoprograms} the user's Start menu
PrivilegesRequired=lowest
DefaultDirName={autopf}\StatsCraft
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesAssociations=yes
ChangesEnvironment=yes
SetupIconFile=crates\gui\icons\statscraft.ico
UninstallDisplayIcon={app}\statscraft-gui.exe
UninstallDisplayName=StatsCraft
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
; an update goes where the earlier version is, without asking
UsePreviousAppDir=yes
; close a running window before its files are replaced. Restart Manager restarts only programs
; that registered with it, which the window does not, so [Run] starts it again on /RELAUNCH=1.
CloseApplications=yes
RestartApplications=no

[Files]
Source: "target\release\statscraft.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\release\statscraft-gui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\StatsCraft"; Filename: "{app}\statscraft-gui.exe"

[Registry]
; the command line on the user's PATH; the [Code] below takes it off again on uninstall
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath(ExpandConstant('{app}'))

; .sav and .zsav open in the window, .sps opens in its syntax editor
Root: HKA; Subkey: "Software\Classes\StatsCraft.Data"; ValueType: string; ValueName: ""; ValueData: "StatsCraft data file"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\StatsCraft.Data\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\statscraft-gui.exe,0"
Root: HKA; Subkey: "Software\Classes\StatsCraft.Data\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\statscraft-gui.exe"" ""%1"""
Root: HKA; Subkey: "Software\Classes\StatsCraft.Syntax"; ValueType: string; ValueName: ""; ValueData: "StatsCraft syntax file"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\StatsCraft.Syntax\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\statscraft-gui.exe,0"
Root: HKA; Subkey: "Software\Classes\StatsCraft.Syntax\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\statscraft-gui.exe"" ""%1"""

; StatsCraft is offered under Open with for each type
Root: HKA; Subkey: "Software\Classes\.sav\OpenWithProgids"; ValueType: string; ValueName: "StatsCraft.Data"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.zsav\OpenWithProgids"; ValueType: string; ValueName: "StatsCraft.Data"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.sps\OpenWithProgids"; ValueType: string; ValueName: "StatsCraft.Syntax"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\Applications\statscraft-gui.exe"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\statscraft-gui.exe\SupportedTypes"; ValueType: string; ValueName: ".sav"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\statscraft-gui.exe\SupportedTypes"; ValueType: string; ValueName: ".zsav"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\statscraft-gui.exe\SupportedTypes"; ValueType: string; ValueName: ".sps"; ValueData: ""

; and it becomes the default only where no other program is the default already
Root: HKA; Subkey: "Software\Classes\.sav"; ValueType: string; ValueName: ""; ValueData: "StatsCraft.Data"; Flags: uninsdeletevalue; Check: FreeOrOurs('.sav', 'StatsCraft.Data')
Root: HKA; Subkey: "Software\Classes\.zsav"; ValueType: string; ValueName: ""; ValueData: "StatsCraft.Data"; Flags: uninsdeletevalue; Check: FreeOrOurs('.zsav', 'StatsCraft.Data')
Root: HKA; Subkey: "Software\Classes\.sps"; ValueType: string; ValueName: ""; ValueData: "StatsCraft.Syntax"; Flags: uninsdeletevalue; Check: FreeOrOurs('.sps', 'StatsCraft.Syntax')

[Run]
Filename: "{app}\statscraft-gui.exe"; Description: "Start StatsCraft"; Flags: nowait postinstall skipifsilent
; after an update from the window, which passes /RELAUNCH=1
Filename: "{app}\statscraft-gui.exe"; Flags: nowait; Check: Relaunch

[Code]
// True when the updater asked for the window to be started again.
function Relaunch: Boolean;
begin
  Result := ExpandConstant('{param:RELAUNCH|0}') = '1';
end;

// Position of Dir in the user's Path, 0 when it is not there.
function PathPos(Paths, Dir: String): Integer;
begin
  Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Paths) + ';');
end;

function NeedsAddPath(Dir: String): Boolean;
var
  Paths: String;
begin
  if RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Paths) then
    Result := PathPos(Paths, Dir) = 0
  else
    Result := True;
end;

// True when the extension has no default program, or has ours from an earlier install.
function FreeOrOurs(Ext, ProgId: String): Boolean;
var
  Current: String;
begin
  if RegQueryStringValue(HKEY_CLASSES_ROOT, Ext, '', Current) then
    Result := (Current = '') or (Current = ProgId)
  else
    Result := True;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Paths, Dir: String;
  P: Integer;
begin
  if CurUninstallStep <> usPostUninstall then
    exit;
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Paths) then
    exit;
  Dir := ExpandConstant('{app}');
  P := PathPos(Paths, Dir);
  if P = 0 then
    exit;
  // P is where Dir starts in Paths; take it out with one of its separators
  if P = 1 then
    Delete(Paths, 1, Length(Dir) + 1)
  else
    Delete(Paths, P - 1, Length(Dir) + 1);
  RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Paths);
end;
