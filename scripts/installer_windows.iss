; Unsigned, per-user development preview; compiler supplied by the Windows runner.
; PayloadDir contains only files admitted by package_build.py.
[Setup]
AppId=Youmu.YM_Link.Preview.x64
AppName=YM_Link
AppVersion={#AppVersion}
AppVerName=YM_Link {#AppVersion} Development Preview ({#Revision})
AppPublisher=Youmu
AppPublisherURL=https://youmu.forrestzhu.com/
AppSupportURL=https://github.com/forrestzhu/youmu-link/issues
DefaultDirName={localappdata}\Programs\Youmu\YM_Link
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0.18362
LicenseFile={#PayloadDir}\LICENSE
OutputDir={#OutputDir}
OutputBaseFilename={#OutputName}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\YM_Link.exe
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "{#PayloadDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{userprograms}\Youmu\YM_Link"; Filename: "{app}\YM_Link.exe"
