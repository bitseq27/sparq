@echo off
REM ============================================================================
REM  sparq - test008: WO-020 INC6 S4, the live-plane device acceptance run.
REM  Discovery -> registration -> adoption (PLAY #58 discharged) -> the driver
REM  thread -> the paced guest draw at the D19 token (15 Hz). Every runtime
REM  step below is a DEVICE measurement (the sandbox refuses wasmtime
REM  categorically - instrument-host.md sec 8; the host-side seams are proven
REM  there by ui --audit smoke 59, recorded not invented).
REM
REM  Order: stamp -> build -> launch words -> PLAY -> the living wall ->
REM         the watchdog injection -> the STREAMS tab + the NASA key -> send-back.
REM  Send back: logs\test008.log + the digest + screenshots of steps E/F/H.
REM  DO NOT CANCEL mid-run: session 8's gates.bat row died to Ctrl+C (every
REM  [FAIL] was exit 0xc000013a, zero real failures) and cost a whole round.
REM ============================================================================
set "SPARQ_ARGS=%*"
if defined CMD_CLICK_STARTED set "SPARQ_DOUBLE_CLICKED=1"
chcp 65001 >nul 2>&1
if not exist "%~dp0..\logs" mkdir "%~dp0..\logs" >nul 2>&1
set "SPARQ_LOG=%~dp0..\logs\test008.log"
call :main %SPARQ_ARGS% > "%SPARQ_LOG%" 2>&1
set "SPARQ_RC=%errorlevel%"
type "%SPARQ_LOG%"
echo.
echo  log saved to %SPARQ_LOG%
set "GD_PY="
where python >nul 2>&1 && set "GD_PY=python"
if not defined GD_PY where py >nul 2>&1 && set "GD_PY=py"
if defined GD_PY "%GD_PY%" "%~dp0..\tools\log_digest.py" "%SPARQ_LOG%"
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
python tools\sync_check.py
if errorlevel 1 (
    echo.
    echo  TEST008 REFUSED: the tree does not match SYNC-STAMP.txt. Re-stamp first:
    echo      python tools\sync_check.py --write --sync ^<this delivery's stamp^>
    echo  ^(or, for a deliberate local edit: scripts\build.bat --skip-sync-check^)
    exit /b 1
)

echo.
echo === [A] the device build ^(instrument-host + streams + streams-net + UI^) =====
echo   FIRST BUILD IS LONG ^(wasmtime + the UI stack, release^): 10-40 min is normal.
call scripts\build.bat
if errorlevel 1 set FAILED=!FAILED! A-build

echo.
echo === [B] the launch registration words ^(D17a^) =================================
echo   target\release\sparq.exe ui runs the window host; the LOG tab must carry:
echo     instruments: dat/observatory loaded - display instance parked ...
echo     instruments: dat/observatory REGISTERED - ... PLAY's #58 refusal lifts ...
echo     streams: the driver thread is polling on cadence ...
echo   A package that refuses to load is WORDS + an absent browser row - never a
echo   half-instrument. SCREENSHOT the log tab.
echo   ^>^> open the shell now: target\release\sparq.exe ui
echo   ^>^> spawn the Observatory from the library, press PLAY.
echo   [B1] PLAY runs with the Observatory on the canvas - the #58 refusal is GONE.
echo   [B2] the wall's cells UPDATE as the broker fetches ^(watch a 60 s feed land^).
echo   [B3] the ticker SCROLLS; the guest's PAUSE toggle FREEZES it ^(the frame
echo        context's clock is the shell's - D8: a display is never replayed^).
echo   [B4] a cell swap through the band's STREAM dropdown re-renders WITHIN A
echo        FRAME ^(the edit forces the draw - D19; the D7 gate re-hangs the 16
echo        pickers the same frame - S2's smoke, now live^).
if errorlevel 1 set FAILED=!FAILED! B-launch

echo.
echo === [C] the paced draw ^(D19: the token's 15 Hz^) ==============================
echo   The wall draws at ~15 Hz, not 60: the UI stays smooth while the ticker
echo   scrolls at the paced cadence. Any param edit / cell swap / resize / LOD
echo   change draws on the NEXT frame regardless of the pace. Resize the card
echo   ^(S1's corner handle^): the guest re-renders at the band's ACTUAL px - the
echo   text stays native at every size ^(O-1^), and the at-rest render scales.

echo.
echo === [D] the fuel watchdog's injection proof ==================================
echo   A gate nobody has seen fail is a gate nobody trusts - on a COPY, never the
echo   shipped package:
echo     1. copy instruments\observatory to %%TEMP%%\obs-tight
echo     2. in the copy's sparqmod.toml set capabilities.max_fuel = 100000
echo        ^(a hundredth of the declared 32 000 000 - a draw MUST overrun^)
echo     3. point the shell's launch root at the copy ^(or swap the dirs^) and relaunch
echo   EXPECT: five consecutive skipped frames, then the panel band's words:
echo     DISPLAY BYPASSED - five consecutive draws overran; the last render
echo     stands ^(the audio instance is untouched^)
echo   ...and PLAY KEEPS RUNNING ^(decision D's isolation: two instances, two
echo   watchdogs, never one mechanism^). SCREENSHOT the band words.
echo   Restore the shipped package afterwards ^(the copy is deleted, the original
echo   untouched - the MUST-NOT-MOVE surfaces stay must-not-move^).

echo.
echo === [E] the STREAMS tab LIVE + the NASA key ^(S3's device half, O-2/O-4^) ======
echo   [E1] the tab's header reads LIVE - the driver thread polls on cadence ^(the
echo        rows are the broker's own mirror^), NOT "AT REST - fixtures".
echo   [E2] with no key: space.neo/epic/power read UNSET ^(DEMO_KEY^) and the live
echo        feeds answer 429/403 - the words per row, never a hole.
echo   [E3] type the NASA key into any ONE of the three rows' KEY field: the echo
echo        is bullets, the commit logs the STATE words + the mask ^(never the
echo        value^), and ALL THREE rows flip to SET ^(file^) - one env var, one
echo        effective key ^(the sibling rule^).
echo   [E4] within one cadence the three feeds fetch at the KEY's rate: the 429s
echo        are gone, the rows go LIVE, the wall's NASA cells populate.
echo   [E5] a 10 s override on a row is honoured ^(the row reads "10 s ^(registry
echo        NNNN s^)"^); a 5 s entry REFUSES in words and keeps the old value ^(O-3^).
echo   [E6] the key survives the relaunch ^(the store file beside the cache^); with
echo        SPARQ_NASA_API_KEY set in the environment the field is DISABLED with
echo        the words SET ^(env^) - the precedence is visible ^(O-4^).
echo   [E7] the key NEVER appears in logs\test008.log, the shell log, any patch or
echo        any at-rest artefact: findstr /C:"<your key's first 8 chars>" over
echo        them all comes back EMPTY ^(the redaction acceptance, on the device^).
if errorlevel 1 set FAILED=!FAILED! E-streams

echo.
echo === [F] the frame histogram + the digest =====================================
echo   Close the shell. The log carries the launch words, the driver's poll lines
echo   and every refusal; the digest keeps the shape. Send back:
echo     logs\test008.log + test008-digest.log + screenshots of B/D/E.
echo.
if not "!FAILED!"=="" (
    echo  ============================================
    echo   TEST008 FAILED ROWS:!FAILED!
    echo  ============================================
    echo  Copy the failing sections above into a message back to me.
    exit /b 1
)
echo  ============================================
echo   test008: every row recorded - send the log + the screenshots back
echo  ============================================
exit /b 0
