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
set "SPARQ_LOG=%~dp0..\logs\run.log"
call :sparq_main %SPARQ_ARGS% > "%SPARQ_LOG%" 2>&1
set "SPARQ_RC=%errorlevel%"
type "%SPARQ_LOG%"
echo.
echo  log saved to %SPARQ_LOG%
if defined SPARQ_DOUBLE_CLICKED pause
exit /b %SPARQ_RC%

:sparq_main
REM ============================================================================
REM  sparq - make sound.
REM
REM  TURN YOUR MONITORS / HEADPHONES DOWN FIRST. The default is a 220 Hz tone
REM  at -12 dBFS: audible, not damaging, but it will start immediately.
REM
REM  Usage:
REM    scripts\run.bat                          play until you press q
REM    scripts\run.bat --seconds 10             play for 10 s, no key input
REM    scripts\run.bat --device 2               use output device index 2
REM    scripts\run.bat --backend wasapi         force a backend
REM    scripts\run.bat --rate 96000 --block 128 higher rate, bigger block
REM    scripts\run.bat --freq 110 --gain -20    different tone, quieter
REM    scripts\run.bat --write-wav out.wav      also capture what sparq produced
REM
REM  Keys while playing:  ] [ gain   f F frequency   m mute   q quit
REM  Any extra arguments are passed straight through to sparq.
REM ============================================================================
setlocal
cd /d "%~dp0.."

REM Always go through build.bat: incremental when the tree is honest, and its stamp
REM guard (defect #41) catches the zip-sync staleness that "build only when missing"
REM cannot see - a synced tree with archive timestamps keeps the OLD binary otherwise.
call scripts\build.bat
if errorlevel 1 exit /b 1

echo  Starting audio. If nothing happens for a few seconds, press Ctrl+C and run
echo  scripts\diag.bat, which captures a full diagnostic log.
echo.
target\release\sparq.exe play %*
set RC=%errorlevel%
echo.
echo  exit code %RC%
if not "%RC%"=="0" echo  Something went wrong - copy the output above into a message back to me.
exit /b %RC%
