@echo off
REM ===========================================================================
REM  sparq test002 - WO-006 acceptance, round 2: identify the real device,
REM  then re-aim the exclusive-mode acceptance at it.
REM
REM  THE CONTRACT: one file from Qwen - this script. Run it, answer its
REM  prompts, send back ONE file: test002.log from the repo root.
REM
REM  WHY ROUND 2. test001 proved the HAL healthy in shared mode - the 10 s
REM  tone was AUDIBLE, 15112 blocks, 0 xruns, 0 allocations, clean stop -
REM  but every endpoint sparq could see was anonymous, single-rate and
REM  exclusive-less, wearing the suspect-clock signature of a virtual
REM  endpoint. Exclusive play, the unplug test and the soak therefore had no
REM  real device to run against. test002 first captures Windows-side ground
REM  truth - real endpoint names from PnP and from the MMDevices registry,
REM  which bypasses the COM property-store path that is being denied - then
REM  lets the operator aim every play step at a chosen device index, then
REM  re-runs the acceptance, falling back to shared mode only where
REM  exclusive still refuses, and saying so loudly in the log.
REM
REM  ALSO FIXED since test001 - harness defect #74: the soak tee wrote
REM  UTF-16 into the log and PowerShell decorated stderr as
REM  NativeCommandError, so the soak section arrived as spaced-out mojibake.
REM  The soak now logs line-by-line as UTF-8 with stderr as plain text.
REM
REM  SAFE TO RE-RUN: the log appends with a dated banner per run.
REM
REM  Rules honoured - tools/check_text_io.py: pure ASCII - rule 3, defect
REM  #42 - and no unescaped parens in echo text inside blocks - rule 4,
REM  defect #69.
REM ===========================================================================
setlocal EnableDelayedExpansion
set "TVER=002"
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
del "%TMPF%" >nul 2>&1

>>"%LOG%" echo.
>>"%LOG%" echo ===== test%TVER% RUN %DATE% %TIME% on %COMPUTERNAME% user %USERNAME% =====

call :say ================================================================
call :say  sparq test%TVER% - WO-006 acceptance round 2: find the real device
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
    call :say crates\sparq-module-api\src - delete it when convenient.
) else (
    call :say no stray copy artefacts - tree is clean
)

REM ---- [02] WINDOWS-SIDE GROUND TRUTH ------------------------------------
call :say " "
call :say [02] Windows audio ground truth - what the OS itself sees.
call :say      This bypasses sparq entirely; it settles which endpoints are real.
call :say " "
call :say [02a] sessions - a lingering remote session keeps Remote Audio alive
set "CMD=qwinsta"
call :run "qwinsta - session table"
call :say " "
call :say [02b] power scheme - HIGH PERFORMANCE expected for acceptance numbers
set "CMD=powercfg /getactivescheme"
call :run "powercfg active scheme"
call :say " "
call :say [02c] PnP audio endpoints - real NAMES and states. Status OK means
call :say      present and working; UNKNOWN usually means not present.
set "CMD=powershell -NoProfile -Command "$d = Get-PnpDevice -Class AudioEndpoint -ErrorAction SilentlyContinue; foreach ($x in $d) { Write-Host ($x.Status.ToString().PadRight(9) + $x.FriendlyName + '   [' + $x.InstanceId + ']') }""
call :run "pnp AudioEndpoint table"
call :say " "
call :say [02d] sound devices and drivers
set "CMD=powershell -NoProfile -Command "$d = Get-CimInstance Win32_SoundDevice -ErrorAction SilentlyContinue; foreach ($x in $d) { Write-Host ($x.Name + '   status=' + $x.Status + ' info=' + $x.StatusInfo) }""
call :run "Win32_SoundDevice table"
call :say " "
call :say [02e] MMDevices registry - name and state per endpoint GUID, read from
call :say      the registry instead of the COM property store that is being denied.
call :say      state: 1 = ACTIVE, 2 = DISABLED, 4 = NOT PRESENT, 8 = UNPLUGGED.
call :say      The GUIDs here match the ids sparq lists in step [05] one for one.
set "CMD=powershell -NoProfile -Command "foreach ($k in 'Render','Capture') { Write-Host ('== MMDevices ' + $k + ' =='); $keys = Get-ChildItem ('HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\' + $k) -ErrorAction SilentlyContinue; foreach ($x in $keys) { $st = (Get-ItemProperty -Path $x.PSPath -Name DeviceState -ErrorAction SilentlyContinue).DeviceState; $nm = (Get-ItemProperty -Path ($x.PSPath + '\Properties') -Name '{a45c254e-df1c-4efd-8020-67d146a850e0},2' -ErrorAction SilentlyContinue).'{a45c254e-df1c-4efd-8020-67d146a850e0},2'; Write-Host ($x.PSChildName + '  state=' + $st + '  name=' + $nm) } }""
call :run "MMDevices registry walk - render and capture"

REM ---- [03] operator prep -------------------------------------------------
call :say " "
call :say [03] PREP - with the tables above in view, do these now:
call :say   1. Is the physical interface listed at all - name, state=1 ACTIVE?
call :say      If NOT: check its USB cable and power, replug it, watch [02c] change.
call :say   2. Sound settings: make the INTERFACE the default playback device.
call :say      The soak has no device picker - it always uses the default.
call :say   3. Interface Properties, Advanced tab: tick BOTH
call :say      ALLOW APPLICATIONS TO TAKE EXCLUSIVE CONTROL and
call :say      GIVE EXCLUSIVE MODE APPLICATIONS PRIORITY. A driver reinstall
call :say      resets these - this alone can explain every exclusive refusal.
call :say   4. If [02a] shows a remote session still connected, log it off -
call :say      Remote Audio endpoints survive sessions and steal the default.
call :say   5. Power plan HIGH PERFORMANCE, sleep NEVER, other audio apps closed,
call :say      monitors DOWN - several steps make sound.
call :say Note the sparq device INDEX of the interface - you will type it below.
call :say When done, press any key.
pause >nul
call :askyn "is the interface now default, with exclusive control ticked"
set "PREP_OK=!ANS!"

REM ---- [04] build + stamp --------------------------------------------------
call :say " "
call :say [04] build - incremental; a few minutes only if sources changed
set "CMD_CLICK_STARTED="
set "SPARQ_DOUBLE_CLICKED="
set "CMD=call scripts\build.bat"
call :run "build via scripts\build.bat - includes the stamp guard"
set "RC04=!RC!"
if not "!RC04!"=="0" (
    call :say BUILD FAILED - nothing else can run. Send test%TVER%.log back now.
    goto summary
)
set "CMD=target\release\sparq.exe version"
call :run "version stamp"
set "RC04B=!RC!"

REM ---- [05]/[06] sparq's own view -------------------------------------------
call :say " "
call :say [05] sparq device list - match these GUIDs against the [02e] names
set "CMD=target\release\sparq.exe devices"
call :run "devices"
set "RC05=!RC!"
call :say " "
call :say [06] sparq capabilities - watch for the interface: a real device lists
call :say      MULTIPLE rates and, with exclusive ticked, exclusive rates too
set "CMD=target\release\sparq.exe devices --caps"
call :run "devices --caps"
set "RC06=!RC!"

REM ---- [07] conformance ------------------------------------------------------
call :say " "
call :say [07] conformance suite - briefly opens the default device at -60 dB
set "CMD=target\release\sparq.exe devices --conformance"
call :run "devices --conformance"
set "RC07=!RC!"

REM ---- [08] choose the target device ------------------------------------------
call :say " "
call :say [08] TARGET - which sparq device index should the play, unplug and
call :say      recovery steps use? Compare [05] against the [02e] names.
set "DEVIDX="
set /p "DEVIDX=      device index [Enter = the Windows default, or type a number]: "
set "DEVARG="
if not "!DEVIDX!"=="" set "DEVARG=--device !DEVIDX!"
call :say target selection: !DEVARG!
if not "!DEVIDX!"=="" call :say NOTE: the soak in step [12] cannot take --device - make sure the
if not "!DEVIDX!"=="" call :say       chosen interface is also the Windows DEFAULT playback device.

REM ---- [09] exclusive playback ---------------------------------------------------
call :say " "
call :say [09] WASAPI EXCLUSIVE playback - a 10 s 220 Hz tone at -12 dB.
call :say      This is the step RDP could never pass and test001 could not reach.
call :say Press any key when ready.
pause >nul
set "CMD=target\release\sparq.exe play --backend wasapi-exclusive --seconds 10 --gain -12 !DEVARG!"
call :run "play wasapi-exclusive 10s"
set "RC09=!RC!"
if "!RC09!"=="0" (
    set "EXCL_OK=1"
) else (
    set "EXCL_OK=0"
    call :say HINT: still refused - recheck the exclusive tickboxes from [03] step 3,
    call :say close any app holding the device, and confirm the target index is the
    call :say interface. Later steps fall back to shared mode and say so.
)
call :askyn "did you hear a clean 10-second tone"
set "HEARD09=!ANS!"

REM ---- [10] shared playback --------------------------------------------------------
call :say " "
call :say [10] WASAPI SHARED playback - the same 10 s tone through the fallback path
call :say Press any key when ready.
pause >nul
set "CMD=target\release\sparq.exe play --backend wasapi-shared --seconds 10 --gain -12 !DEVARG!"
call :run "play wasapi-shared 10s"
set "RC10=!RC!"
call :askyn "did you hear a clean 10-second tone"
set "HEARD10=!ANS!"

REM ---- [11] unplug + recovery ---------------------------------------------------------
if "!EXCL_OK!"=="1" (set "TESTBE=wasapi-exclusive") else (set "TESTBE=wasapi-shared")
call :say " "
call :say [11] UNPLUG TEST on backend !TESTBE! - removing the device mid-run must
call :say      give a clean error state and a recoverable stop, not a crash or hang.
call :say      A 30-second tone starts on the next step. ABOUT 10 SECONDS AFTER IT
call :say      STARTS, physically unplug the USB interface. Leave it unplugged.
call :say Press any key when ready - then count 10 seconds from the tone and pull.
pause >nul
set "CMD=target\release\sparq.exe play --backend !TESTBE! --seconds 30 --gain -12 !DEVARG!"
call :run "unplug test - 30s on !TESTBE!, interface unplugged at about 10s"
set "RC11=!RC!"
call :say rc for the unplug run was !RC11! - the TEXT above decides, not the rc:
call :say expected is a Removed-device error, a clean stop, no hang, no crash.
call :say " "
call :say NOW: plug the interface back in, wait 10 seconds for Windows to settle,
call :say then press any key.
pause >nul
call :say interface re-plugged, recovery step starting
call :say " "
call :say [12] RECOVERY - the same 10 s tone on !TESTBE! must work again after re-plug
set "CMD=target\release\sparq.exe play --backend !TESTBE! --seconds 10 --gain -12 !DEVARG!"
call :run "play 10s on !TESTBE! after re-plug"
set "RC12=!RC!"
call :askyn "did you hear the recovery tone"
set "HEARD12=!ANS!"

REM ---- [13] acceptance soak ---------------------------------------------------------------
call :say " "
if "!EXCL_OK!"=="1" (set "SOAKBE=wasapi-exclusive") else (set "SOAKBE=wasapi-shared")
call :say [13] SOAK on backend !SOAKBE! - the headline criterion is ZERO xruns.
if "!SOAKBE!"=="wasapi-shared" call :say      NOTE: exclusive refused earlier, so this is the SHARED rehearsal,
if "!SOAKBE!"=="wasapi-shared" call :say      not the acceptance run. The log says which - honesty over vanity.
call :say      It MAKES SOUND the whole run; the machine must not sleep.
set "RATE=96000"
set /p "RATE=      rate [Enter = 96000, or type e.g. 48000 to match the device]: "
set "SOAKMIN=120"
set /p "SOAKMIN=      minutes [Enter = 120 acceptance, 15 = rehearsal, 0 = skip]: "
if "!SOAKMIN!"=="0" (
    set "RC13=SKIP"
    call :say soak skipped by operator
) else (
    call :say soaking !SOAKMIN! min at !RATE! Hz / %BLOCK% on !SOAKBE! - live below, logged as UTF-8.
    call :say status reports arrive every 30 seconds. Ctrl+C aborts; the log keeps what ran.
    powershell -NoProfile -Command "$ErrorActionPreference='SilentlyContinue'; & 'target\release\sparq.exe' soak --minutes !SOAKMIN! --report-every 30 --backend !SOAKBE! --rate !RATE! --block %BLOCK% 2>&1 | ForEach-Object { $s = [string]$_; Write-Host $s; Add-Content -LiteralPath '%LOG%' -Value $s -Encoding UTF8 }; exit $LASTEXITCODE"
    set "RC13=!ERRORLEVEL!"
)

REM ---- [14] summary --------------------------------------------------------------------------
:summary
call :say " "
call :say ================================================================
call :say  SUMMARY - test%TVER% - %DATE% %TIME%
call :say ================================================================
call :say      [02] ground truth captured - read the tables, they are the point
if defined RC04 call :verdict "[04] build + stamp guard" "!RC04!"
if defined RC05 call :verdict "[05] devices" "!RC05!"
if defined RC06 call :verdict "[06] devices --caps" "!RC06!"
if defined RC07 call :verdict "[07] conformance" "!RC07!"
if defined RC09 call :verdict "[09] play exclusive" "!RC09!"
if defined RC10 call :verdict "[10] play shared" "!RC10!"
if defined RC11 call :say      [11] unplug rc=!RC11! on !TESTBE! - judged from the TEXT above
if defined RC12 call :verdict "[12] recovery play" "!RC12!"
if defined RC13 call :verdict "[13] soak on !SOAKBE!" "!RC13!"
call :say tone heard - [09] !HEARD09!  [10] !HEARD10!  [12] !HEARD12!
call :say prep confirmed by operator: !PREP_OK!
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
REM the operator's guide and the log is the complete transcript of the
REM session - instructions, answers, command output and verdicts, in order.

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
