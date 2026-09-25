@echo off
REM ===========================================================================
REM  sparq test005 - WO-013 inc2 + WO-014 inc1: the no-device chain on SATURN.
REM
REM  THE CONTRACT: one file from Qwen - this script. Run it, answer the prompts,
REM  send back ONE file: test005.log from the repo root.
REM
REM  WHY. You are back on RDP and the studio acceptance - test004, exclusive
REM  mode - waits for the physical session. This script proves everything that
REM  does NOT need an audio device, on the stage machine: the module registry
REM  and manifests, the offline executor render, and the new canvas bridge -
REM  the audit drives the UI by synthetic touch, taps RENDER WAV, and writes
REM  canvas-render.wav. If RDP audio redirects you can even HEAR the two wavs;
REM  if it does not, the SHA-256 hashes in the log are the proof.
REM
REM  SAFE TO RE-RUN: the log appends with a dated banner per run.
REM
REM  Rules honoured - tools/check_text_io.py: pure ASCII - rule 3, defect
REM  #42 - and no unescaped parens in echo text inside blocks - rule 4,
REM  defect #69.
REM ===========================================================================
setlocal EnableDelayedExpansion
set "TVER=005"
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
del exec.wav >nul 2>&1

>>"%LOG%" echo.
>>"%LOG%" echo ===== test%TVER% RUN %DATE% %TIME% on %COMPUTERNAME% user %USERNAME% =====

call :say ================================================================
call :say  sparq test%TVER% - the no-device chain: modules, exec, canvas bridge
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
REM Content, not timestamps: does this tree match the zip byte for byte? This is the
REM question defect #78 could not answer from a build log - six rustc errors about items
REM that WERE in the file on disk, because either that file never landed or cargo answered
REM "Fresh sparq-ui" and served an rlib built from the previous increment. build.bat runs
REM the same check and refuses to build on a mismatch; running it here as well puts the
REM full per-file report into THIS log, which is the one file that comes back.
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
call :say      zip-restored mtime can never serve the last increment's rlib; the
call :say      dependencies stay cached, so this costs a minute at most.
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
call :say [02] module registry - the three manifests, strict
set "CMD=target\release\sparq.exe modules --strict"
call :run "modules --strict"
set "RC02=!RC!"

REM ---- [03] offline exec render -----------------------------------------------
call :say " "
call :say [03] offline render - the exec demo chain, 5 s, no device
set "CMD=target\release\sparq.exe exec --out exec.wav --seconds 5"
call :run "exec --out exec.wav --seconds 5"
set "RC03=!RC!"
if exist exec.wav (
    set "CMD=certutil -hashfile exec.wav SHA256"
    call :run "exec.wav hash"
) else (
    call :say exec.wav MISSING - step 03 did not write a file.
)

REM ---- [04] the canvas bridge via the audit --------------------------------------
call :say " "
call :say [04] canvas bridge - the audit drives synthetic touch: double-tap,
call :say      long-press the empty canvas, tap the RENDER WAV row. It writes
call :say      canvas-render.wav in the repo root; the log gets the evidence line.
set "CMD=target\release\sparq.exe ui --audit"
call :run "ui --audit - 16 smokes incl. the render chain"
set "RC04=!RC!"
if exist canvas-render.wav (
    set "CMD=certutil -hashfile canvas-render.wav SHA256"
    call :run "canvas-render.wav hash"
) else (
    call :say canvas-render.wav MISSING - the render smoke did not write.
)

REM ---- [05] listening, if RDP audio redirects ----------------------------------------
call :say " "
call :say [05] OPTIONAL listen: if this RDP session redirects audio, play the two
call :say      wavs in the repo root - exec.wav and canvas-render.wav. Both carry a
call :say      steady 440 Hz sine, 5 s each; canvas-render.wav is the drawn demo
call :say      patch - sine into gain into rms - rendered by the bridge.
set "HEARD=skip"
set /p "HEARD=      heard both, y / n / skip [Enter = skip]: "
call :say operator listen answer: !HEARD!

REM ---- [06] summary --------------------------------------------------------------
:summary
call :say " "
call :say ================================================================
call :say  SUMMARY - test%TVER% - %DATE% %TIME%
call :say ================================================================
if defined RC00 call :verdict "[00b] tree matches SYNC-STAMP.txt" "!RC00!"
if defined RC01 call :verdict "[01] build + stamp guard" "!RC01!"
if defined RC02 call :verdict "[02] modules --strict" "!RC02!"
if defined RC03 call :verdict "[03] exec offline render" "!RC03!"
if defined RC04 call :verdict "[04] ui --audit incl. RENDER WAV smoke" "!RC04!"
call :say steps failing on rc: !FAILED!
call :say " "
call :say Next: the studio session - test004 runs the EXCLUSIVE acceptance,
call :say real fingers touch-test the canvas, and the DPI matrix gets walked.
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
