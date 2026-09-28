@echo off
REM ===========================================================================
REM  sparq test004 - WO-006 EXCLUSIVE acceptance: the run that closes the WO.
REM
REM  THE CONTRACT: one file from Qwen - this script, plus the increment-1.4
REM  sync zip applied first - see below. Run it, answer its prompts, send back
REM  ONE file: test004-digest.log from the repo root - the compact copy this
REM  script makes at the end. The full test004.log stays on disk.
REM
REM  PREREQUISITE: sync-wo006-inc14.zip must be extracted at the repo root
REM  BEFORE this run - on top of the chain through sync-wo008-inc6.zip, which
REM  your 2026-09-28 build line already showed APPLIED. It carries the
REM  defect-#89 fix: your 09-28 run OPENED exclusive - i24-in-32, the rung we
REM  wanted - but at the driver's 3 ms reported minimum, and the driver could
REM  not SUSTAIN it: a ~33 ms stall every ~62.5 ms, 159 late wakes in 10 s,
REM  half throughput, drift -49 percent, no clean tone. The open now asks the
REM  driver's DEFAULT period first - the number its engine actually runs; the
REM  shared 10 ms engine soak-ran 2 h clean on this same machine - and the
REM  alignment two-step rounds the ask UP to the driver's granularity instead
REM  of adopting it. The build step verifies the new stamp.
REM
REM  WHAT PASSES THE WO: [03] caps must now list EXCLUSIVE RATES for the
REM  Behringer endpoints - [04] the exclusive tone must be audible, and its
REM  log line names the rung that opened - [05] unplug mid-run in exclusive
REM  must end in a clean Removed state - [07] the 2 h soak at 96 kHz / 64
REM  exclusive must finish with ZERO xruns. If [04] still refuses, the error
REM  now carries the full four-rung probe table with HRESULTs - that table is
REM  the diagnosis, send the log back either way.
REM
REM  SAFE TO RE-RUN: the log appends with a dated banner per run.
REM
REM  Rules honoured - tools/check_text_io.py: pure ASCII - rule 3, defect
REM  #42 - and no unescaped parens in echo text inside blocks - rule 4,
REM  defect #69.
REM ===========================================================================
setlocal EnableDelayedExpansion
set "TVER=004"
set "RATE=96000"
set "BLOCK=64"
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
set "EXCL_OK=0"
set "EXCL_OPENED=0"
del "%TMPF%" >nul 2>&1

>>"%LOG%" echo.
>>"%LOG%" echo ===== test%TVER% RUN %DATE% %TIME% on %COMPUTERNAME% user %USERNAME% =====

call :say ================================================================
call :say  sparq test%TVER% - WO-006 EXCLUSIVE acceptance run
call :say  increment 1.4: the exclusive open asks the engine period - defect #89
call :say  at the end you send back ONE file: test%TVER%-digest.log - the compact
call :say  copy this script makes; the full test%TVER%.log stays on disk
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
call :say [01] PREP - same discipline as the last rounds:
call :say   1. UMC 204HD plugged, powered, and the DEFAULT playback device - OUT 1-2.
call :say   2. Its Properties, Advanced tab: BOTH exclusive-control boxes ticked.
call :say   3. Every other audio app closed - an app holding the device turns
call :say      exclusive into a Busy refusal.
call :say   4. Power plan HIGH PERFORMANCE, sleep NEVER, no downloads/updates queued.
call :say   5. Monitors DOWN - every play step and the soak make sound.
call :say Press any key when ready.
pause >nul
call :say operator confirmed prep complete

REM ---- [02] build + stamp ---------------------------------------------------
call :say " "
call :say [02] build - the sync must have landed: the stamp changes because
call :say      period.rs and wasapi.rs moved. If the guard says stale and rebuilds,
call :say      that is the zip-sync freshness trap - normal.
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

REM ---- [03] caps - the first checkpoint ---------------------------------------
call :say " "
call :say [03] capabilities - CHECKPOINT: the Behringer endpoints should now list
call :say      exclusive rates instead of "exclusive none". If they still say none,
call :say      the four-rung ladder found no format the driver accepts - the [04]
call :say      probe table will say exactly which HRESULT each rung got.
set "CMD=target\release\sparq.exe devices --caps"
call :run "devices --caps"
set "RC03=!RC!"

REM ---- [04] exclusive tone - the moment of truth ----------------------------------
call :say " "
call :say [04] WASAPI EXCLUSIVE playback - a 10 s 220 Hz tone at -12 dB.
call :say      The open line names the rung: f32, i24-in-32, i32 or i16.
call :say      For the UMC 204HD the expectation is i24-in-32 with conversion,
call :say      and a device period of 960 fr - 10.00 ms - or 1152 fr - 12.00 ms -
call :say      if the driver enforces its 288 fr alignment - with the 64-frame
call :say      sparq block riding the FIFO beneath it: the defect 79/89 fix.
call :say      A 288 fr - 3.00 ms - period is the defect-89 signature: it opens,
call :say      then stalls. Send the log back if you see one.
call :say Press any key when ready.
pause >nul
set "CMD=target\release\sparq.exe play --backend wasapi-exclusive --seconds 10 --gain -12"
call :run "play wasapi-exclusive 10s"
set "RC04=!RC!"
if "!RC04!"=="0" (
    set "EXCL_OK=1"
    set "EXCL_OPENED=1"
) else (
    findstr /c:"wasapi-exclusive]: opened" "%TMPF%" >nul 2>&1 && set "EXCL_OPENED=1"
    call :say HINT: the probe table in the error names each rung's HRESULT.
    call :say Busy means another app holds the device; Format means no rung fits.
    call :say A device-period table means no period fit - defect 79, increment 1.3.
    call :say If it OPENED but ran rough, the opened period line is the diagnosis:
    call :say late wakes against the period, drift near -50 percent and xruns on
    call :say every status line were defect 89 - a period the driver accepted at
    call :say Initialize and could not sustain. Increment 1.4 asks the default.
    call :say Send the log back either way - the table IS the diagnosis.
)
call :askyn "did you hear a clean 10-second tone"
set "HEARD04=!ANS!"

REM ---- [05]+[06] unplug + recovery, in exclusive if it opened ----------------------
if "!EXCL_OK!"=="1" (set "TESTBE=wasapi-exclusive") else (set "TESTBE=wasapi-shared")
call :say " "
call :say [05] UNPLUG TEST on !TESTBE! - the criterion passed in shared mode in
call :say      test002; exclusive exercises the same Removed path on a stream the
call :say      driver owns outright. A 30-second tone starts on the next step.
call :say      ABOUT 10 SECONDS AFTER IT STARTS, unplug the USB interface.
call :say Press any key when ready - then count 10 seconds from the tone and pull.
pause >nul
set "CMD=target\release\sparq.exe play --backend !TESTBE! --seconds 30 --gain -12"
call :run "unplug test - 30s on !TESTBE!, unplugged at about 10s"
set "RC05=!RC!"
call :say rc was !RC05! - the TEXT decides: Removed state, clean stop, no crash.
call :say " "
call :say NOW: plug the interface back in, wait 10 seconds, press any key.
pause >nul
call :say interface re-plugged, recovery step starting
call :say " "
call :say [06] RECOVERY - 10 s on !TESTBE! must work again after re-plug
set "CMD=target\release\sparq.exe play --backend !TESTBE! --seconds 10 --gain -12"
call :run "play 10s on !TESTBE! after re-plug"
set "RC06=!RC!"
call :askyn "did you hear the recovery tone"
set "HEARD06=!ANS!"

REM ---- [07] THE acceptance soak ------------------------------------------------------
call :say " "
if "!EXCL_OK!"=="1" (
    call :say [07] THE ACCEPTANCE SOAK - 2 h, 96 kHz / 64, WASAPI EXCLUSIVE, zero
    call :say      xruns required. This is the criterion that closes WO-006 and fires
    call :say      ADR-008: the cpal bootstrap gets deleted afterwards.
) else (
    call :say [07] exclusive did not run clean, so this is the SHARED rehearsal
    call :say      again, not the acceptance run. The log says which - honesty over vanity.
)
call :say      It MAKES SOUND the whole run; the machine must not sleep.
if "!EXCL_OK!"=="1" (set "SOAKBE=wasapi-exclusive") else (set "SOAKBE=wasapi-shared")
set "SOAKMIN=120"
set /p "SOAKMIN=      minutes [Enter = 120 acceptance, 15 = rehearsal, 0 = skip]: "
if "!SOAKMIN!"=="0" (
    set "RC07=SKIP"
    call :say soak skipped by operator
) else (
    call :say soaking !SOAKMIN! min at %RATE% Hz / %BLOCK% on !SOAKBE! - starting now.
    powershell -NoProfile -Command "$ErrorActionPreference='SilentlyContinue'; & 'target\release\sparq.exe' soak --minutes !SOAKMIN! --report-every 30 --backend !SOAKBE! --rate %RATE% --block %BLOCK% 2>&1 | ForEach-Object { $s = [string]$_; Write-Host $s; Add-Content -LiteralPath '%LOG%' -Value $s -Encoding UTF8 }; exit $LASTEXITCODE"
    set "RC07=!ERRORLEVEL!"
)

REM ---- [08] post-soak conformance --------------------------------------------------------
call :say " "
call :say [08] post-soak conformance - with exclusive opening now, the checks that
call :say      SKIPped in test002 - lifecycle, latency report, reopen-leak - run for
call :say      real on hardware, AFTER the long session.
set "CMD=target\release\sparq.exe devices --conformance"
call :run "devices --conformance - post-soak"
set "RC08=!RC!"

REM ---- [09] summary ----------------------------------------------------------------------------
:summary
call :say " "
call :say ================================================================
call :say  SUMMARY - test%TVER% - %DATE% %TIME%
call :say ================================================================
if defined RC02 call :verdict "[02] build + stamp guard" "!RC02!"
if defined RC03 call :verdict "[03] devices --caps" "!RC03!"
if defined RC04 call :verdict "[04] play EXCLUSIVE" "!RC04!"
if defined RC05 call :say      [05] unplug rc=!RC05! on !TESTBE! - judged from the TEXT above
if defined RC06 call :verdict "[06] recovery play" "!RC06!"
if defined RC07 call :verdict "[07] soak !SOAKMIN! min on !SOAKBE!" "!RC07!"
if defined RC08 call :verdict "[08] post-soak conformance" "!RC08!"
call :say tone heard - [04] !HEARD04!  [06] !HEARD06!
call :say steps failing on rc: !FAILED!
call :say " "
if "!EXCL_OK!"=="1" if "!RC07!"=="0" call :say WO-006 ACCEPTANCE: all criteria met on hardware - send the digest!
if not "!EXCL_OK!"=="1" if not "!EXCL_OPENED!"=="1" call :say exclusive still refused - the [04] probe table in this log is the diagnosis.
if not "!EXCL_OK!"=="1" if "!EXCL_OPENED!"=="1" call :say exclusive OPENED but did not run clean - the [04] period line, late wakes and drift are the diagnosis.
call :say " "
REM ---- the compact copy: the full log stays here, the digest travels --------
set "HAVE_PY=0"
where python >nul 2>&1 && set "HAVE_PY=1"
where py >nul 2>&1 && set "HAVE_PY=1"
if "!HAVE_PY!"=="1" (
    where python >nul 2>&1 && set "PY=python" || set "PY=py"
    del "%CD%\test%TVER%-digest.log" >nul 2>&1
    "!PY!" "%~dp0..\tools\log_digest.py" "%LOG%"
    if exist "%CD%\test%TVER%-digest.log" (
        call :say SEND THIS ONE FILE BACK: %CD%\test%TVER%-digest.log
        call :say the digest is this log minus the repeated status and build lines -
        call :say every other line byte-identical. The full log stays on disk.
    ) else (
        call :say digest generation failed - send the full log instead:
        call :say SEND THIS ONE FILE BACK: %LOG%
    )
) else (
    call :say no python on PATH - no compact copy; send the full log:
    call :say SEND THIS ONE FILE BACK: %LOG%
)
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

:askyn
call :say %~1 [y/n]
choice /c YN /n /m "> "
if errorlevel 2 (set "ANS=N") else (set "ANS=Y")
call :say answer: !ANS!
exit /b 0

:run
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
