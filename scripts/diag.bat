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
set "SPARQ_LOG=%~dp0..\logs\diag.log"
call :sparq_main %SPARQ_ARGS% > "%SPARQ_LOG%" 2>&1
set "SPARQ_RC=%errorlevel%"
type "%SPARQ_LOG%"
echo.
echo  log saved to %SPARQ_LOG%
if defined SPARQ_DOUBLE_CLICKED pause
exit /b %SPARQ_RC%

:sparq_main
REM ============================================================================
REM  sparq - full diagnostic capture. Run this when there is no sound.
REM
REM  Produces sparq-diag.log containing: environment, device list on every
REM  backend, a timed 10-second playback with telemetry, and a WAV capture of
REM  what sparq actually produced. Send me that file and I can tell you whether
REM  the problem is sparq, the driver, or the OS mixer.
REM
REM  Usage:  scripts\diag.bat
REM ============================================================================
setlocal
cd /d "%~dp0.."
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
set LOG=sparq-diag.log
set WAV=sparq-diag.wav

REM Always go through build.bat: incremental when the tree is honest, and its stamp
REM guard (defect #41) catches the zip-sync staleness that "build only when missing"
REM cannot see - a synced tree with archive timestamps keeps the OLD binary otherwise.
call scripts\build.bat
if errorlevel 1 exit /b 1

echo  writing %LOG% ...
(
    echo ===== sparq diagnostic capture =====
    echo date        : %DATE% %TIME%
    echo computer    : %COMPUTERNAME%
    echo user        : %USERNAME%
    echo processor   : %PROCESSOR_IDENTIFIER%
    echo cores       : %NUMBER_OF_PROCESSORS%
    echo os          :
    ver
    echo.
    echo ===== toolchain =====
    rustc --version
    cargo --version
    rustup show active-toolchain
    echo.
    echo ===== sparq version =====
    target\release\sparq.exe version
    echo.
    echo ===== devices =====
    target\release\sparq.exe play --list-devices
    echo.
    echo ===== selftest =====
    target\release\sparq.exe selftest --golden
    echo.
    echo ===== 10 second playback with capture =====
    target\release\sparq.exe play --seconds 10 --gain -12 --write-wav %WAV%
    echo.
    echo ===== capture file =====
    if exist %WAV% (
        for %%A in (%WAV%) do echo %WAV% : %%~zA bytes
    ) else (
        echo %WAV% : NOT CREATED
    )
) > "%LOG%" 2>&1

echo.
echo  ============================================================
echo   wrote %CD%\%LOG%
if exist %WAV% echo   wrote %CD%\%WAV%  - open it in any audio editor
echo.
echo   1. Open %WAV%. Hear/see a sine?  -^> sparq is fine, the problem is
echo      the playback device, the OS mixer, or interface routing.
echo      No sine?                       -^> the problem is inside sparq.
echo   2. Either way, send me %LOG%.
echo  ============================================================
exit /b 0
