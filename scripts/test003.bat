@echo off
REM ===========================================================================
REM  sparq test003 - WO-006: the long shared-mode soak on the real interface.
REM
REM  THE CONTRACT: one file from Qwen - this script. Run it, answer two
REM  prompts, walk away for the soak, send back ONE file: test003.log from
REM  the repo root.
REM
REM  WHY ROUND 3. test002 resolved the endpoints: your default device IS the
REM  BEHRINGER UMC 204HD 192k, the unplug criterion PASSED, and shared mode
REM  is clean on hardware - 5 min, 450307 blocks, zero xruns, zero
REM  allocations. Exclusive mode is refused by the driver for float formats
REM  - defect #77 - and gets integer rungs in the next code increment, after
REM  which test004 runs the exclusive acceptance. Meanwhile the long soak in
REM  shared mode proves everything the acceptance soak depends on EXCEPT the
REM  exclusive open itself: the pump, the FIFO, MMCSS discipline, counters
REM  and histograms over hours instead of minutes. It is labelled in the log
REM  as the shared rehearsal, never as the acceptance run.
REM
REM  SAFE TO RE-RUN: the log appends with a dated banner per run.
REM
REM  Rules honoured - tools/check_text_io.py: pure ASCII - rule 3, defect
REM  #42 - and no unescaped parens in echo text inside blocks - rule 4,
REM  defect #69.
REM ===========================================================================
setlocal EnableDelayedExpansion
set "TVER=003"
set "RATE=96000"
set "BLOCK=64"
set "SOAKBE=wasapi-shared"
title sparq test%TVER%
chcp 65001 >nul 2>&1
cd /d "%~dp0.."

if not exist "scripts\build.bat" (
    echo  test%TVER%.bat must sit in the scripts folder of the sparq repo.
    echo  Move it there and run it again.
    pause
    exit /b 1
)

set "LOG=%CD%\test%TVER%.log"
set "TMPF=%CD%\_test%TVER%.tmp"
set "FAILED=0"
del "%TMPF%" >nul 2>&1

>>"%LOG%" echo.
>>"%LOG%" echo ===== test%TVER% RUN %DATE% %TIME% on %COMPUTERNAME% user %USERNAME% =====

call :say ================================================================
call :say  sparq test%TVER% - the long shared-mode soak on the UMC 204HD
call :say  exclusive acceptance follows in test004 once defect #77 is fixed
call :say  at the end you send back ONE file: test%TVER%.log from the repo root
call :say ================================================================

REM ---- [00] environment -------------------------------------------------
call :say " "
call :say [00] environment
set "CMD=ver"
call :run "os version"
if exist "crates\sparq-module-api\src\decode (2).rs" (
    call :say WARNING: stray decode copy artefact present - delete when convenient.
) else (
    call :say tree clean, no stray artefacts
)

REM ---- [01] prep ----------------------------------------------------------
call :say " "
call :say [01] PREP for a multi-hour run:
call :say   1. Default playback device = OUT 1-2 BEHRINGER UMC 204HD, powered, plugged.
call :say   2. Power plan HIGH PERFORMANCE - confirmed last run - sleep NEVER on AC.
call :say   3. Screen may turn off; the MACHINE must not sleep or lock into standby.
call :say   4. Close other audio apps. No downloads/updates scheduled mid-run.
call :say   5. Monitors DOWN - the soak makes sound the whole time.
call :say Press any key when ready.
pause >nul
call :say operator confirmed prep complete

REM ---- [02] build + stamp --------------------------------------------------
call :say " "
call :say [02] build - incremental, seconds if nothing changed
set "CMD_CLICK_STARTED="
set "SPARQ_DOUBLE_CLICKED="
set "CMD=call scripts\build.bat"
call :run "build via scripts\build.bat - includes the stamp guard"
set "RC02=!RC!"
if not "!RC02!"=="0" (
    call :say BUILD FAILED - send test%TVER%.log back now.
    goto summary
)
set "CMD=target\release\sparq.exe version"
call :run "version stamp"
set "RC02B=!RC!"

REM ---- [03] quick device sanity ---------------------------------------------
call :say " "
call :say [03] device sanity after any re-plugging - default should still be OUT 1-2
set "CMD=target\release\sparq.exe devices --caps"
call :run "devices --caps"
set "RC03=!RC!"

REM ---- [04] THE SOAK ----------------------------------------------------------
call :say " "
call :say [04] SOAK on !SOAKBE! at %RATE% Hz / %BLOCK% samples - the long run.
call :say      This is the SHARED rehearsal, not the exclusive acceptance - the log
call :say      says which, always. Zero xruns is the bar. Status every 30 s; the
call :say      window shows live output and the log receives every line as UTF-8.
call :say      Walk away when it starts; do not sleep the machine.
set "SOAKMIN=120"
set /p "SOAKMIN=      minutes [Enter = 120, 30 = short, 0 = skip]: "
if "!SOAKMIN!"=="0" (
    set "RC04=SKIP"
    call :say soak skipped by operator
) else (
    call :say soaking !SOAKMIN! minutes - starting now.
    powershell -NoProfile -Command "$ErrorActionPreference='SilentlyContinue'; & 'target\release\sparq.exe' soak --minutes !SOAKMIN! --report-every 30 --backend !SOAKBE! --rate %RATE% --block %BLOCK% 2>&1 | ForEach-Object { $s = [string]$_; Write-Host $s; Add-Content -LiteralPath '%LOG%' -Value $s -Encoding UTF8 }; exit $LASTEXITCODE"
    set "RC04=!ERRORLEVEL!"
)

REM ---- [05] post-soak health ----------------------------------------------------
call :say " "
call :say [05] post-soak health - conformance again, so the reopen-leak check runs
call :say      AFTER the long session - a leak that only shows after hours is the kind
call :say      this catches.
set "CMD=target\release\sparq.exe devices --conformance"
call :run "devices --conformance - post-soak"
set "RC05=!RC!"

REM ---- [06] summary ---------------------------------------------------------------
:summary
call :say " "
call :say ================================================================
call :say  SUMMARY - test%TVER% - %DATE% %TIME%
call :say ================================================================
if defined RC02 call :verdict "[02] build + stamp guard" "!RC02!"
if defined RC03 call :verdict "[03] devices --caps" "!RC03!"
if defined RC04 call :verdict "[04] soak !SOAKMIN! min shared 96k/64" "!RC04!"
if defined RC05 call :verdict "[05] post-soak conformance" "!RC05!"
call :say steps failing on rc: !FAILED!
call :say " "
call :say Next: increment 1.2 - integer exclusive rungs, defect #77 - then test004
call :say runs the EXCLUSIVE acceptance: tone, unplug, and the 2 h soak at 96k/64.
call :say " "
call :say SEND THIS ONE FILE BACK: %LOG%
call :say ================================================================
echo.
echo  done - you can close this window.
del "%TMPF%" >nul 2>&1
pause
exit /b 0

REM ============================== subroutines ==============================

:say
echo  %*
>>"%LOG%" echo  %*
exit /b 0

:run
REM CMD = the command line to execute; %1 = label for the log banners.
echo.
echo  ---- [RUN] %~1 ----
>>"%LOG%" echo.
>>"%LOG%" echo ---- [RUN] %~1 ----
%CMD% > "%TMPF%" 2>&1
set "RC=!ERRORLEVEL!"
type "%TMPF%"
type "%TMPF%" >> "%LOG%"
echo  ---- [RC !RC!] %~1 ----
>>"%LOG%" echo ---- [RC !RC!] %~1 ----
exit /b 0

:verdict
set "V=FAIL"
if "%~2"=="0" set "V=PASS"
if "%~2"=="" set "V=NOT-RUN"
if "%~2"=="SKIP" set "V=SKIP"
call :say %~1 : !V! rc=%~2
if "!V!"=="FAIL" set /a FAILED+=1
exit /b 0
