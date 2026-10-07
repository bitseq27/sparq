@echo off
REM ============================================================================
REM  sparq - test007: WO-020 INC5, the device acceptance run (plan sec 15.2).
REM  The stream plane live + the instrument runtime, measured on the >= 2 GB host
REM  (the sandbox refuses wasmtime compilation categorically - instrument-host.md
REM  sec 8; every step below is a DEVICE measurement, recorded not invented).
REM
REM  Order: build -> live probe -> the FULL hand-in gate -> the shell eyes ->
REM         the network story -> the frame histogram -> re-theme -> gates -> stamp.
REM  Send back: logs\test007.log + test007-digest.log + logs\ui-digest.log +
REM  screenshots of steps E/G/I.
REM ============================================================================
set "SPARQ_ARGS=%*"
if defined CMD_CLICK_STARTED set "SPARQ_DOUBLE_CLICKED=1"
chcp 65001 >nul 2>&1
if not exist "%~dp0..\logs" mkdir "%~dp0..\logs" >nul 2>&1
set "SPARQ_LOG=%~dp0..\logs\test007.log"
call :main %SPARQ_ARGS% > "%SPARQ_LOG%" 2>&1
set "SPARQ_RC=%errorlevel%"
type "%SPARQ_LOG%"
echo.
echo  log saved to %SPARQ_LOG%
set "GD_PY="
where python >nul 2>&1 && set "GD_PY=python"
if not defined GD_PY where py >nul 2>&1 && set "GD_PY=py"
if defined GD_PY "%GD_PY" "%~dp0..\tools\log_digest.py" "%SPARQ_LOG%"
if defined SPARQ_DOUBLE_CLICKED pause
exit /b %SPARQ_RC%

:main
setlocal EnableDelayedExpansion
cd /d "%~dp0.."
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
set FAILED=

where cargo >nul 2>&1
if errorlevel 1 (
    echo  cargo not found on PATH. Run scripts\setup.bat first.
    exit /b 1
)

echo === [0] the stamp gate - FAIL FAST ^(the 2026-10-07 device lesson^) ==========
echo   build.bat REFUSES on a stamp mismatch ^(defect #78^) - and every later step
echo   would then run against the STALE exe ^(unknown command `streams`/`mod`^).
python tools\sync_check.py
if errorlevel 1 (
    echo.
    echo  TEST007 REFUSED: the tree does not match SYNC-STAMP.txt. Re-stamp first:
    echo      python tools\sync_check.py --write --sync sparq-wo020-inc5r8-2026-10-07
    echo  ^(or, for a deliberate local edit: scripts\build.bat --skip-sync-check^)
    exit /b 1
)

echo.
echo === [A] the device build ^(instrument-host + streams + streams-net^) =========
echo   the stamp gate above passed, so this build really runs. FIRST BUILD IS
echo   LONG ^(wasmtime + the UI stack, release^): 10-40 min is normal.
call scripts\build.bat
if errorlevel 1 set FAILED=!FAILED! A-build

echo.
echo === [B] the live probe - all 23 endpoints, statuses recorded ===============
echo   DONKI stays dead ^(registry comment^); sec 3.3's sandbox-DNS asterisks clear
echo   here or they are recorded as device facts. KEY NEEDED rows are the D14
echo   refusal working ^(FIRMS without SPARQ_FIRMS_KEY^).
target\release\sparq.exe streams probe --live
if errorlevel 1 set FAILED=!FAILED! B-probe
target\release\sparq.exe streams probe
if errorlevel 1 set FAILED=!FAILED! B-probe-static

echo.
echo === [C] the sealed component is in the pack =================================
echo   instruments\observatory\observatory.wasm ^(206 316 B, sha 4b29a5ae... - the
echo   session-7 rebuild carrying INC4's cell_ground fix, pinned in WO020-STATE.md's
echo   session-7 record^). A device rebuild follows the STATE resume recipe sec 5 -
echo   the guest workspace lockfile door is LATER.md's INC5 row.
if not exist instruments\observatory\observatory.wasm (
    echo  MISSING: instruments\observatory\observatory.wasm
    set FAILED=!FAILED! C-component
)

echo.
echo === [D] the FULL hand-in gate - smoke, golden x2, budgets, visual ===========
echo   Expect: GATE: COMPLETE + PASS, preview.svg regenerated ^(file 5 of 5^),
echo   the golden hash bdf59cdb... recorded in stage 4's words, the measured
echo   fuel/memory inside the declared class ^(stage 5^). This is also step J.
target\release\sparq.exe mod validate instruments\observatory\
if errorlevel 1 set FAILED=!FAILED! D-validate

echo.
echo === [E]+[F] the shell eyes ^(OPERATOR - screenshots please^) ==================
echo   E: launch the UI - the Observatory offers in the browser's instrument
echo      group; the wall shows at rest from the stage-6 publish, LIVE cells
echo      where the broker has filled windows.
echo   F: dropdown swaps a cell's stream live; LAYOUT 4-16; SOLO/FULL + canvas
echo      FOCUS; tickers scroll, PAUSE freezes; stamps are UTC-with-units.
echo   NOTE ^(INC5b^): PLAY with the instrument on canvas still refuses in words
echo   ^(#58^) until the launch-registration slice wires the loader into the
echo   registry - the shell offers, the executor declines, nobody lies.
target\release\sparq.exe ui
if errorlevel 1 set FAILED=!FAILED! E-shell

echo.
echo === [G] the network story ^(OPERATOR - airplane mode^) ========================
echo   With the wall live: airplane mode -> STALE at 3x cadence -> OFFLINE at
echo   10x, IN WORDS; audio unaffected; restore -> LIVE. Screenshot the words.

echo.
echo === [H] the frame-time histogram ============================================
echo   Wall + 50 backbone nodes, >= 60 fps ^(the plan's acceptance line^):
target\release\sparq.exe ui --headless 120
if errorlevel 1 set FAILED=!FAILED! H-frames

echo.
echo === [I] the token re-theme ^(WO-019's criterion, instrument #1^) ==============
echo   Change color.signal.audio in design\tokens\colors.toml, re-run
echo   python tools\token_gen.py, rebuild: the wall's phosphor traces re-theme
echo   with ZERO package bytes changed. Screenshot before/after. Then revert.

echo.
echo === [K] the full gate table ==================================================
call scripts\gates.bat
if errorlevel 1 set FAILED=!FAILED! K-gates

echo.
echo === [L] re-stamp ==============================================================
python tools\sync_check.py --write --sync sparq-wo020-inc5r3-%date:~-4%%date:~3,2%%date:~0,2%
if errorlevel 1 set FAILED=!FAILED! L-stamp

echo.
if defined FAILED (
    echo TEST007: FAILED steps:!FAILED!
    echo   every failure above named itself in words; send the log regardless.
    exit /b 1
)
echo TEST007: ALL AUTOMATED STEPS GREEN.
echo   Operator items: E/F/G/I screenshots + the four behaviours filmed or
echo   screenshotted for the checklist ^(plan sec 9 INC5 acceptance^).
exit /b 0
