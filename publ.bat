
@echo off
title CodeLab Git Publisher
color 0B

:: Switch to the directory containing this BAT file
cd /d "%~dp0"

echo ============================
echo       CodeLab Publisher
echo ============================
echo.

:: Verify Git repository
if not exist ".git" (
    echo [ERROR] No Git repository found!
    echo Make sure this BAT file is inside your CodeLab repository.
    pause
    exit /b 1
)

set /p commit_msg="Enter commit message: "

if "%commit_msg%"=="" (
    echo Commit message cannot be empty!
    pause
    exit /b 1
)

echo.
echo [1/4] Staging changes...
git add .
if errorlevel 1 goto error

echo.
echo [2/4] Committing...
git commit -m "%commit_msg%"
if errorlevel 1 goto error

echo.
echo [3/4] Configuring Git buffer...
git config --global http.postBuffer 524288000

echo.
echo [4/4] Pushing to GitHub...
git push -u origin main --force-with-lease
if errorlevel 1 goto error

echo.
echo ============================
echo       PUBLISH SUCCESSFUL!
echo ============================
pause
exit /b 0

:error
echo.
echo [ERROR] Git operation failed!
pause
exit /b 1
