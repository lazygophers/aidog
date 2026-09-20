; AiDog Flutter 壳的 Windows 安装包（票 I13）。CI 里由 Inno Setup 编译：
;   iscc /DMyAppVersion=<版本> flutter\packaging\flutter_windows.iss
; 产物：flutter\build\installer\AiDog-Flutter-v<版本>-x64-setup.exe
; 该 exe 就是 WinSparkle appcast 里的 enclosure —— WinSparkle 对 Inno Setup 安装器
; 自带静默参数（/SILENT 等），无需 sparkle:installerArguments。

#define MyAppName "AiDog"
#define MyAppVersion "0.0.0"
#define MyAppPublisher "lazygophers"
#define MyAppExeName "aidog_flutter.exe"

[Setup]
AppId={{7C4A6C1E-9D2B-4E8F-A5D3-AIDOGFLUTTER}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\{#MyAppName}
DisableProgramGroupPage=yes
; 与老 Tauri 壳同一个应用槽位：跨栈升级 = 原地替换，回滚 = 重装上一个 Tauri 安装包。
UsedUserAreasWarning=no
OutputDir=..\build\installer
OutputBaseName=AiDog-Flutter-v{#MyAppVersion}-x64-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64compatible
ArchitecturesAllowed=x64compatible
UninstallDisplayIcon={app}\{#MyAppExeName}

[Files]
; flutter build windows --release 的输出（含 aidog-kernel.exe，由 CMake install 阶段落位）。
Source: "..\build\windows\x64\runner\Release\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent
