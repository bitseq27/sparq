@echo off
REM ===========================================================================
REM  sparq digest - compact copies of the device logs, for sending back.
REM
REM  tools\log_digest.py keeps every verdict, banner, probe table, open line
REM  and operator answer byte-identical, and collapses exactly three classes
REM  of repetition: the [t+ Ns] periodic status runs - first 2 and last 2 of
REM  each run are kept, the middle becomes one count line carrying the
REM  blocks/xrun ranges - the cargo build chatter, and runs of identical
REM  lines. The full log stays on disk; the digest is what travels.
REM
REM  Usage:  scripts\digest.bat                 every test*.log and logs\*.log
REM          scripts\digest.bat test004.log     just the named file(s)
REM
REM  test004.bat, test006.bat and gates.bat already call the tool themselves
REM  at the end of a run; this script is for older logs and for re-runs.
REM
REM  Rules honoured - tools/check_text_io.py: pure ASCII - rule 3, defect
REM  #42 - and no unescaped parens in echo text inside blocks - rule 4,
REM  defect #69.
REM ===========================================================================
setlocal EnableDelayedExpansion
chcp 65001 >nul 2>&1
cd /d "%~dp0.."
set "PY="
where python >nul 2>&1 && set "PY=python"
if not defined PY where py >nul 2>&1 && set "PY=py"
if not defined PY (
    echo  no python or py on PATH - the digest tool needs Python 3.11+.
    if defined CMD_CLICK_STARTED pause
    exit /b 1
)
if not "%~1"=="" (
    "%PY%" tools\log_digest.py %*
    set "RC=!ERRORLEVEL!"
    if defined CMD_CLICK_STARTED pause
    exit /b !RC!
)
set "FOUND=0"
for %%F in (test*.log logs\*.log) do (
    set "N=%%~nF"
    echo !N!| findstr /e /c:"-digest" >nul 2>&1
    if errorlevel 1 (
        set "FOUND=1"
        "%PY%" tools\log_digest.py "%%F"
    )
)
if "!FOUND!"=="0" echo  no test*.log or logs\*.log found - nothing to digest.
echo.
echo  each digest sits beside its source as NAME-digest.log
if defined CMD_CLICK_STARTED pause
exit /b 0
