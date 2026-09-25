@echo off
REM Capture the arguments BEFORE any `call`. Inside a called subroutine `%*` refers to the
REM subroutine's own arguments, not the script's - so forwarding them from in here silently drops
REM every argument the user typed. This was a real bug: `demo.bat --pattern breakcore` rendered the
REM default techno patch and printed no error, because the exe never saw the flag.
set "SPARQ_ARGS=%*"
REM Keep the window open when this script was double-clicked, and always leave a log behind so
REM the result survives the window closing. CMD_CLICK_STARTED is set only by Explorer, so the
REM pause never fires when the script is run from an existing terminal.
if defined CMD_CLICK_STARTED set "SPARQ_DOUBLE_CLICKED=1"
REM UTF-8, so the middot and em-dash the binary prints do not arrive as mojibake on a cp850 console.
chcp 65001 >nul 2>&1
if not exist "%~dp0..\logs" mkdir "%~dp0..\logs" >nul 2>&1
set "SPARQ_LOG=%~dp0..\logs\devices.log"
call :sparq_main %SPARQ_ARGS% > "%SPARQ_LOG%" 2>&1
set "SPARQ_RC=%errorlevel%"
type "%SPARQ_LOG%"
echo.
echo  log saved to %SPARQ_LOG%
if defined SPARQ_DOUBLE_CLICKED pause
exit /b %SPARQ_RC%

:sparq_main
REM ============================================================================
REM  sparq - list every audio device on every backend.
REM
REM  Run this FIRST on a new machine. It answers: does Windows see your
REM  interface, what sample rates and channel counts does it offer, and which
REM  backends exist (WASAPI / DirectSound / MME / ASIO)?
REM
REM  Usage:  scripts\devices.bat
REM          scripts\devices.bat > my-devices.txt     (to save and send back)
REM ============================================================================
setlocal
cd /d "%~dp0.."
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

REM Always go through build.bat: incremental when the tree is honest, and its stamp
REM guard (defect #41) catches the zip-sync staleness that "build only when missing"
REM cannot see - a synced tree with archive timestamps keeps the OLD binary otherwise.
call scripts\build.bat
if errorlevel 1 exit /b 1

echo  ============================ HAL backends (WO-006) ============================
echo.
target\release\sparq.exe devices --caps
echo.
echo  ======================= bootstrap backends (cpal, ADR-008) =====================
echo.
target\release\sparq.exe play --list-devices
echo.
echo  HAL:  sparq play --backend wasapi-exclusive    (or wasapi-shared, or null)
echo  bootstrap: scripts\run.bat --device N --backend wasapi
echo  full HAL bring-up + diagnostics: scripts\hal.bat
exit /b 0
