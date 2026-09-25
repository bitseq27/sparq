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
set "SPARQ_LOG=%~dp0..\logs\soak.log"
call :sparq_main %SPARQ_ARGS% > "%SPARQ_LOG%" 2>&1
set "SPARQ_RC=%errorlevel%"
type "%SPARQ_LOG%"
echo.
echo  log saved to %SPARQ_LOG%
if defined SPARQ_DOUBLE_CLICKED pause
exit /b %SPARQ_RC%

:sparq_main
REM ============================================================================
REM  sparq - long-run reliability soak (WO-016 gate).
REM
REM  Renders continuously, discarding the audio in 10-second chunks so memory
REM  stays flat, while checking: zero allocations on the audio path, zero
REM  underruns against the real-time budget, and no determinism drift.
REM  It fails fast on the first defect rather than reporting at the end.
REM
REM  Usage:
REM    scripts\soak.bat                    120 minutes, offline render (the Phase 0 gate)
REM    scripts\soak.bat 2                  2 minutes (pre-flight check)
REM    scripts\soak.bat 720                12 hours (the interactive soak target)
REM    scripts\soak.bat 2 null             paced HAL soak against the virtual device
REM    scripts\soak.bat 120 wasapi-exclusive
REM                                          the WO-006 acceptance soak: 2 h at 96 kHz/64
REM                                          through the real HAL, on the stage device.
REM                                          MAKES SOUND (a -6 dB 220 Hz tone); monitors down.
REM    scripts\soak.bat 5 wasapi-exclusive --heavy 40
REM                                          extra args are forwarded (--heavy loads the
REM                                          callback deliberately, per WO-006 task 6)
REM ============================================================================
setlocal EnableDelayedExpansion
cd /d "%~dp0.."
set MINUTES=%1
if "%MINUTES%"=="" set MINUTES=120
set BACKEND=%2
REM Collect args 3..N verbatim (e.g. --heavy 40). SHIFT /3 leaves %1/%2 intact
REM and slides everything from %3 left, so this terminates when %3 empties.
set "EXTRA="
:collect_extra
if "%~3"=="" goto extra_done
set "EXTRA=!EXTRA! %3"
shift /3
goto collect_extra
:extra_done

REM Always go through build.bat: incremental when the tree is honest, and its stamp
REM guard (defect #41) catches the zip-sync staleness that "build only when missing"
REM cannot see - a synced tree with archive timestamps keeps the OLD binary otherwise.
call scripts\build.bat
if errorlevel 1 exit /b 1

set BACKEND_ARG=
if not "%BACKEND%"=="" set BACKEND_ARG=--backend %BACKEND%
if "%BACKEND%"=="" (
    echo  soaking %MINUTES% minutes of audio at 96 kHz / 64-sample blocks ...
    echo  This is CPU work, not playback: it does not need a sound card and makes no sound.
) else (
    echo  soaking %MINUTES% minutes through the HAL backend `%BACKEND%` at 96 kHz / 64 ...
    echo  This is a PACED run: with a device backend it MAKES SOUND. Monitors down first.
)
echo  Press Ctrl+C to stop early.
echo.
target\release\sparq.exe soak --minutes %MINUTES% --report-every 30 %BACKEND_ARG% !EXTRA!
set RC=%errorlevel%
echo.
if "%RC%"=="0" (
    echo  SOAK PASSED
) else (
    echo  SOAK FAILED - copy the output above into a message back to me.
)
exit /b %RC%
