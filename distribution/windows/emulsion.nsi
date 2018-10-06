;NSIS Modern User Interface
;Basic Example Script
;Written by Joost Verburg

;--------------------------------
;Includes and related defines
    !define PROGRAM_NAME "Emulsion"
    !define REG_PROG_PATH "SOFTWARE\Emulsion"
    ;!define REG_EXT_PROG_KEY "Software\Classes\Emulsion"
    ;!define REG_EXT_OPEN_COMMAND_KEY "Software\Classes\Emulsion\shell\open\command"
    !define REG_UNINST_PATH "Software\Microsoft\Windows\CurrentVersion\Uninstall\Emulsion"
    !define MULTIUSER_INSTALLMODE_INSTDIR "${PROGRAM_NAME}"
    !define MULTIUSER_INSTALLMODE_INSTDIR_REGISTRY_KEY "${REG_PROG_PATH}"
    !define MULTIUSER_EXECUTIONLEVEL Highest
    !define MULTIUSER_USE_PROGRAMFILES64
    !define MULTIUSER_MUI
    !define MULTIUSER_INSTALLMODE_COMMANDLINE
    
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

;--------------------------------
;Interface Settings

    !define MUI_ABORTWARNING
    !define MUI_ICON "emulsion.ico"
    !define MUI_HEADERIMAGE
    ;!define MUI_HEADERIMAGE_BITMAP
    !define MUI_HEADERIMAGE_BITMAP "empty.bmp"
    !define MUI_HEADERIMAGE_RIGHT
    
    !define MUI_DIRECTORYPAGE_TEXT_TOP "The setup will install ${PROGRAM_NAME} in the following folder. To install in a different folder, click Browse and select another destination. Click Install to start the installation.$\n$\n--------------------------------------------------------------------------$\nRun this setup as administrator to install under Program Files!$\n--------------------------------------------------------------------------"

;--------------------------------
;Pages

    !insertmacro MULTIUSER_PAGE_INSTALLMODE
    !insertmacro MUI_PAGE_LICENSE "LICENSE.txt"
    !insertmacro MUI_PAGE_COMPONENTS
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
    ;WriteRegStr SHCTX "Software\Classes\.${ExtensionName}" "" "${PROGRAM_NAME}"
    ;!insertmacro APP_ASSOCIATE "${ExtensionName}" "${PROGRAM_NAME}.${ExtensionName}" "${Description}" "$\"$INSTDIR\emulsion.exe$\",0" "Open with ${PROGRAM_NAME}" "$\"$INSTDIR\emulsion.exe$\" $\"%1$\""
    ${RegisterExtension} "$INSTDIR\emulsion.exe" ".${ExtensionName}" "${PROGRAM_NAME}.${ExtensionName}"
!macroend

!macro EmulsionUnregisterExtension ExtensionName
    ${UnRegisterExtension} ".${ExtensionName}" "${PROGRAM_NAME}.${ExtensionName}"
!macroend

;--------------------------------
;Installer Sections

Section "Emulsion" SecEmulsion
    SectionIn RO

    SetOutPath "$INSTDIR"
    SetOverwrite on

    ;ADD YOUR OWN FILES HERE...
    File /r "program\*"

    ;Store installation folder
    WriteRegStr SHCTX "${REG_PROG_PATH}" "Install Directory" "$INSTDIR"
    
    WriteRegStr SHCTX "${REG_UNINST_PATH}" "DisplayName" "${PROGRAM_NAME}"
    WriteRegStr SHCTX "${REG_UNINST_PATH}" "DisplayIcon" "$\"$INSTDIR\emulsion.exe$\""
    WriteRegStr SHCTX "${REG_UNINST_PATH}" "UninstallString" "$\"$INSTDIR\Uninstall.exe$\" /$MultiUser.InstallMode"
    WriteRegStr SHCTX "${REG_UNINST_PATH}" "QuietUninstallString" "$\"$INSTDIR\Uninstall.exe$\" /$MultiUser.InstallMode /S"
                 
    ;Create uninstaller
    WriteUninstaller "$INSTDIR\Uninstall.exe"
SectionEnd

Section /o "Associate supported files" SecAssociate
    ;WriteRegStr SHCTX "Software\Classes\${PROGRAM_NAME}" "" ""
    ;WriteRegStr SHCTX "Software\Classes\${PROGRAM_NAME}\shell" "" ""
    ;WriteRegStr SHCTX "Software\Classes\${PROGRAM_NAME}\shell\open" "" ""
    ;WriteRegStr SHCTX "Software\Classes\${PROGRAM_NAME}\shell\open\command" "" "$\"$INSTDIR\emulsion.exe$\" $\"%1$\""
    
    !insertmacro EmulsionRegisterExtension "jpg" "JPG Image"
    !insertmacro EmulsionRegisterExtension "jpeg" "JPEG Image"
    !insertmacro EmulsionRegisterExtension "png" "PNG Image"
    !insertmacro EmulsionRegisterExtension "bmp" "BMP Image"
    !insertmacro EmulsionRegisterExtension "tga" "TGA Image"
    !insertmacro EmulsionRegisterExtension "webp" "WEBP Image"
    !insertmacro EmulsionRegisterExtension "tif" "TIF Image"
    !insertmacro EmulsionRegisterExtension "tiff" "TIFF Image"
    !insertmacro EmulsionRegisterExtension "ico" "ICO Image"
    !insertmacro EmulsionRegisterExtension "hdr" "HDR Image"
    !insertmacro EmulsionRegisterExtension "pbm" "PBM Image"
    !insertmacro EmulsionRegisterExtension "pam" "PAM Image"
    !insertmacro EmulsionRegisterExtension "ppm" "PPM Image"
    !insertmacro EmulsionRegisterExtension "pgm" "PGM Image"

    !insertmacro UPDATEFILEASSOC
SectionEnd

;--------------------------------
;Descriptions
    ;Language strings
    LangString DESC_SecEmulsion ${LANG_ENGLISH} "The program itself."
    LangString DESC_SecAssociate ${LANG_ENGLISH} "Associate jpg, jpeg, png, bmp, tga, webp, tif, tiff, ico, hdr, pbm, pam, ppm, and pgm files with Emulsion"

    ;Assign language strings to sections
    !insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
        !insertmacro MUI_DESCRIPTION_TEXT ${SecEmulsion} $(DESC_SecEmulsion)
        !insertmacro MUI_DESCRIPTION_TEXT ${SecAssociate} $(DESC_SecAssociate)
    !insertmacro MUI_FUNCTION_DESCRIPTION_END

;--------------------------------
; Uninstaller
;--------------------------------
Section Uninstall

    Delete "$INSTDIR\cfg.bin" ; Created by the program
    
    Delete "$INSTDIR\emulsion.exe"
    
    Delete "$INSTDIR\resource\cogs.png"
    Delete "$INSTDIR\resource\emulsion48.png"
    Delete "$INSTDIR\resource\light.png"
    Delete "$INSTDIR\resource\moon.png"
    Delete "$INSTDIR\resource\question_button.png"
    Delete "$INSTDIR\resource\question_button_light.png"
    Delete "$INSTDIR\resource\usage.png"
    
    RMDir "$INSTDIR\resource"
    
    Delete "$INSTDIR\Uninstall.exe"
    RMDir "$INSTDIR"
    
    ;Remove registry keys
    DeleteRegKey SHCTX "${REG_PROG_PATH}"
    DeleteRegKey SHCTX "${REG_UNINST_PATH}"
    
    !insertmacro EmulsionUnregisterExtension "jpg"
    !insertmacro EmulsionUnregisterExtension "jpeg"
    !insertmacro EmulsionUnregisterExtension "png"
    !insertmacro EmulsionUnregisterExtension "bmp"
    !insertmacro EmulsionUnregisterExtension "tga"
    !insertmacro EmulsionUnregisterExtension "gif"
    !insertmacro EmulsionUnregisterExtension "webp"
    !insertmacro EmulsionUnregisterExtension "tif"
    !insertmacro EmulsionUnregisterExtension "tiff"
    !insertmacro EmulsionUnregisterExtension "ico"
    !insertmacro EmulsionUnregisterExtension "hdr"
    !insertmacro EmulsionUnregisterExtension "pbm"
    !insertmacro EmulsionUnregisterExtension "pam"
    !insertmacro EmulsionUnregisterExtension "ppm"
    !insertmacro EmulsionUnregisterExtension "pgm"
    
SectionEnd
