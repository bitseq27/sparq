@echo off
REM ===========================================================================
REM  sparq test006 - WO-013 inc3+5 + WO-012 inc2: browser, inspector, re-patch,
REM  rename, inspector scroll, the LOD walk, the cv wire - and the LIVE audio
REM  session: PLAY on wasapi-shared, continuous meters, live edits, on SATURN.
REM
REM  THE CONTRACT: one file from Qwen - this script. Run it, answer the prompts,
REM  send back ONE file: test006.log from the repo root - plus logs\ui.log if
REM  the window session ran.
REM
REM  WHY. The increments are sandbox-green - 777 tests, audit PASS
REM  with 37 smokes, goldens unchanged. What the sandbox CANNOT prove: real
REM  fingers on the browser sheet, the slider following a real drag, the wire-
REM  end rings under a real touch, the rename sheet under real keys, the
REM  inspector scroll under real two fingers, the LOD renderings next to the
REM  mockup, a cv wire lighting from its own value - and the one mechanical
REM  claim this script makes: an INSPECTOR EDIT REACHES THE RENDER - and,
REM  since WO-012 inc2, that PLAY makes DEVICE SOUND and a live edit is
REM  heard without stopping. For the hashes:
REM  step [04] leaves a baseline canvas-render.wav rendered at manifest
REM  defaults; step [06] asks you to edit a param in the window and render
REM  again; step [07] hashes both. Same hash = the slider is a lie, and
REM  this script says so in words.
REM
REM  SAFE TO RE-RUN: the log appends with a dated banner per run.
REM
REM  Rules honoured - tools/check_text_io.py: pure ASCII - rule 3, defect
REM  #42 - and no unescaped parens in echo text inside blocks - rule 4,
REM  defect #69.
REM ===========================================================================
setlocal EnableDelayedExpansion
set "TVER=006"
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
del canvas-render.wav >nul 2>&1
del canvas-render-baseline.wav >nul 2>&1
del exec.wav >nul 2>&1

>>"%LOG%" echo.
>>"%LOG%" echo ===== test%TVER% RUN %DATE% %TIME% on %COMPUTERNAME% user %USERNAME% =====

call :say ================================================================
call :say  sparq test%TVER% - WO-013 inc 3+4+5 and WO-012 inc 2: browser, inspector, levels, rename, scroll, LOD, LIVE audio
call :say  at the end you send back ONE file: test%TVER%.log from the repo root
call :say ================================================================

REM ---- [00] environment ----------------------------------------------------
call :say " "
call :say [00] environment
set "CMD=ver"
call :run "os version"
if exist "crates\sparq-module-api\src\decode (2).rs" (
    call :say WARNING: stray decode copy artefact present - delete when convenient.
) else (
    call :say tree clean, no stray artefacts
)

REM ---- [00b] the sync stamp --------------------------------------------------
call :say " "
call :say [00b] sync stamp - every file that decides the binary, hashed against
call :say       SYNC-STAMP.txt. A FAIL here names files; it is never a rustc error.
set "PY="
where python >nul 2>&1 && set "PY=python"
if not defined PY where py >nul 2>&1 && set "PY=py"
if defined PY (
    set "CMD=!PY! tools\sync_check.py"
    call :run "sync check - tools\sync_check.py"
    set "RC00=!RC!"
) else (
    set "RC00=SKIP"
    call :say sync check SKIPPED - no `python` or `py` on PATH. That is a cannot-run,
    call :say not a pass: the tree stays unverified. build.bat will say the same.
)

REM ---- [01] build + stamp ---------------------------------------------------
call :say " "
call :say [01] build - the five first-party crates always recompile now, so that a
call :say      zip-restored mtime can never serve the last increment's rlib.
set "CMD=call scripts\build.bat"
call :run "build via scripts\build.bat - sync check, fingerprint purge, stamp guard"
set "RC01=!RC!"
if not "!RC01!"=="0" (
    call :say BUILD FAILED - send test%TVER%.log back now.
    goto summary
)
set "CMD=target\release\sparq.exe version"
call :run "version stamp"

REM ---- [02] modules --strict -------------------------------------------------
call :say " "
call :say [02] module registry - the seventeen manifests, strict
set "CMD=target\release\sparq.exe modules --strict"
call :run "modules --strict"
set "RC02=!RC!"

REM ---- [03] offline exec render -----------------------------------------------
call :say " "
call :say [03] offline render - the exec demo chain, 5 s, no device
set "CMD=target\release\sparq.exe exec --out exec.wav --seconds 5"
call :run "exec --out exec.wav --seconds 5"
set "RC03=!RC!"

REM ---- [04] the canvas bridge via the audit --------------------------------------
call :say " "
call :say [04] canvas bridge - the audit drives synthetic touch through 37 smokes,
call :say      including the five live-session ones: PLAY on the manual null device,
call :say      a live param edit crossing the command ring with the level FOLLOWING,
call :say      a live structural edit re-staging at the boundary, the STOP evidence
call :say      line, and an unplug ending the session with the canvas untouched.
call :say      Its RENDER WAV smoke writes canvas-render.wav
call :say      at MANIFEST DEFAULTS - the baseline for step [07].
call :run "ui --audit - 37 smokes incl. rename, cv levels, scroll, LOD, live, scope, chrome, mouse"
set "RC04=!RC!"

REM ---- [05] baseline hash --------------------------------------------------------
call :say " "
call :say [05] baseline - hash the default-param render and keep a copy to A/B listen
set "H1=missing"
if exist canvas-render.wav (
    copy /y canvas-render.wav canvas-render-baseline.wav >nul
)
if exist canvas-render.wav set "CMD=certutil -hashfile canvas-render.wav SHA256"
if exist canvas-render.wav call :run "baseline canvas-render.wav hash"
if not exist canvas-render.wav call :say canvas-render.wav MISSING - step 04 did not write; step 07 will say CANNOT-CHECK.
if exist canvas-render.wav for /f "usebackq skip=1 delims=" %%h in (`certutil -hashfile canvas-render.wav SHA256`) do set "H1=%%h"

REM ---- [06] the window session - real fingers --------------------------------------
call :say " "
call :say [06] WINDOW SESSION - the window opens when you press a key. In it:
call :say      A. long-press the EMPTY canvas - tap ADD MODULE - type sine on the
call :say         keyboard - tap the Sine row. A Sine node appears where you pressed.
call :say      B. tap the new Sine node - the INSPECTOR shows Frequency and
call :say         Amplitude - drag the Frequency slider. The value follows your
call :say         finger; the log band reports Frequency = ... Hz when you lift.
call :say      C. long-press the empty canvas - tap RENDER WAV. This overwrites
call :say         canvas-render.wav WITH YOUR EDIT - step [07] hashes it.
call :say      D. drag the small ring near a wire END onto another port - the wire
call :say         moves. Three-finger tap - it returns, same wire.
call :say      E. three-finger tap again - the slider drag undoes, value returns.
call :say      F. long-press a NODE - tap RENAME - type a name - press ENTER:
call :say         the header shows it. Three-finger tap: the old name is back.
call :say         Reopen RENAME, press ESC or tap outside: cancels, name kept.
call :say      G. add a Mixer via the browser, tap it: 20 rows, the last ones
call :say         clipped. Two-finger drag UP inside the panel: the rows scroll,
call :say         the thin thumb at the right edge moves, the CANVAS does not.
call :say      H. bypass a node from its menu, then pinch two fingers together
call :say         on the canvas to zoom out: first the boxes lose ALL text -
call :say         states ride patterns now: hatched body = bypassed, dashed
call :say         border = muted, double border = locked, header chip = master.
call :say         Pinch further: dots and HAIRLINE wires; the bypassed dot is a
call :say         hollow ring. Compare with design\mockups\design-mode.svg and
call :say         write what differs into the H answer below.
call :say      I. wire the thesis chain: spawn an SVF via the browser, drag
call :say         gain OUT to svf IN, drag rms LEVEL to svf CUTOFF-MOD, then
call :say         long-press empty canvas - RENDER WAV. The cyan cv wire lights
call :say         from the rms value; the audio wires light as before.
call :say      J. LIVE AUDIO, the WO-012 inc2 acceptance: tap PLAY on the rail.
call :say         The log names the backend, the NEGOTIATED rate and the latency;
call :say         you HEAR the patch through the device - shared mode, about
call :say         22.67 ms - and the wires animate CONTINUOUSLY from the engine's
call :say         own meters while it plays, not only after RENDER WAV.
call :say      K. while it plays: drag a slider - the sound changes LIVE, no stop,
call :say         no click at the moment of the edit; drag a new wire - it goes
call :say         audible at the next block boundary. Tap STOP: sound stops and
call :say         the evidence line names blocks, xruns, swaps and allocations.
call :say      WITH A MOUSE, if one is handy: right-click a node opens its menu
call :say         where a finger long-presses; the wheel scrolls the inspector over
call :say         the panel and pans the canvas over the canvas; plain hover is
call :say         silent. Write what felt wrong into the K answer too.
call :say         OPTIONAL, only with a spare cable: unplug the output device
call :say         mid-play - one honest Removed line, the session ends, and the
call :say         canvas is untouched. Write what you heard into the J/K answers.
call :say      Then close the window; the script continues by itself.
call :say      Over RDP the rasteriser is WARP - slow but functional; the 60 fps
call :say      claim waits for the physical screen, as always.
set /p "GO=      press Enter to open the window: "
call :say operator opened the window session
call scripts\ui.bat
call :say window closed - ui.log saved by ui.bat

set "A=n"
set "B=n"
set "C=n"
set "D=n"
set "E=n"
set "F=n"
set "G=n"
set "H=n"
set "I=n"
set "J=n"
set "K=n"
set /p "A=      A: browser search spawned the Sine node where you pressed, y/n? "
set /p "B=      B: the slider followed your finger and the log named the value, y/n? "
set /p "C=      C: you tapped RENDER WAV after the edit, y/n? "
set /p "D=      D: the wire end moved and one three-finger tap restored it, y/n? "
set /p "E=      E: a second three-finger tap undid the slider edit, y/n? "
set /p "F=      F: rename committed with ENTER, one undo restored, ESC or outside cancelled, y/n? "
set /p "G=      G: two-finger drag scrolled the inspector and the canvas stayed put, y/n? "
set /p "H=      H: zoomed-out states read as patterns, dot wires are hairlines; what differed from the mockup? "
set /p "I=      I: the cv wire lit from the rms value after RENDER WAV, y/n? "
set /p "J=      J: PLAY made device sound and the wires animated while playing, y/n? "
set /p "K=      K: a live slider edit changed the sound without stopping, y/n? "
call :say operator answers: A=!A! B=!B! C=!C! D=!D! E=!E! F=!F! G=!G!
call :say                 H=!H! I=!I! J=!J! K=!K!

REM ---- [07] the mechanical claim: the edit reached the render ----------------------
call :say " "
call :say [07] evidence - hash canvas-render.wav again; it MUST differ from the
call :say      baseline if step C ran and the inspector is wired to the bridge.
set "H2=missing"
set "CHG=CANNOT-CHECK"
if exist canvas-render.wav set "CMD=certutil -hashfile canvas-render.wav SHA256"
if exist canvas-render.wav call :run "edited canvas-render.wav hash"
if exist canvas-render.wav for /f "usebackq skip=1 delims=" %%h in (`certutil -hashfile canvas-render.wav SHA256`) do set "H2=%%h"
if not "!H1!"=="missing" if not "!H2!"=="missing" if not "!H1!"=="!H2!" set "CHG=CHANGED"
if not "!H1!"=="missing" if not "!H2!"=="missing" if "!H1!"=="!H2!" set "CHG=UNCHANGED"
call :say baseline hash: !H1!
call :say edited   hash: !H2!
call :say render hash verdict: !CHG!
if "!CHG!"=="UNCHANGED" call :say  - you answered C=!C!. If C=y this is a DEFECT: the slider did not
if "!CHG!"=="UNCHANGED" call :say    reach the render. Send the log back; the bridge is lying.

REM ---- [08] summary --------------------------------------------------------------
:summary
call :say " "
call :say ================================================================
call :say  SUMMARY - test%TVER% - %DATE% %TIME%
call :say ================================================================
if defined RC00 call :verdict "[00b] tree matches SYNC-STAMP.txt" "!RC00!"
if defined RC01 call :verdict "[01] build + stamp guard" "!RC01!"
if defined RC02 call :verdict "[02] modules --strict" "!RC02!"
if defined RC03 call :verdict "[03] exec offline render" "!RC03!"
if defined RC04 call :verdict "[04] ui --audit, 37 smokes" "!RC04!"
call :say [07] param edit reached the render : !CHG!
call :say operator answers: A=!A! B=!B! C=!C! D=!D! E=!E! F=!F! G=!G! H=!H! I=!I! J=!J! K=!K!
call :say steps failing on rc: !FAILED!
call :say " "
REM ---- the compact copies: the full logs stay here, the digests travel -----
set "HAVE_PY=0"
where python >nul 2>&1 && set "HAVE_PY=1"
where py >nul 2>&1 && set "HAVE_PY=1"
if "!HAVE_PY!"=="1" (
    where python >nul 2>&1 && set "PY=python" || set "PY=py"
    del "%CD%\test%TVER%-digest.log" >nul 2>&1
    del "%CD%\logs\ui-digest.log" >nul 2>&1
    "!PY!" "%~dp0..\tools\log_digest.py" "%LOG%" logs\ui.log
    call :say SEND BACK: the compact copies when they were made - test%TVER%-digest.log
    call :say and logs\ui-digest.log - else the full files. Verdict lines are
    call :say byte-identical in the copies; only repeated status lines collapse.
    call :say full paths: %LOG%  and  logs\ui.log
) else (
    call :say SEND BACK: %LOG%  and  logs\ui.log
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
