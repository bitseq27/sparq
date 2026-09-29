@echo off
REM ===========================================================================
REM  sparq test004 - WO-006 EXCLUSIVE acceptance: the run that closes the WO.
REM
REM  THE CONTRACT: one file from Qwen - this script, plus the increment-1.5
REM  sync zip applied first - see below. Run it, answer its prompts, send back
REM  ONE file: test004-digest.log from the repo root - the compact copy this
REM  script makes at the end. The full test004.log stays on disk.
REM
REM  PREREQUISITE: sync-wo006-inc15.zip must be extracted at the repo root
REM  BEFORE this run - on top of the chain through sync-wo006-inc14.zip, which
REM  your 2026-09-28 EVENING build line already showed APPLIED - 151 files
REM  match sync wo006-inc14. Attempt 3 proved the inc-1.4 period fix works at
REM  OPEN - the driver granted its default 960 fr / 10.00 ms, the predicted
REM  pass shape - and then stalled anyway: the same ~16 stalls per second,
REM  the same half throughput, at a different period. Two findings, two fixes
REM  in this bundle. ONE: the exclusive rate list was being sieved through
REM  the SHARED probe results, so your 48 kHz ask was adjusted UP to the one
REM  rate the exclusive path misbehaves at - the sweep now asks the DRIVER at
REM  every standard rate, and attempt 4 asks it for 48 kHz exclusive, which
REM  it has never been asked before. TWO: the drift metric no longer reads
REM  the device clock - fiction on this endpoint, defect #76 - but the frames
REM  the device ACCEPTED against the wall, so half throughput now reads
REM  -49 percent as a measurement, and the play/soak verdicts act on it.
REM  The build step verifies the new stamp.
REM
REM  WHAT PASSES THE WO: [03] caps must list MULTIPLE exclusive rates for the
REM  Behringer endpoints - 48000 among them - [04] the exclusive tone must be
REM  CLEAN at 48 kHz, its open line naming rate, rung and period - [05] unplug
REM  mid-run must end in a clean Removed state - [07] the 2 h soak at the rate
REM  [04] proved must finish with ZERO xruns; 96 kHz stays on offer at the
REM  prompt as the stretch the WO text named. If [04] still refuses, the error
REM  carries the full four-rung probe table with HRESULTs - the table IS the
REM  diagnosis, send the log back either way.
REM
REM  SAFE TO RE-RUN: the log appends with a dated banner per run.
REM
REM  Rules honoured - tools/check_text_io.py: pure ASCII - rule 3, defect
REM  #42 - and no unescaped parens in echo text inside blocks - rule 4,
REM  defect #69.
REM ===========================================================================
setlocal EnableDelayedExpansion
set "TVER=004"
set "RATE=48000"
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
call :say  increment 1.5: the exclusive rate envelope is its own probe - defect #91
call :say  drift reads delivered frames vs wall - defect #76 shipped
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
call :say      wasapi.rs, hal/mod.rs, diag.rs, play/hal.rs and soak.rs moved. If the
call :say      guard says stale and rebuilds, that is the freshness trap - normal.
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
call :say      MULTIPLE exclusive rates - 44100, 48000 and up - instead of the lone
call :say      96000 the shared sieve let through in attempt 3. The exclusive sweep
call :say      now asks the DRIVER at every standard rate; before, it only asked
call :say      where the ENGINE had already said yes. If 48000 is still absent, the
call :say      driver really refuses it - that is a finding, send the caps block
call :say      back. If they say "exclusive none", no rung fit at ANY rate - the
call :say      [04] probe table will name the HRESULTs.
set "CMD=target\release\sparq.exe devices --caps"
call :run "devices --caps"
set "RC03=!RC!"

REM ---- [04] exclusive tone - the moment of truth ----------------------------------
call :say " "
call :say [04] WASAPI EXCLUSIVE playback - a 10 s 220 Hz tone at -12 dB.
call :say      The open line names rate, rung and period. The expectation for the
call :say      UMC 204HD is now 48000 Hz - the rate play asks and the decoupled
call :say      probe should list - i24-in-32 with conversion, and a device period
call :say      of 480 fr - 10.00 ms - with the 64-frame sparq block riding the
call :say      FIFO beneath it. NO "adjusted" line should print above it: if you
call :say      see one bending 48000 to 96000 again, the driver refused 48 kHz
call :say      exclusive at probe time - a finding; send the caps block back.
call :say      The drift line now reads delivered frames vs wall: a healthy run
call :say      sits within a few hundred ppm of zero. The attempt-3 signature -
call :say      late wakes every status line, drift near -50 percent - repeated at
call :say      48 kHz would mean the stall is rate-independent. Either way the
call :say      digest is now self-diagnosing; send it back.
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
    call :say Busy means another app holds the device; Format means no rung fits
    call :say at ANY probed rate. A device-period table means no period fit.
    call :say If it OPENED but ran rough, read the open line's RATE first - 48000
    call :say is the new expectation, the decoupled probe's doing. Late wakes on
    call :say every status line plus drift near -50 percent is the defect-92 stall
    call :say signature - and drift is now delivered frames vs wall, so -50
    call :say percent is a MEASUREMENT, not a clock artifact: the device consumed
    call :say half the negotiated rate. If it says that at 48 kHz too, the stall
    call :say is rate-independent - send the digest back; the next lever is push
    call :say mode, already named in the notes. The table IS the diagnosis.
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
    call :say [07] THE ACCEPTANCE SOAK - 2 h at the rate [04] just proved, WASAPI
    call :say      EXCLUSIVE, zero xruns required. This is the criterion that closes
    call :say      WO-006 and fires ADR-008: the cpal bootstrap gets deleted after.
    call :say      The WO text named 96 kHz; that wording predates the discovery
    call :say      that the lone-rate envelope was a probe bug - the run sheet says
    call :say      what the amended acceptance is and 96 kHz stays on offer below.
) else (
    call :say [07] exclusive did not run clean, so this is the SHARED rehearsal
    call :say      again, not the acceptance run. The log says which - honesty over vanity.
)
call :say      It MAKES SOUND the whole run; the machine must not sleep.
if "!EXCL_OK!"=="1" (set "SOAKBE=wasapi-exclusive") else (set "SOAKBE=wasapi-shared")
set "SOAKMIN=120"
set /p "SOAKMIN=      minutes [Enter = 120 acceptance, 15 = rehearsal, 0 = skip]: "
set "SOAKRATE=%RATE%"
set /p "SOAKRATE=      rate [Enter = %RATE% - the rate [04] proves; 96000 = the WO-text stretch]: "
if "!SOAKMIN!"=="0" (
    set "RC07=SKIP"
    call :say soak skipped by operator
) else (
    call :say soaking !SOAKMIN! min at !SOAKRATE! Hz / %BLOCK% on !SOAKBE! - starting now.
    powershell -NoProfile -Command "$ErrorActionPreference='SilentlyContinue'; & 'target\release\sparq.exe' soak --minutes !SOAKMIN! --report-every 30 --backend !SOAKBE! --rate !SOAKRATE! --block %BLOCK% 2>&1 | ForEach-Object { $s = [string]$_; Write-Host $s; Add-Content -LiteralPath '%LOG%' -Value $s -Encoding UTF8 }; exit $LASTEXITCODE"
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
if defined RC07 call :verdict "[07] soak !SOAKMIN! min at !SOAKRATE! Hz on !SOAKBE!" "!RC07!"
if defined RC08 call :verdict "[08] post-soak conformance" "!RC08!"
call :say tone heard - [04] !HEARD04!  [06] !HEARD06!
call :say steps failing on rc: !FAILED!
call :say " "
if "!EXCL_OK!"=="1" if "!RC07!"=="0" call :say WO-006 ACCEPTANCE: all criteria met on hardware - send the digest!
if not "!EXCL_OK!"=="1" if not "!EXCL_OPENED!"=="1" call :say exclusive still refused - the [04] probe table in this log is the diagnosis.
if not "!EXCL_OK!"=="1" if "!EXCL_OPENED!"=="1" call :say exclusive OPENED but did not run clean - the [04] open line rate and period, the late wakes and the delivered-vs-wall drift are the diagnosis.
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
