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
set "SPARQ_LOG=%~dp0..\logs\demo.log"
call :sparq_main %SPARQ_ARGS% > "%SPARQ_LOG%" 2>&1
set "SPARQ_RC=%errorlevel%"
type "%SPARQ_LOG%"
echo.
echo  log saved to %SPARQ_LOG%
if defined SPARQ_DOUBLE_CLICKED pause
exit /b %SPARQ_RC%

:sparq_main
REM ============================================================================
REM  sparq - render the Phase C1 patch to WAV. NO AUDIO DEVICE NEEDED.
REM
REM  Offline and deterministic: you audition the WAV on your own machine. Same
REM  settings + same seed = bit-identical file, so a change can be A/B'd by hash.
REM
REM  Usage:
REM    scripts\demo.bat                       4 bars, default style, no mutation
REM    scripts\demo.bat --list-patterns       every style and mutation preset
REM    scripts\demo.bat --pattern breakcore --mutation grid-break --seed 7
REM    scripts\demo.bat --pattern glitch --mutation chaos --seed 3 --show-dna
REM    scripts\demo.bat --bars 8 --bpm 160 --crush-bits 6 --delay-send 0.5
REM  Any extra arguments are passed straight through to sparq.
REM
REM  This script ALWAYS rebuilds first. It used to build only when sparq.exe was
REM  missing, which meant a synced source tree silently kept running the old
REM  binary: arguments were accepted, ignored, and the same WAV came out every
REM  time. `sparq version` prints the build stamp if you want to check by hand.
REM ============================================================================
setlocal
cd /d "%~dp0.."

echo  [1/3] building (always - a stale exe is worse than a slow build) ...
call scripts\build.bat
if errorlevel 1 (
    echo.
    echo  BUILD FAILED - see logs\build.log
    exit /b 1
)

echo.
echo  [2/3] checking the binary understands the current options ...
target\release\sparq.exe demo --list-patterns >nul 2>&1
if errorlevel 1 (
    echo.
    echo  ============================================================
    echo   The freshly built sparq.exe does not recognise --list-patterns.
    echo   That means the SOURCE on disk is older than this script, so the
    echo   sync did not bring the Phase C1 files across.
    echo.
    echo   Expected new files:
    echo     crates\sparq-audio\src\patterns.rs
    echo     crates\sparq-audio\src\mutation.rs
    echo     crates\sparq-audio\src\dna.rs
    echo     crates\sparq-audio\src\presets.rs
    echo     crates\sparq-kernel\src\seed.rs
    echo   See SYNC.md for the full list, then re-run this script.
    echo  ============================================================
    for %%f in (patterns mutation dna presets) do if not exist "crates\sparq-audio\src\%%f.rs" echo   MISSING crates\sparq-audio\src\%%f.rs
    if not exist "crates\sparq-kernel\src\seed.rs" echo   MISSING crates\sparq-kernel\src\seed.rs
    exit /b 1
)

echo.
if "%~1"=="" (
    echo  [3/3] rendering with NO arguments - that means the defaults:
    echo        pattern techno, mutation none, seed 0xA17E, 4 bars at 138 bpm.
    echo        This is the unmutated preset, so it is the SAME FILE every time.
    echo        For variation pass arguments, e.g.
    echo          scripts\demo.bat --pattern breakcore --mutation grid-break --seed 7
    echo          scripts\demo.bat --list-patterns
) else (
    echo  [3/3] rendering with arguments: %*
)
echo.
target\release\sparq.exe demo %*
set RC=%errorlevel%
echo.
if "%RC%"=="0" (
    echo  Copy the WAV to your local machine and listen. Over RDP it is at:
    echo    %CD%\sparq-demo.wav
    echo  Different arguments must give a different hash - if they do not, the
    echo  arguments are not reaching sparq, so send me logs\demo.log.
) else (
    echo  demo failed with exit code %RC% - copy the output above into a message to me.
)
exit /b %RC%
