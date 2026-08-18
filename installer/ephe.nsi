; ponytail: minimum viable installer. LZMA-solid. Logo.ico drives every icon.

Unicode True
SetCompressor /SOLID lzma
SetCompressorDictSize 64
SetDatablockOptimize on

Name "ephe"
OutFile "..\release\ephe-setup.exe"
InstallDir "$LOCALAPPDATA\ephe"
RequestExecutionLevel user
Icon "Logo.ico"
UninstallIcon "Logo.ico"

!include "MUI2.nsh"

!define MUI_ABORTWARNING
!define MUI_ICON "Logo.ico"
!define MUI_UNICON "Logo.ico"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Install" SecInstall
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "ephe.exe"
  File "Logo.png"
  File "text-logo.png"
  File "PressStart2P-Regular.ttf"
  File "Logo.ico"
  WriteUninstaller "$INSTDIR\uninst.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ephe" "DisplayName" "ephe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ephe" "DisplayIcon" "$INSTDIR\Logo.ico,0"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ephe" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ephe" "UninstallString" "$\"$INSTDIR\uninst.exe$\""
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ephe" "Publisher" "ephe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ephe" "DisplayVersion" "0.10.0"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ephe" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ephe" "NoRepair" 1
SectionEnd

Section "Start menu shortcut"
  CreateDirectory "$SMPROGRAMS\ephe"
  CreateShortcut "$SMPROGRAMS\ephe\ephe.lnk" "$INSTDIR\ephe.exe" "" "$INSTDIR\Logo.ico" 0
SectionEnd

Section "Uninstall"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ephe"
  Delete "$SMPROGRAMS\ephe\ephe.lnk"
  RMDir "$SMPROGRAMS\ephe"
  Delete "$INSTDIR\ephe.exe"
  Delete "$INSTDIR\Logo.png"
  Delete "$INSTDIR\text-logo.png"
  Delete "$INSTDIR\PressStart2P-Regular.ttf"
  Delete "$INSTDIR\Logo.ico"
  Delete "$INSTDIR\uninst.exe"
  RMDir "$INSTDIR"
SectionEnd