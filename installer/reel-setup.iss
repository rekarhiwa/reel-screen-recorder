; REEL — Kurdish screen recorder installer
; Info pages are UTF-16 LE (BOM) so Kurdish renders correctly in the wizard.

#define MyAppName "ڕیل"
#define MyAppNameEn "REEL Screen Recorder"
#define MyAppVersion "0.3.0"
#define MyAppPublisher "REEL"
#define MyAppURL "https://github.com/rekarhiwa/reel-screen-recorder"
#define MyAppExeName "screen-recorder.exe"
#define MyAppId "REEL-ScreenRecorder-Kurdish"

[Setup]
AppId={#MyAppId}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={localappdata}\REEL
DefaultGroupName=REEL
DisableProgramGroupPage=yes
DisableWelcomePage=no
InfoBeforeFile=pages\about-ckb.rtf
LicenseFile=pages\features-ckb.rtf
InfoAfterFile=pages\howto-ckb.rtf
OutputDir=..\dist
OutputBaseFilename=REEL-Setup-{#MyAppVersion}
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName=ڕیل — سکرین ڕیکۆردەر
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
VersionInfoVersion={#MyAppVersion}
VersionInfoCompany={#MyAppPublisher}
VersionInfoDescription={#MyAppNameEn}
VersionInfoProductName={#MyAppName}
CloseApplications=yes
RestartApplications=no
ShowLanguageDialog=no

[Languages]
Name: "kurdish"; MessagesFile: "compiler:Default.isl"

[LangOptions]
LanguageName=Kurdish
LanguageID=$0401
LanguageCodePage=0
DialogFontName=Segoe UI
DialogFontSize=9
WelcomeFontName=Segoe UI
WelcomeFontSize=12
RightToLeft=yes

[Messages]
kurdish.WelcomeLabel1=بەخێربێیت بۆ دامەزراندنی ڕیل
kurdish.WelcomeLabel2=ئەم ویزاردە ڕیل (سکرین ڕیکۆردەری کوردی) لەسەر کۆمپیوتەرەکەت دادەمەزرێنێت.%n%nپێش بەردەوامبوون، تکایە هەموو بەرنامەکانی تر دابخە. پاشان «دواتر» داگرە.
kurdish.ClickNext=بۆ بەردەوامبوون «دواتر» داگرە.
kurdish.ButtonNext=&دواتر >
kurdish.ButtonBack=< &پاشەوە
kurdish.ButtonCancel=هەڵوەشاندنەوە
kurdish.ButtonInstall=&دامەزراندن
kurdish.ButtonFinish=&تەواو
kurdish.SelectDirLabel3=ڕیل لەم فۆڵدەرە دادەمەزرێت.
kurdish.SelectDirBrowseLabel=بۆ بەردەوامبوون «دواتر» داگرە. ئەگەر دەتەوێت فۆڵدەر بگۆڕیت، «گەڕان» داگرە.
kurdish.BrowseDialogTitle=فۆڵدەری دامەزراندن هەڵبژێرە
kurdish.DiskSpaceMBLabel=[kb] ک.ب بۆشایی پێویستە.
kurdish.ReadyLabel1=ئامادەیە بۆ دامەزراندنی ڕیل لەسەر کۆمپیوتەرەکەت.
kurdish.ReadyLabel2a=«دامەزراندن» داگرە بۆ دەستپێکردنی دامەزراندن، یان «پاشەوە» بۆ پێداچوونەوەی ڕێکخستنەکان.
kurdish.ReadyLabel2b=«دامەزراندن» داگرە بۆ دەستپێکردنی دامەزراندن.
kurdish.InstallingLabel=تکایە چاوەڕوان بە تا ڕیل دادەمەزرێت…
kurdish.FinishedHeadingLabel=دامەزراندنی ڕیل تەواو بوو
kurdish.FinishedLabelNoIcons=ڕیل بە سەرکەوتوویی لەسەر کۆمپیوتەرەکەت دامەزرا.
kurdish.FinishedLabel=ڕیل بە سەرکەوتوویی دامەزرا. دەتوانیت لە ئایکۆنەکانی دامەزراو بەکاری بهێنیت.
kurdish.ClickFinish=بۆ داخستنی ویزارد «تەواو» داگرە.
kurdish.InfoBeforeLabel=تکایە ئەم زانیارییە بخوێنەوە پێش دامەزراندنی ڕیل، پاشان «دواتر» داگرە.
kurdish.InfoAfterLabel=دامەزراندن تەواو بوو. تکایە ئەم ڕێنماییە بخوێنەوە، پاشان «تەواو» داگرە.
kurdish.WizardInfoBefore=دەربارەی ڕیل
kurdish.WizardInfoAfter=چۆن بەکاری بهێنیت
kurdish.WizardLicense=تایبەتمەندییەکان
kurdish.LicenseLabel=تایبەتمەندییە سەرەکییەکانی ڕیل بخوێنەوە، پاشان «دواتر» داگرە.
kurdish.LicenseLabel3=تکایە تایبەتمەندییەکان بخوێنەوە، پاشان «دواتر» داگرە.
kurdish.LicenseAccepted=خوێندمەوە — بەردەوام دەبم
kurdish.LicenseNotAccepted=نا مەوێت بەردەوام بم
kurdish.WizardReady=ئامادە بۆ دامەزراندن
kurdish.WizardInstalling=خەریکی دامەزراندن
kurdish.WizardSelectDir=شوێنی دامەزراندن
kurdish.SelectTasksLabel2=ئەو ئەرکانە هەڵبژێرە کە دەتەوێت Setup ئەنجامیان بدات، پاشان «دواتر» داگرە.
kurdish.WizardSelectTasks=هەڵبژاردنی ئەرکەکان
kurdish.StatusExtractFiles=دەرکردنی فایلەکان…
kurdish.StatusCreateIcons=دروستکردنی ئایکۆنەکان…
kurdish.ConfirmUninstall=دڵنیایت دەتەوێت %1 و هەموو پێکهاتەکانی لاببەیت؟
kurdish.UninstallAppFullTitle=لابردنی %1
kurdish.UninstalledAll=%1 بە تەواوی لە کۆمپیوتەرەکەت لابرا.
kurdish.ExitSetupTitle=دەرچوون لە Setup؟
kurdish.ExitSetupMessage=دامەزراندن تەواو نەبووە. ئەگەر ئێستا دەربچیت، ڕیل دادەمەزرێت نییە.%n%nدواتر دەتوانیت Setup دووبارە بکەیتەوە.%n%nدەرچوون؟

[CustomMessages]
kurdish.CreateDesktopIcon=ئایکۆنی Desktop دروست بکە
kurdish.AdditionalIcons=ئایکۆنی زیادە:
kurdish.LaunchAfter=کردنەوەی ڕیل دوای دامەزراندن
kurdish.NameAndVersion=%1 وەشانی %2

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: checkedonce

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\icon.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\ڕیل"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\icon.ico"; Comment: "ڕیل — سکرین ڕیکۆردەر"
Name: "{group}\لابردنی ڕیل"; Filename: "{uninstallexe}"
Name: "{autodesktop}\ڕیل"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\icon.ico"; WorkingDir: "{app}"; Comment: "ڕیل — سکرین ڕیکۆردەر"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchAfter}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: files; Name: "{app}\reel-settings.json"
