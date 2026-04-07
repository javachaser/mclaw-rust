#ifndef MyAppVersion
  #define MyAppVersion "1.0.2"
#endif

#define MyAppName "MClaw"
#define MyAppPublisher "MClaw"
#define MyAppURL "http://127.0.0.1:19000"
#define MyAppExeName "MClaw.exe"
#define MyBuildDir "..\\target\\x86_64-pc-windows-gnu\\release"
#define MyPayloadDir "payload"
#define MyAppIcon "..\\assets\\MClaw.ico"
#define WebView2Bootstrapper "MicrosoftEdgeWebview2Setup.exe"

[Setup]
AppId={{A5D40F31-7B4A-47D4-9932-4A6A5E8F5F63}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
DefaultDirName={localappdata}\Programs\MClaw
DisableDirPage=no
DefaultGroupName={#MyAppName}
SetupIconFile={#MyAppIcon}
UninstallDisplayIcon={app}\MClaw.ico
OutputDir=..\dist
OutputBaseFilename=MClaw-Setup-{#MyAppVersion}-x64
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

PrivilegesRequired=lowest
ChangesAssociations=no
DisableProgramGroupPage=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"


[Tasks]
Name: "desktopicon"; Description: "创建桌面快捷方式"; GroupDescription: "附加任务:"; Flags: unchecked

[Files]
Source: "{#MyBuildDir}\mclaw.exe"; DestDir: "{app}"; DestName: "{#MyAppExeName}"; Flags: ignoreversion
Source: "{#MyBuildDir}\WebView2Loader.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MyAppIcon}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MyPayloadDir}\{#WebView2Bootstrapper}"; DestDir: "{tmp}"; Flags: deleteafterinstall

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\MClaw.ico"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\MClaw.ico"; Tasks: desktopicon

[Run]
Filename: "{tmp}\{#WebView2Bootstrapper}"; Parameters: "/silent /install"; StatusMsg: "正在安装 Microsoft WebView2 Runtime..."; Flags: waituntilterminated runhidden; Check: NeedsWebView2Runtime
Filename: "{app}\{#MyAppExeName}"; Description: "启动 {#MyAppName}"; Flags: nowait postinstall skipifsilent

[Code]
const
  WebView2ClientGuid = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';

function GetWebView2Version(RootKey: Integer; const SubKey: string): string;
begin
  if not RegQueryStringValue(RootKey, SubKey, 'pv', Result) then
    Result := '';
end;

function HasInstalledWebView2Runtime: Boolean;
var
  Version: string;
begin
  Version := GetWebView2Version(HKLM64, 'SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\' + WebView2ClientGuid);
  if Version = '' then
    Version := GetWebView2Version(HKCU, 'Software\\Microsoft\\EdgeUpdate\\Clients\\' + WebView2ClientGuid);
  if Version = '' then
    Version := GetWebView2Version(HKLM, 'SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\' + WebView2ClientGuid);
  if Version = '' then
    Version := GetWebView2Version(HKCU, 'Software\\Microsoft\\EdgeUpdate\\Clients\\' + WebView2ClientGuid);

  Result := (Version <> '') and (Version <> '0.0.0.0');
  if Result then
    Log(Format('Detected WebView2 Runtime version: %s', [Version]))
  else
    Log('WebView2 Runtime not detected.');
end;

function NeedsWebView2Runtime: Boolean;
begin
  Result := not HasInstalledWebView2Runtime;
end;
