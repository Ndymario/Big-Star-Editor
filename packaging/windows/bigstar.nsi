; NSIS installer for Big Star Editor.
;
; Driven from build.sh, which passes every path and the version in as defines:
;   VERSION    x.y.z
;   VIVERSION  x.y.z.0 (VIProductVersion needs four fields)
;   DIST_DIR   the staged dist\ tree (the exe and its data folder), native path
;   LICENSE_FILE the GPLv3 text shown on the license page (UTF-8 with a BOM)
;   ICON       the .ico used for the installer and the uninstaller
;   OUTFILE    where to write the Setup exe
;
; Only the program is installed and removed. The app's settings (in
; %APPDATA%\bigstar) and the projects belong to the user and are left where
; they are by the uninstaller.

Unicode true
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!include "FileFunc.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef VIVERSION
  !define VIVERSION "0.0.0.0"
!endif

!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\BigStarEditor"

Name "Big Star Editor"
BrandingText "Big Star Editor ${VERSION}"
OutFile "${OUTFILE}"
InstallDir "$PROGRAMFILES64\Big Star Editor"
InstallDirRegKey HKLM "Software\BigStarEditor" "InstallDir"
RequestExecutionLevel admin

VIProductVersion "${VIVERSION}"
VIAddVersionKey "ProductName" "Big Star Editor"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "CompanyName" "TheGameratorT"
VIAddVersionKey "FileDescription" "Big Star Editor Setup"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "GNU General Public License v3"

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_ABORTWARNING

!define MUI_FINISHPAGE_RUN "$INSTDIR\bigstar.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Run Big Star Editor"

!insertmacro MUI_PAGE_LICENSE "${LICENSE_FILE}"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Big Star Editor" SecMain
    ; An update replaces the data whole: a catalog file the new version no
    ; longer carries must not stay behind for the editor to load.
    RMDir /r "$INSTDIR\data"

    SetOutPath "$INSTDIR"
    File /r "${DIST_DIR}\*"

    ; All-users shortcuts.
    SetShellVarContext all
    CreateDirectory "$SMPROGRAMS\Big Star Editor"
    CreateShortcut "$SMPROGRAMS\Big Star Editor\Big Star Editor.lnk" \
        "$INSTDIR\bigstar.exe" "" "$INSTDIR\bigstar.exe" 0 \
        SW_SHOWNORMAL "" "Level editor for New Super Mario Bros. DS projects"
    CreateShortcut "$DESKTOP\Big Star Editor.lnk" \
        "$INSTDIR\bigstar.exe" "" "$INSTDIR\bigstar.exe" 0 \
        SW_SHOWNORMAL "" "Level editor for New Super Mario Bros. DS projects"

    WriteRegStr HKLM "Software\BigStarEditor" "InstallDir" "$INSTDIR"
    WriteUninstaller "$INSTDIR\Uninstall.exe"

    ; Add/Remove Programs entry.
    WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "Big Star Editor"
    WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
    WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "TheGameratorT"
    WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\bigstar.exe"
    WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
    WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
    WriteRegStr HKLM "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
    WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
    WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1
    ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
    IntFmt $0 "0x%08X" $0
    WriteRegDWORD HKLM "${UNINST_KEY}" "EstimatedSize" "$0"
SectionEnd

Section "Uninstall"
    SetShellVarContext all
    Delete "$SMPROGRAMS\Big Star Editor\Big Star Editor.lnk"
    RMDir "$SMPROGRAMS\Big Star Editor"
    Delete "$DESKTOP\Big Star Editor.lnk"

    ; Run normally (no _?= switch), so NSIS copies this uninstaller to $TEMP
    ; and executes from there, letting RMDir /r remove $INSTDIR including the
    ; original Uninstall.exe.
    RMDir /r "$INSTDIR"

    DeleteRegKey HKLM "${UNINST_KEY}"
    DeleteRegKey HKLM "Software\BigStarEditor"
SectionEnd
