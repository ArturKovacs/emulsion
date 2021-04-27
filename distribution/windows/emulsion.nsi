;NSIS Modern User Interface

Unicode True

;--------------------------------
;Includes and related defines

    ; VERSION is defined through a command line argument to the maker
    ; for example:
    ; makensis.exe /DVERSION=9.0 emulsion.nsi

    !define PROGRAM_NAME "Emulsion"
    !define REG_PROG_PATH "SOFTWARE\Emulsion"
    !define REG_UNINST_PATH "Software\Microsoft\Windows\CurrentVersion\Uninstall\Emulsion"
    !define MULTIUSER_INSTALLMODE_INSTDIR "${PROGRAM_NAME}"
    !define MULTIUSER_INSTALLMODE_INSTDIR_REGISTRY_KEY "${REG_PROG_PATH}"
    !define MULTIUSER_EXECUTIONLEVEL Highest
    !define MULTIUSER_USE_PROGRAMFILES64
    !define MULTIUSER_MUI
    !define MULTIUSER_INSTALLMODE_COMMANDLINE

    ; The Programmatic ID for file associations
    ; see: https://docs.microsoft.com/en-us/windows/win32/shell/fa-progids
    !define GENERIC_PROG_ID "${PROGRAM_NAME}.Generic"
    
    !include "MultiUser.nsh"
    !include "MUI2.nsh"
    !include "FileAssociation.nsh"
    
;--------------------------------
;General

    ;Name and file
    Name "Emulsion"
    OutFile "Emulsion-Installer.exe"

    ;Default installation folder
    ;InstallDir "$LOCALAPPDATA\Emulsion"

    ;Get installation folder from registry if available
    ;InstallDirRegKey SHCTX "${REG_PROG_PATH}" ""

    ;Request application privileges for Windows Vista
    RequestExecutionLevel user
    ManifestDPIAware true

;--------------------------------
;Interface Settings

    !define MUI_ABORTWARNING
    !define MUI_ICON "emulsion.ico"
    !define MUI_HEADERIMAGE
    ;!define MUI_HEADERIMAGE_BITMAP
    !define MUI_HEADERIMAGE_BITMAP "empty.bmp"
    !define MUI_HEADERIMAGE_RIGHT
    
    !define MUI_DIRECTORYPAGE_TEXT_TOP "The setup will install ${PROGRAM_NAME} in the following folder. To install in a different folder, click Browse and select another destination. Click Install to start the installation.$\n$\n--------------------------------------------------------------------------$\nRun this setup as administrator to install under Program Files.$\n--------------------------------------------------------------------------"

;--------------------------------
;Pages

    !insertmacro MULTIUSER_PAGE_INSTALLMODE
    !insertmacro MUI_PAGE_LICENSE "LICENSE.txt"
    ;!insertmacro MUI_PAGE_COMPONENTS
    !insertmacro MUI_PAGE_DIRECTORY
    !insertmacro MUI_PAGE_INSTFILES
    !insertmacro MUI_PAGE_FINISH

    !insertmacro MUI_UNPAGE_CONFIRM
    !insertmacro MUI_UNPAGE_INSTFILES
    !insertmacro MUI_UNPAGE_FINISH
    

;--------------------------------
;Languages

    !insertmacro MUI_LANGUAGE "English"


Function .onInit
    !insertmacro MULTIUSER_INIT
FunctionEnd

Function un.onInit
    !insertmacro MULTIUSER_UNINIT
FunctionEnd


!macro EmulsionRegisterExtension ExtensionName Description
    ; First, let's create the ProgID for the extension
    ; For more information about what is being done here, see
    ; "Default Programs"
    ; https://docs.microsoft.com/en-us/windows/win32/shell/default-programs
    WriteRegStr SHCTX "SOFTWARE\Classes\${GENERIC_PROG_ID}" "" "${Description}"
    WriteRegStr SHCTX "SOFTWARE\Classes\${GENERIC_PROG_ID}" "DefaultIcon" "$\"$INSTDIR\${PROGRAM_NAME}.exe$\""
    WriteRegStr SHCTX "SOFTWARE\Classes\${GENERIC_PROG_ID}\Shell\Open\Command" "" "$\"$INSTDIR\${PROGRAM_NAME}.exe$\" $\"%1$\""

    ; Then, let's specify this extension as a supported one under the capabilties
    WriteRegStr SHCTX "${REG_PROG_PATH}\Capabilities\FileAssociations" ".${ExtensionName}" "${GENERIC_PROG_ID}"
!macroend

; !macro EmulsionUnregisterExtension ExtensionName Description
;     ${UnRegisterExtension} ".${ExtensionName}" "${Description}"
; !macroend

;--------------------------------
;Installer Sections

Section "Emulsion" SecEmulsion
    SectionIn RO

    SetOutPath "$INSTDIR"
    SetOverwrite on

    ;ADD YOUR OWN FILES HERE...
    File /r "program\*"

    ; Start Menu
    createDirectory "$SMPROGRAMS\${PROGRAM_NAME}"
    createShortCut "$SMPROGRAMS\${PROGRAM_NAME}\${PROGRAM_NAME}.lnk" "$INSTDIR\${PROGRAM_NAME}.exe"
    
    ;Store installation folder
    WriteRegStr SHCTX "${REG_PROG_PATH}" "Install Directory" "$INSTDIR"

    WriteRegStr SHCTX "${REG_UNINST_PATH}" "DisplayName" "${PROGRAM_NAME}"
    WriteRegStr SHCTX "${REG_UNINST_PATH}" "DisplayIcon" "$\"$INSTDIR\emulsion.exe$\""
    WriteRegStr SHCTX "${REG_UNINST_PATH}" "UninstallString" "$\"$INSTDIR\Uninstall.exe$\" /$MultiUser.InstallMode"
    WriteRegStr SHCTX "${REG_UNINST_PATH}" "QuietUninstallString" "$\"$INSTDIR\Uninstall.exe$\" /$MultiUser.InstallMode /S"

    ;Create uninstaller
    WriteUninstaller "$INSTDIR\Uninstall.exe"

    ; Program description and list of supported extensions
    ; (This in itself does not associate the extensions to Emulsion)
    WriteRegStr SHCTX "${REG_PROG_PATH}\Capabilities" "ApplicationDescription" "A fast and minimalistic image viewer"
    WriteRegStr SHCTX "${REG_PROG_PATH}\Capabilities" "ApplicationName" "${PROGRAM_NAME}"

    !insertmacro EmulsionRegisterExtension "jpg" "JPG Image"
    !insertmacro EmulsionRegisterExtension "jpeg" "JPEG Image"
    !insertmacro EmulsionRegisterExtension "png" "PNG Image"
    !insertmacro EmulsionRegisterExtension "bmp" "BMP Image"
    !insertmacro EmulsionRegisterExtension "gif" "GIF Image"
    !insertmacro EmulsionRegisterExtension "tga" "TGA Image"
    !insertmacro EmulsionRegisterExtension "avif" "AVIF Image"
    !insertmacro EmulsionRegisterExtension "webp" "WEBP Image"
    !insertmacro EmulsionRegisterExtension "tif" "TIF Image"
    !insertmacro EmulsionRegisterExtension "tiff" "TIFF Image"
    !insertmacro EmulsionRegisterExtension "ico" "ICO Image"
    !insertmacro EmulsionRegisterExtension "hdr" "HDR Image"
    !insertmacro EmulsionRegisterExtension "pbm" "PBM Image"
    !insertmacro EmulsionRegisterExtension "pam" "PAM Image"
    !insertmacro EmulsionRegisterExtension "ppm" "PPM Image"
    !insertmacro EmulsionRegisterExtension "pgm" "PGM Image"

    WriteRegStr SHCTX "SOFTWARE\RegisteredApplications" "ArturK.${PROGRAM_NAME}.${VERSION}" "${REG_PROG_PATH}\Capabilities"

    ; This won't apply the file associations, it merely tells the system that there are new
    ; programs available for the specified formats
    !insertmacro UPDATEFILEASSOC
SectionEnd

; These are the programs that are needed by Emulsion.
Section -Prerequisites
    IfFileExists $SYSDIR\vcruntime140.dll endVsRedist beginVsRedist
    Goto endVsRedist
    beginVsRedist:
    SetOutPath "$INSTDIR\prerequisites"
    File ".\prerequisites\vc_redist.x64.exe"
    ExecWait "$INSTDIR\prerequisites\vc_redist.x64.exe"
    endVsRedist:
SectionEnd

;--------------------------------
;Descriptions
    ;Language strings
    LangString DESC_SecEmulsion ${LANG_ENGLISH} "The program itself."
    ;LangString DESC_SecAssociate ${LANG_ENGLISH} "Associate jpg, jpeg, png, bmp, gif, tga, avif, webp, tif, tiff, hdr, pbm, pam, ppm, and pgm files with Emulsion"

    ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
    ; These are only relevant when using the MUI_PAGE_COMPONENTS
    ; But we are not using that since the option to assign file associations has been
    ; removed. (It was removed because it's not supported by Windows)
    ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
    ;Assign language strings to sections
    ;!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
    ;    !insertmacro MUI_DESCRIPTION_TEXT ${SecEmulsion} $(DESC_SecEmulsion)
    ;    !insertmacro MUI_DESCRIPTION_TEXT ${SecAssociate} $(DESC_SecAssociate)
    ;!insertmacro MUI_FUNCTION_DESCRIPTION_END
    ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

;--------------------------------
; Uninstaller
;--------------------------------
Section Uninstall
    
    ; Remove Start Menu launcher
    Delete "$SMPROGRAMS\${PROGRAM_NAME}\${PROGRAM_NAME}.lnk"
    ; Try to remove the Start Menu folder - this will only happen if it is empty
    RMDir "$SMPROGRAMS\${PROGRAM_NAME}"
    
    Delete "$INSTDIR\emulsion.exe"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir "$INSTDIR" ; This is okay, rmdir fails if the directory is not empty.
    
    ;Remove registry keys
    DeleteRegKey SHCTX "${REG_PROG_PATH}"
    DeleteRegKey SHCTX "${REG_UNINST_PATH}"
    
    ; Extensions mustn't be unregistered here. They might be associated
    ; with a program other than Emulsion and removing those would be wrong.
    
SectionEnd
