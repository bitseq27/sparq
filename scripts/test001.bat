@echo off
REM ===========================================================================
REM  sparq test001 - WO-006 physical-machine acceptance, one guided session,
REM  one log.
REM
REM  THE CONTRACT: Qwen sends you ONE file - this script. You drop it into the
REM  scripts folder of the sparq repo on SATURN, run it, and answer its
REM  prompts. It writes ONE file - test001.log in the repo root, next to
REM  README.md - and you send that one file back. Version convention:
REM  testNNN.bat always writes testNNN.log, so rounds never get mixed up.
REM
REM  WHAT THIS VERSION TESTS - first physical run since the audio driver
REM  reinstall - build + stamp guard, device enumeration and capabilities,
REM  HAL conformance, WASAPI EXCLUSIVE playback - refused over RDP, must pass
REM  at the machine, and the first time anyone hears sparq through the HAL -
REM  shared playback, the unplug and recovery test, and the zero-xrun
REM  acceptance soak at 96 kHz / 64 exclusive.
REM
REM  SAFE TO RE-RUN: the log appends with a dated banner per run, so a failed
REM  attempt plus a retry both survive in the one file you send back.
REM
REM  Rules honoured - tools/check_text_io.py: pure ASCII - rule 3, defect
REM  #42 - and no unescaped parens in echo text inside blocks - rule 4,
REM  defect #69.
REM ===========================================================================
setlocal EnableDelayedExpansion
set "TVER=001"
set "RATE=96000"
set "BLOCK=64"
title sparq test%TVER%
chcp 65001 >nul 2>&1
cd /d "%~dp0.."

if not exist "scripts\build.bat" (
    echo  test%TVER%.bat must sit in the scripts folder of the sparq repo,
    echo  so that scripts\build.bat is one level down from its parent.
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
call :say  sparq test%TVER% - WO-006 physical acceptance run
call :say  everything shown here is also written to test%TVER%.log
call :say  at the end you send back ONE file: test%TVER%.log from the repo root
call :say ================================================================

REM ---- [00] environment -------------------------------------------------
call :say " "
call :say [00] environment
set "CMD=ver"
call :run "os version"
call :say processor: %PROCESSOR_IDENTIFIER%
call :say cores: %NUMBER_OF_PROCESSORS%

REM ---- [01] stray zip-artefact guard - defect #71 ------------------------
call :say " "
call :say [01] stray zip-artefact guard - defect #71
if exist "crates\sparq-module-api\src\decode (2).rs" (
    call :say WARNING: the stray decode copy artefact is present in
    call :say crates\sparq-module-api\src - delete it when convenient. It does not
    call :say block this run, but it changes the build fingerprint.
) else (
    call :say no stray copy artefacts - tree is clean
)

REM ---- [02] operator prep checklist --------------------------------------
call :say " "
call :say [02] PREP - do these five now; they decide whether the numbers mean anything
call :say   1. Power plan HIGH PERFORMANCE, and sleep set to NEVER while on AC.
call :say   2. Close every other audio app - browser, DAW, players, chat apps.
call :say   3. Sound settings - your interface - Properties - Advanced tab:
call :say      tick ALLOW APPLICATIONS TO TAKE EXCLUSIVE CONTROL OF THIS DEVICE.
call :say   4. Make the USB interface the DEFAULT playback device.
call :say   5. Monitors or headphones DOWN - several steps make sound.
call :say When all five are done, press any key to continue.
pause >nul
call :say operator confirmed prep complete

REM ---- [03] build + stamp ------------------------------------------------
call :say " "
call :say [03] build - release, hal-wasapi + bootstrap-audio + ui-window.
call :say      First build takes a few minutes; its output appears when it finishes.
set "CMD_CLICK_STARTED="
set "SPARQ_DOUBLE_CLICKED="
set "CMD=call scripts\build.bat"
call :run "build via scripts\build.bat - includes the stamp guard"
set "RC03=!RC!"
if not "!RC03!"=="0" (
    call :say BUILD FAILED - nothing else can run. Send test%TVER%.log back now.
    goto summary
)
set "CMD=target\release\sparq.exe version"
call :run "version stamp"
set "RC03B=!RC!"

REM ---- [04] devices -------------------------------------------------------
call :say " "
call :say [04] device enumeration - the interface should be visible now; RDP hid it
set "CMD=target\release\sparq.exe devices"
call :run "devices"
set "RC04=!RC!"

REM ---- [05] capabilities ---------------------------------------------------
call :say " "
call :say [05] capabilities - rates, channels, exclusive support, per backend
set "CMD=target\release\sparq.exe devices --caps"
call :run "devices --caps"
set "RC05=!RC!"

REM ---- [06] conformance ----------------------------------------------------
call :say " "
call :say [06] conformance suite - briefly opens the default device with a -60 dB signal
set "CMD=target\release\sparq.exe devices --conformance"
call :run "devices --conformance"
set "RC06=!RC!"

REM ---- [07] exclusive playback - the RDP blocker ---------------------------
call :say " "
call :say [07] WASAPI EXCLUSIVE playback - a 10 s 220 Hz tone at -12 dB.
call :say      Over RDP this was always honestly refused; here it must pass.
call :say      The tone starts within a couple of seconds of the next step.
call :say Press any key when ready.
pause >nul
set "CMD=target\release\sparq.exe play --backend wasapi-exclusive --seconds 10 --gain -12"
call :run "play wasapi-exclusive 10s"
set "RC07=!RC!"
if not "!RC07!"=="0" call :say HINT: a Busy or Format error usually means another app holds the device,
if not "!RC07!"=="0" call :say or exclusive control is off - Sound settings, device Properties, Advanced.
if not "!RC07!"=="0" call :say Fix it and re-run this script; re-running is safe, the log appends.
call :askyn "did you hear a clean 10-second tone"
set "HEARD07=!ANS!"

REM ---- [08] shared playback -------------------------------------------------
call :say " "
call :say [08] WASAPI SHARED playback - same 10 s tone, the fallback path
call :say Press any key when ready.
pause >nul
set "CMD=target\release\sparq.exe play --backend wasapi-shared --seconds 10 --gain -12"
call :run "play wasapi-shared 10s"
set "RC08=!RC!"
call :askyn "did you hear a clean 10-second tone"
set "HEARD08=!ANS!"

REM ---- [09] unplug test -------------------------------------------------------
call :say " "
call :say [09] UNPLUG TEST - acceptance: removing the device mid-run must give a
call :say      clean error state and a recoverable stop, not a crash or a hang.
call :say      A 30-second tone starts on the next step. ABOUT 10 SECONDS AFTER IT
call :say      STARTS, physically unplug the USB interface. Leave it unplugged.
call :say Press any key when ready - then count 10 seconds from the tone and pull.
pause >nul
set "CMD=target\release\sparq.exe play --backend wasapi-exclusive --seconds 30 --gain -12"
call :run "unplug test - exclusive 30s, interface unplugged at about 10s"
set "RC09=!RC!"
call :say rc for the unplug run was !RC09! - the TEXT above decides, not the rc:
call :say expected is a Removed-device error, a clean stop, no hang, no crash.
call :say " "
call :say NOW: plug the interface back in, wait 10 seconds for Windows to settle,
call :say then press any key.
pause >nul
call :say interface re-plugged, recovery step starting

REM ---- [10] recovery -----------------------------------------------------------
call :say " "
call :say [10] RECOVERY - the same 10 s exclusive tone must work again after re-plug
set "CMD=target\release\sparq.exe play --backend wasapi-exclusive --seconds 10 --gain -12"
call :run "play wasapi-exclusive 10s after re-plug"
set "RC10=!RC!"
call :askyn "did you hear the recovery tone"
set "HEARD10=!ANS!"

REM ---- [11] acceptance soak ------------------------------------------------------
call :say " "
call :say [11] ACCEPTANCE SOAK - the headline criterion: ZERO xruns, idle system.
call :say      Paced through the HAL at %RATE% Hz / %BLOCK% samples, exclusive mode,
call :say      so it MAKES SOUND for the whole run - set monitors low now.
call :say      The machine must not sleep; this window must stay open.
set "SOAKMIN=120"
set /p "SOAKMIN=      minutes [Enter = 120 acceptance, 15 = rehearsal, 0 = skip]: "
if "!SOAKMIN!"=="0" (
    set "RC11=SKIP"
    call :say soak skipped by operator
) else (
    call :say soaking !SOAKMIN! minutes - live output below; every line also goes to the log.
    call :say status reports arrive every 30 seconds. Ctrl+C aborts; the log keeps what ran.
    powershell -NoProfile -Command "& 'target\release\sparq.exe' soak --minutes !SOAKMIN! --report-every 30 --backend wasapi-exclusive --rate %RATE% --block %BLOCK% 2>&1 | Tee-Object -FilePath '%LOG%' -Append; exit $LASTEXITCODE"
    set "RC11=!ERRORLEVEL!"
)

REM ---- [12] summary ----------------------------------------------------------------
:summary
call :say " "
call :say ================================================================
call :say  SUMMARY - test%TVER% - %DATE% %TIME%
call :say ================================================================
call :verdict "[03] build + stamp guard" "!RC03!"
if defined RC03B call :verdict "[03b] version stamp" "!RC03B!"
call :verdict "[04] devices" "!RC04!"
call :verdict "[05] devices --caps" "!RC05!"
call :verdict "[06] conformance" "!RC06!"
call :verdict "[07] play exclusive" "!RC07!"
call :verdict "[08] play shared" "!RC08!"
call :say      [09] unplug rc=!RC09! - judged from the TEXT, see the run above
call :verdict "[10] recovery play" "!RC10!"
call :verdict "[11] acceptance soak" "!RC11!"
call :say tone heard - [07] !HEARD07!  [08] !HEARD08!  [10] !HEARD10!
call :say steps failing on rc: !FAILED!
call :say " "
call :say SEND THIS ONE FILE BACK: %LOG%
call :say ================================================================
echo.
echo  done - you can close this window.
del "%TMPF%" >nul 2>&1
pause
exit /b 0

REM ============================== subroutines ==============================
REM Every helper writes to BOTH the console and the log, so the terminal is
REM the operator's guide and the log is the complete transcript of the session
REM - instructions, answers, command output and verdicts, in order.

:say
REM %* = one line of text, printed and logged verbatim.
echo  %*
>>"%LOG%" echo  %*
exit /b 0

:askyn
REM %1 = question; sets ANS to Y or N; the answer is logged.
call :say %~1 [y/n]
choice /c YN /n /m "> "
if errorlevel 2 (set "ANS=N") else (set "ANS=Y")
call :say answer: !ANS!
exit /b 0

:run
REM CMD = the command line to execute; %1 = label for the log banners.
REM Output is tee-d through a temp file so console and log stay identical.
REM Sets RC to the command's exit code.
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
REM %1 = step label, %2 = rc value or SKIP or empty; counts FAILs.
set "V=FAIL"
if "%~2"=="0" set "V=PASS"
if "%~2"=="" set "V=NOT-RUN"
if "%~2"=="SKIP" set "V=SKIP"
call :say %~1 : !V! rc=%~2
if "!V!"=="FAIL" set /a FAILED+=1
exit /b 0
