@echo off
REM Capture the arguments BEFORE any `call`. Inside a called subroutine `%*` refers to the
REM subroutine's own arguments, not the script's - so forwarding them from in here silently drops
REM every argument the user typed.
set "SPARQ_ARGS=%*"
REM Keep the window open when this script was double-clicked, and always leave a log behind so
REM the result survives the window closing. CMD_CLICK_STARTED is set only by Explorer, so the
REM pause never fires when the script is run from an existing terminal.
if defined CMD_CLICK_STARTED set "SPARQ_DOUBLE_CLICKED=1"
chcp 65001 >nul 2>&1
if not exist "%~dp0..\logs" mkdir "%~dp0..\logs" >nul 2>&1
set "SPARQ_LOG=%~dp0..\logs\synccheck.log"
call :sparq_main %SPARQ_ARGS% > "%SPARQ_LOG%" 2>&1
set "SPARQ_RC=%errorlevel%"
type "%SPARQ_LOG%"
echo.
echo  log saved to %SPARQ_LOG%
if defined SPARQ_DOUBLE_CLICKED pause
exit /b %SPARQ_RC%

:sparq_main
REM ============================================================================
REM  sparq synccheck - is THIS tree the tree the sync shipped?
REM
REM  Seconds, no cargo, no build: tools\sync_check.py hashes every file that
REM  decides what the binary is against SYNC-STAMP.txt and names the ones that
REM  differ. Run it any time a build produces errors that contradict the source
REM  you can see on disk - that is defect #78's signature: six rustc errors
REM  about `resolve_master`/`master`/`RenderWav` on SATURN while the file that
REM  defines them was 51 583 bytes and correct, because cargo had answered
REM  "Fresh sparq-ui" for an rlib built from the PREVIOUS increment.
REM
REM  build.bat runs the same check before every build. This wrapper exists so
REM  the question can be answered without a three-minute compile, and so the
REM  full per-file report - not just the verdict - lands in a log.
REM
REM  Exit 0 = the tree matches. 1 = it does not; the list names what differs.
REM  Exit 2 = the check could not run, which is NOT a pass.
REM
REM  Usage:  scripts\synccheck.bat            [--json]
REM ============================================================================
setlocal
cd /d "%~dp0.."

set "PY="
where python >nul 2>&1 && set "PY=python"
if not defined PY where py >nul 2>&1 && set "PY=py"
if not defined PY (
    echo  synccheck: CANNOT RUN - no `python` or `py` on PATH.
    echo  Install Python 3.11+ and run this again. This is not a pass: the tree is
    echo  unverified, and a half-applied sync would show up later as rustc errors in
    echo  a crate that is not the one that is wrong.
    exit /b 2
)

echo  sparq synccheck - tree vs SYNC-STAMP.txt
echo.
%PY% tools\sync_check.py %SPARQ_ARGS%
set "RC=%errorlevel%"
echo.
if "%RC%"=="0" echo  verdict: the tree matches the stamp. Safe to build.
if "%RC%"=="1" echo  verdict: the tree does NOT match. Re-extract the sync zip at the
if "%RC%"=="1" echo           repo root, overwriting, then run scripts\build.bat again.
if "%RC%"=="2" echo  verdict: the check could not run - not a pass.
exit /b %RC%
