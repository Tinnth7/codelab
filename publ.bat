
@echo off
title CodeLab Release Manager
color 0B

cd /d "%~dp0"

echo =====================================
echo          CodeLab Release Manager
echo =====================================
echo.

:: Check Git repository
if not exist ".git" (
    echo [ERROR] Not a Git repository!
    pause
    exit /b 1
)

:: Ask for version
set /p version="Enter release version (e.g. 0.1.0): "

if "%version%"=="" (
    echo [ERROR] Version cannot be empty!
    pause
    exit /b 1
)

:: Ask for commit reason
set /p reason="Enter commit reason: "

if "%reason%"=="" (
    echo [ERROR] Reason cannot be empty!
    pause
    exit /b 1
)

:: Define paths
set "SOURCE=%~dp0installer-o\CodeLab.exe"
set "ARCHIVE=%~dp0archive"
set "DEST=%~dp0archive\CodeLab-%version%.exe"

echo.
echo =====================================
echo           RELEASE SUMMARY
echo =====================================
echo Version: %version%
echo Reason:  %reason%
echo.
echo Source:  installer-o\CodeLab.exe
echo Target:  archive\CodeLab-%version%.exe
echo =====================================
echo.

pause

:: Move and rename executable
echo.
echo [1/5] Archiving executable...

if not exist "%SOURCE%" (
    echo [ERROR] CodeLab.exe not found!
    goto error
)

if not exist "%ARCHIVE%" mkdir "%ARCHIVE%"

if exist "%DEST%" (
    echo [ERROR] This version already exists!
    goto error
)

move "%SOURCE%" "%DEST%"
if errorlevel 1 goto error

:: Git staging
echo.
echo [2/5] Staging changes...

git add .
if errorlevel 1 goto error

:: Git commit
echo.
echo [3/5] Committing...

git commit -m "%reason%"
if errorlevel 1 goto error

:: Configure buffer
echo.
echo [4/5] Configuring Git...

git config --global http.postBuffer 524288000

:: Push
echo.
echo [5/5] Publishing to GitHub...

git push -u origin main
if errorlevel 1 goto error

echo.
echo =====================================
echo         RELEASE PUBLISHED!
echo =====================================
echo Version: CodeLab-%version%.exe
echo Archived in: archive\
echo.
pause
exit /b 0

:error
echo.
echo =====================================
echo             RELEASE FAILED
echo =====================================
echo Check the error above.
echo Your executable may already have been moved.
pause
exit /b 1
