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
set "SPARQ_LOG=%~dp0..\logs\hal.log"
call :sparq_main %SPARQ_ARGS% > "%SPARQ_LOG%" 2>&1
set "SPARQ_RC=%errorlevel%"
type "%SPARQ_LOG%"
echo.
echo  log saved to %SPARQ_LOG%
if defined SPARQ_DOUBLE_CLICKED pause
exit /b %SPARQ_RC%

:sparq_main
REM ============================================================================
REM  sparq - WO-006 HAL bring-up and pre-flight on real hardware.
REM
REM  Runs, in order, and logs everything to logs\hal.log:
REM    1. build (hal-wasapi + bootstrap features)
REM    2. sparq devices                      every backend, every device
REM    3. sparq devices --caps               probed capabilities per device
REM    4. sparq devices --conformance        the behavioural promises, measured
REM                                           (briefly opens the default device at -60 dB)
REM    5. sparq play --backend wasapi-exclusive --seconds 10
REM    6. sparq play --backend wasapi-shared  --seconds 10
REM    7. sparq soak --backend wasapi-exclusive --minutes 2   (96 kHz / 64, the
REM                                           acceptance configuration, short version)
REM
REM  Steps 5-7 MAKE SOUND (220 Hz at -6 dB, then -12 dB). MONITORS DOWN FIRST.
REM
REM  This script is diagnostics, not a gate: a failing step does not stop the
REM  run - the log is more useful complete. The exit code is the count of
REM  failed steps.
REM
REM  The FULL WO-006 acceptance soak (2 hours) is:  scripts\soak.bat 120 wasapi-exclusive
REM  Run it on the stage machine, idle, once this script is clean.
REM ============================================================================
setlocal EnableDelayedExpansion
cd /d "%~dp0.."
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
set FAILED=0

where cargo >nul 2>&1
if errorlevel 1 (
    echo  cargo not found on PATH. Run scripts\setup.bat first.
    exit /b 1
)

echo  [1/7] building (hal-wasapi + bootstrap-audio) ...
call scripts\build.bat
if errorlevel 1 (
    echo  build failed - nothing else can run.
    exit /b 1
)
echo.

call :step "2/7" "devices"               "target\release\sparq.exe devices"
call :step "3/7" "devices --caps"        "target\release\sparq.exe devices --caps"
echo  NOTE: the next step briefly opens the default device with a -60 dB signal.
call :step "4/7" "conformance"           "target\release\sparq.exe devices --conformance"
echo  NOTE: steps 5-7 MAKE SOUND. Monitors down. Each plays a 220 Hz tone.
echo  NOTE: over remote desktop, step 5 (exclusive) is EXPECTED to fail with an honest
echo        Format/Busy error - RDP endpoints do not allow exclusive mode. Steps 4 and 6
echo        must pass everywhere; step 5 must pass at the physical machine.
call :step "5/7" "play wasapi-exclusive" "target\release\sparq.exe play --backend wasapi-exclusive --seconds 10 --gain -12"
call :step "6/7" "play wasapi-shared"    "target\release\sparq.exe play --backend wasapi-shared --seconds 10 --gain -12"
call :step "7/7" "soak 2 min exclusive"  "target\release\sparq.exe soak --backend wasapi-exclusive --minutes 2 --rate 96000 --block 64 --report-every 30"
echo  NOTE: step 7 needs exclusive mode at 96 kHz - over RDP it fails honestly, at the
echo        physical machine it is the acceptance rehearsal. A shared-mode rehearsal that
echo        works everywhere:  sparq soak --backend wasapi-shared --minutes 2


echo.
echo  ============================================
if "!FAILED!"=="0" (
    echo   HAL pre-flight: ALL STEPS PASSED
    echo   Next: the 2-hour acceptance soak on the stage machine:
    echo     scripts\soak.bat 120 wasapi-exclusive
) else (
    echo   HAL pre-flight: !FAILED! step^(s^) failed
    echo   Copy logs\hal.log into a message back - the whole file, not the tail.
)
echo  ============================================
exit /b !FAILED!

:step
REM  %~1 = label, %~2 = title, %~3 = command
echo  [%~1] %~2 ...
%~3
set RC=%errorlevel%
if not "!RC!"=="0" (
    echo  [%~1] FAILED ^(exit !RC!^)
    set /a FAILED+=1
) else (
    echo  [%~1] ok
)
echo.
exit /b 0
