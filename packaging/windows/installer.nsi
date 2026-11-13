; Pyxee installer (NSIS 3, Unicode). Built by `cargo xtask installer`, which passes these defines:
;
;   VERSION         0.5.2 or 0.5.2-rc1
;   VI_VERSION      0.5.2.0 (the four numbers Windows wants for the file properties)
;   STAGEDIR        the folder to install (target\stage\Pyxee: Pyxee.exe, yt-dlp.exe, the Flutter files, notices)
;   OUTFILE         where to write Pyxee_<version>_x64-setup.exe
;   UNINSTALL_LIST  an include with the Delete / RMDir lines for exactly the files in STAGEDIR
;   ASSETS          assets\icons (the installer icon and the two pictures)
;   SIGN_XTASK      optional: path of xtask.exe; when set, the uninstaller is signed by `xtask sign-file`
;
; It installs for the current user (no administrator prompt) into %LOCALAPPDATA%\Pyxee, which is where the
; installers of earlier versions put it, under the same uninstall entry, so a new version installs over an old one.
; The in-app updater starts this installer with no arguments right after Pyxee has closed: it runs as the normal
; wizard, over the copy that was running a moment ago. `/S` (silent) and `/D=<folder>` work as in any NSIS installer.
;
; The user's library and settings are in %LOCALAPPDATA%\dev.pyxee.app, outside the install folder. Installing over
; a copy never touches them, and the uninstaller keeps them unless the person says otherwise.

Unicode true
ManifestDPIAware true
ManifestDPIAwareness PerMonitorV2
SetCompressor /SOLID lzma

!define PRODUCT "Pyxee"
!define MAIN_EXE "Pyxee.exe"
!define PUBLISHER "Pylxyr"
!define HOMEPAGE "https://github.com/Pylxyr/Pyxee"
!define DATA_DIR_NAME "dev.pyxee.app"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT}"

!ifndef VERSION
  !error "VERSION is not defined (build with: cargo xtask installer)"
!endif
!ifndef STAGEDIR
  !error "STAGEDIR is not defined (build with: cargo xtask installer)"
!endif
!ifndef OUTFILE
  !error "OUTFILE is not defined (build with: cargo xtask installer)"
!endif
!ifndef UNINSTALL_LIST
  !error "UNINSTALL_LIST is not defined (build with: cargo xtask installer)"
!endif
!ifndef ASSETS
  !error "ASSETS is not defined (build with: cargo xtask installer)"
!endif
!ifndef VI_VERSION
  !define VI_VERSION "0.0.0.0"
!endif

Name "${PRODUCT}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\${PRODUCT}"
RequestExecutionLevel user
BrandingText "${PRODUCT} ${VERSION}"

VIProductVersion "${VI_VERSION}"
VIAddVersionKey "ProductName" "${PRODUCT}"
VIAddVersionKey "FileDescription" "${PRODUCT} installer"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "CompanyName" "${PUBLISHER}"
VIAddVersionKey "LegalCopyright" "MIT licence"

!include MUI2.nsh
!include LogicLib.nsh
!include FileFunc.nsh
!include x64.nsh

!define MUI_ICON "${ASSETS}\icon.ico"
!define MUI_UNICON "${ASSETS}\icon.ico"
!define MUI_ABORTWARNING
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_BITMAP "${ASSETS}\installer\header.bmp"
!define MUI_HEADERIMAGE_RIGHT
!define MUI_WELCOMEFINISHPAGE_BITMAP "${ASSETS}\installer\sidebar.bmp"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "${ASSETS}\installer\sidebar.bmp"

; The installer is not run as administrator, so the install folder is the person's own.
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_FUNCTION LaunchApp
; The finish page's second box is "Create desktop shortcut", and it starts unticked.
!define MUI_FINISHPAGE_SHOWREADME
!define MUI_FINISHPAGE_SHOWREADME_TEXT "Create desktop shortcut"
!define MUI_FINISHPAGE_SHOWREADME_FUNCTION CreateDesktopShortcut
!define MUI_FINISHPAGE_SHOWREADME_NOTCHECKED

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; Sign the uninstaller that NSIS generates (the installer itself is signed by xtask afterwards).
!ifdef SIGN_XTASK
  !uninstfinalize '"${SIGN_XTASK}" sign-file "%1"'
!endif

; ---------------------------------------------------------------------------------------------- helpers

Function .onInit
  ${If} ${RunningX64}
    SetRegView 64
  ${EndIf}
  ; Update in place: use the folder of the copy that is already installed, unless /D= chose one (NSIS has then
  ; already put it in $INSTDIR, so it no longer equals the default).
  ${If} $INSTDIR == "$LOCALAPPDATA\${PRODUCT}"
    ReadRegStr $R2 HKCU "${UNINSTKEY}" "InstallLocation"
    ${If} $R2 != ""
      ; Earlier installers wrote the folder in quotes.
      StrCpy $R3 $R2 1
      ${If} $R3 == '"'
        StrLen $R4 $R2
        IntOp $R4 $R4 - 2
        StrCpy $R2 $R2 $R4 1
      ${EndIf}
      ${If} ${FileExists} "$R2\${MAIN_EXE}"
        StrCpy $INSTDIR $R2
      ${EndIf}
    ${EndIf}
  ${EndIf}
FunctionEnd

; A running Pyxee keeps its program file open, so the file cannot be opened for writing. The updater has closed
; Pyxee already; for a manual install, say what is wrong and let the person close it. Silent installs just fail.
Function CheckNotRunning
  ${If} ${FileExists} "$INSTDIR\${MAIN_EXE}"
  retry:
    ClearErrors
    FileOpen $0 "$INSTDIR\${MAIN_EXE}" a
    ${If} ${Errors}
      IfSilent give_up
      MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "${PRODUCT} is running. Close it (also from the tray icon), then choose Retry." IDRETRY retry
    give_up:
      SetErrorLevel 2
      Abort
    ${EndIf}
    FileClose $0
  ${EndIf}
FunctionEnd

Function LaunchApp
  ; Run it as the person who started the installer, not through the installer's own process.
  Exec '"$INSTDIR\${MAIN_EXE}"'
FunctionEnd

Function CreateDesktopShortcut
  CreateShortCut "$DESKTOP\${PRODUCT}.lnk" "$INSTDIR\${MAIN_EXE}"
FunctionEnd

; ---------------------------------------------------------------------------------------------- install

Section "Install"
  Call CheckNotRunning

  SetOutPath "$INSTDIR"
  File /r "${STAGEDIR}\*.*"

  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateShortCut "$SMPROGRAMS\${PRODUCT}.lnk" "$INSTDIR\${MAIN_EXE}"
  ; A silent install (a script, or a managed PC) has no finish page to tick, and has always made the shortcut.
  IfSilent 0 +2
    Call CreateDesktopShortcut

  WriteRegStr HKCU "${UNINSTKEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayIcon" '"$INSTDIR\${MAIN_EXE}"'
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTKEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINSTKEY}" "InstallLocation" '"$INSTDIR"'
  WriteRegStr HKCU "${UNINSTKEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINSTKEY}" "URLInfoAbout" "${HOMEPAGE}"
  WriteRegStr HKCU "${UNINSTKEY}" "HelpLink" "${HOMEPAGE}"
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINSTKEY}" "EstimatedSize" "$0"
SectionEnd

; -------------------------------------------------------------------------------------------- uninstall

Function un.onInit
  ${If} ${RunningX64}
    SetRegView 64
  ${EndIf}
FunctionEnd

Section "Uninstall"
  ; Never delete a folder recursively: the install folder is the person's choice. The list is exactly what the
  ; installer wrote (generated from the staged folder), and the folder itself goes only if it is then empty.
  !include "${UNINSTALL_LIST}"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${PRODUCT}.lnk"
  Delete "$DESKTOP\${PRODUCT}.lnk"
  DeleteRegKey HKCU "${UNINSTKEY}"

  ; The library, playlists and settings are kept unless the person asks for them to go (never on a silent
  ; uninstall, which has nobody to ask).
  IfSilent done
  MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "Also delete your library, playlists and settings?$\r$\n$\r$\nThey are kept in $LOCALAPPDATA\${DATA_DIR_NAME}. Choose No to keep them for a later install." IDNO done
  RMDir /r "$LOCALAPPDATA\${DATA_DIR_NAME}"
done:
SectionEnd
