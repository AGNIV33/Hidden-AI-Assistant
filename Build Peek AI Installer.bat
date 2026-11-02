@echo off
title Building the Peek AI installer
cd /d "%~dp0"
echo.
echo ===== Peek AI installer builder =====
echo Step 1/2: installing packages...
call npm install
if errorlevel 1 goto fail
echo.
echo Step 2/2: building the app and installer (first time can take 5-15 minutes)...
call npm run tauri build
if errorlevel 1 goto fail
echo.
echo DONE! Your installer is in this folder (it opens now):
echo   %~dp0src-tauri\target\release\bundle\nsis
echo File name: "Peek AI_1.0.0_x64-setup.exe"
explorer "%~dp0src-tauri\target\release\bundle\nsis"
pause
exit /b 0
:fail
echo.
echo Build failed. Take a screenshot of the red text above and send it.
pause
exit /b 1
