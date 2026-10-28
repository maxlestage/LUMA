; L'installateur de LÜMA pour Windows (Inno Setup 6), fabriqué par
; scripts/bundle-windows.ps1 : iscc /DVersion=… /DSource=<dossier de l'app> luma.iss
;
; Pour vous seul, sans mot de passe administrateur (dans %LOCALAPPDATA%\Programs) ; un
; raccourci dans le menu Démarrer, un sur le bureau si vous le voulez, et LÜMA dans
; « Ajouter ou supprimer des programmes ». Vos réglages (%APPDATA%\LÜMA) restent si
; vous désinstallez.

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Source
  #define Source "..\..\target\windows\LUMA-windows-x86_64"
#endif

[Setup]
AppId={{6B3E2F0A-2C1D-4E8B-9F57-1A4C7D0E5B92}
AppName=LÜMA
AppVersion={#Version}
AppVerName=LÜMA {#Version}
AppPublisher=Maxime Nathan Lestage
AppPublisherURL=https://maxlestage.github.io/LUMA/
DefaultDirName={autopf}\LÜMA
DefaultGroupName=LÜMA
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputBaseFilename=LUMA-windows-x86_64-setup
SetupIconFile={#Source}\bin\luma.ico
UninstallDisplayIcon={app}\luma.ico
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ChangesAssociations=no

[Languages]
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Source}\bin\luma-studio.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\bin\luma.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\bin\luma.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Source}\LISEZMOI.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\LÜMA"; Filename: "{app}\luma-studio.exe"; IconFilename: "{app}\luma.ico"
Name: "{autodesktop}\LÜMA"; Filename: "{app}\luma-studio.exe"; IconFilename: "{app}\luma.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\luma-studio.exe"; Description: "{cm:LaunchProgram,LÜMA}"; Flags: nowait postinstall skipifsilent
