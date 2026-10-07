@echo off
REM Capture the arguments BEFORE any `call`. Inside a called subroutine `%*` refers to the
REM subroutine's own arguments, not the script's - so forwarding them from in here silently drops
REM every argument the user typed. This was a real bug: `demo.bat --pattern breakcore` rendered the
REM default techno patch and printed no error, because the exe never saw the flag.
set "SPARQ_ARGS=%*"
REM Keep the window open when this script was double-clicked, and always leave a log behind so
REM the result survives the window closing. CMD_CLICK_STARTED is set only by Explorer, so the
REM pause never fires when the script is run from an existing terminal.
if defined CMD_CLICK_STARTED set "SPARQ_DOUBLE_CLICKED=1"
REM UTF-8, so the middot and em-dash the binary prints do not arrive as mojibake on a cp850 console.
REM NOTE: chcp 65001 is for the BINARY's output only. This file itself is pure ASCII by rule -
REM a multibyte character in a .bat makes cmd's parser lose sync and die silently (defect #42:
REM this script shipped a UTF-8 middot in the stamp guard and exited without a log). The rule is
REM enforced by tools/check_text_io.py rule 3. Rule 4 of the same gate covers the other parser
REM killer: inside a parenthesised block the first unescaped `)` ENDS the block, even when a `(`
REM on the same echo line looks like it balances - defect #69, which killed this script after
REM every successful build for a whole increment. Escape block-internal echo parens: ^( ^).
chcp 65001 >nul 2>&1
if not exist "%~dp0..\logs" mkdir "%~dp0..\logs" >nul 2>&1
set "SPARQ_LOG=%~dp0..\logs\build.log"
call :sparq_main %SPARQ_ARGS% > "%SPARQ_LOG%" 2>&1
set "SPARQ_RC=%errorlevel%"
type "%SPARQ_LOG%"
echo.
echo  log saved to %SPARQ_LOG%
if defined SPARQ_DOUBLE_CLICKED pause
exit /b %SPARQ_RC%

:sparq_main
REM ============================================================================
REM  sparq - build sparq.exe (release, HAL WASAPI backends + bootstrap + UI shell)
REM
REM  Always builds (never "only if the exe is missing" - a stale exe is worse
REM  than a slow build), then VERIFYs the result with the stamp guard: the
REM  fingerprint the binary carries must equal the fingerprint of THIS tree.
REM
REM  Why the guard exists: zip-synced trees defeat cargo's freshness check
REM  (defect #41). Extraction restores archive timestamps, so freshly-synced
REM  sources can look older than the cached build and cargo serves the previous
REM  binary. Two remedies were tried and both failed on a real machine:
REM    - `cargo clean -p` reported "Removed 0 files" next to a full feature
REM      build, because clean consults the same fingerprints that are wrong.
REM    - touching sources (`type nul >>`) did not help either: touching sets
REM      mtime to NOW, and the fingerprint files already recorded a comparable
REM      NOW from the previous build minutes earlier, so cargo still saw fresh.
REM  What cannot be argued with is deleting the outputs, which is what the
REM  forced path below does (defect #49).
REM
REM  Usage:  scripts\build.bat
REM ============================================================================
setlocal
cd /d "%~dp0.."
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

REM --skip-sync-check: the escape hatch for a local edit you mean to keep. Parsed here, before any
REM `call`, because %* inside a subroutine is the subroutine's own argument list - the same trap
REM the wrapper at the top of this file already documents.
set "SKIP_SYNC="
if /i "%~1"=="--skip-sync-check" set "SKIP_SYNC=1"

where cargo >nul 2>&1
if errorlevel 1 (
    echo  cargo not found on PATH. Run scripts\setup.bat first.
    exit /b 1
)

REM ---- the sync stamp: is this tree the tree that was verified? ^(defect #78^) -------------
REM Content, not timestamps. test005 on SATURN failed with six rustc errors of one shape -
REM `no method named resolve_master`, `no field master`, and a note listing inc1's five
REM CanvasState fields - while every one of those items sat in the interact.rs the sync shipped,
REM and the six line numbers matched the shipped sparq-app files exactly. The app crate was new;
REM the sparq-ui it compiled against was not. tools\sync_check.py hashes this tree against
REM SYNC-STAMP.txt, so the answer is a list of file names in words, printed BEFORE cargo runs.
set "PY="
where python >nul 2>&1 && set "PY=python"
if not defined PY where py >nul 2>&1 && set "PY=py"
if defined SKIP_SYNC (
    echo  sync check skipped: --skip-sync-check was given. The tree is NOT verified.
) else if not defined PY (
    echo  sync check SKIPPED - no `python` or `py` on PATH. That is a cannot-run, not a
    echo  pass: a half-applied sync would arrive as rustc errors in the wrong crate. Install
    echo  Python 3.11+, or run scripts\synccheck.bat on a machine that has it.
) else (
    %PY% tools\sync_check.py --quiet
    if errorlevel 1 (
        echo.
        echo  ============================================
        echo   TREE DOES NOT MATCH THE SYNC
        echo  ============================================
        echo   Not building. cargo would compile a tree that is not the one that was
        echo   verified, and the errors would name a crate that is not the one that is
        echo   wrong ^(defect #78^). The files listed above are the answer to the question
        echo   "did the sync land". Re-extract the sync zip at the repo root, overwriting,
        echo   and run this script again. Full report: scripts\synccheck.bat
        echo   To build anyway, if you edited a source file here on purpose:
        echo       scripts\build.bat --skip-sync-check
        exit /b 1
    )
)

REM ---- defeat cargo's freshness cache BEFORE the build, not after it ^(defect #78^) ---------
REM Zip extraction restores the ARCHIVE's timestamps, so a freshly-synced source file can be
REM OLDER than the cached build. cargo then reports "Fresh", serves the previous increment's
REM rlib, and every crate downstream fails to compile with "no method named ..." for methods
REM that are sitting in the file on disk. This is defect #41's mechanism in a new place: #41 and
REM #49 were about a stale BINARY passing a run, so the guard below checks the exe after a
REM successful build - a stale dependency rlib never gets that far, it fails the compile instead.
REM Detector and remedy were both on the wrong side of the failure.
REM
REM Measured in the sandbox, release profile, 2 cores:
REM   build sparq-ui from inc1's interact.rs ................ 1.28 s, rlib written
REM   inc2's interact.rs in place, mtime = the archive's, then
REM   cargo build --release -p sparq-ui -v .... "Fresh sparq-ui", 0.06 s
REM      and that rlib contains `resolve_master` 0 times
REM   delete the first-party fingerprints, rebuild .......... 4.20 s
REM      and that rlib contains `resolve_master` 6 times
REM
REM Only the five first-party crates are purged, and only when they might be wrong. gates.bat
REM deletes the whole .fingerprint tree every run, which is right for a gate table and wrong here:
REM that rebuilds wgpu on every build, and run.bat/demo.bat/devices.bat all come through this
REM script, so "unchanged tree costs seconds" is a promise worth keeping. The test is CONTENT, not
REM timestamps: sync_check.py digests the tree and compares it with the marker this script writes
REM after a good build. Same content -> trust cargo, build incrementally. Different content ->
REM purge, because that is exactly when cargo's mtime test cannot be trusted. No python on PATH ->
REM purge, since "cannot check" must never mean "assume fresh".
set "PURGE="
if not defined PY set "PURGE=1"
if defined PY (
    %PY% tools\sync_check.py --freshness target\release\.sparq-tree >nul 2>&1
    if errorlevel 1 set "PURGE=1"
)
if not defined PURGE (
    echo  note: tree content unchanged since the last good build - building incrementally.
) else (
    if exist target\release\.fingerprint (
        for /d %%D in (target\release\.fingerprint\sparq-*) do rd /s /q "%%D"
        echo  note: tree content changed since the last good build - the five first-party
        echo  fingerprints are cleared, so cargo cannot serve a crate from the last sync.
    )
)

echo  building sparq.exe ^(release, hal-wasapi + bootstrap-audio + ui-window + instrument-host + streams + streams-net^) ...
echo  first build takes a few minutes (wgpu is big); later builds are incremental.
echo.
cargo build --release -p sparq-app --features bootstrap-audio,hal-wasapi,ui-window,instrument-host,streams,streams-net
if errorlevel 1 (
    echo.
    echo  ============================================
    echo   BUILD FAILED
    echo  ============================================
    echo   If the error mentions `link.exe` or `linker not found`:
    echo     the MSVC C++ Build Tools are missing. Run scripts\setup.bat, or install
    echo     "Build Tools for Visual Studio" with the "Desktop development with C++"
    echo     workload, then open a NEW terminal and re-run this script.
    echo.
    echo   If the error mentions `d3d12`, `dxgi` or `wgpu`:
    echo     the UI shell needs the DX12 backend; make sure the sync included
    echo     crates\sparq-app\Cargo.toml ^(the per-target wgpu features live there^).
    echo.
    echo   If the error mentions `alsa` or `pkg-config`:
    echo     you are not on Windows. On Linux install libasound2-dev and pkg-config.
    echo.
    echo   If the error is `no method named` or `no field` for something that IS in the
    echo   file on disk: the cache and the tree disagreed. Run scripts\synccheck.bat, then
    echo   delete target\release and re-run this script ^(defect #78^).
    echo.
    echo   Anything else: copy the whole error into a message back to me.
    exit /b 1
)

if not exist "target\release\sparq.exe" (
    echo  BUILD reported success but target\release\sparq.exe is missing.
    exit /b 1
)

call :stamp_check
if errorlevel 1 (
    echo.
    echo  STALE BINARY: the exe does not match the sources in this tree.
    echo  Deleting cargo's record of what it built, then rebuilding.
    echo.
    echo  Why deletion and not a lighter remedy - all three were tried:
    echo    - `cargo clean -p` reported "Removed 0 files" beside a full feature
    echo      build, because clean consults the same fingerprints that are wrong.
    echo    - touching sources did not help: a touch sets mtime to NOW, and the
    echo      fingerprint files already recorded a comparable NOW from the build
    echo      minutes earlier, so cargo still concluded "fresh" ^(0.03 s, twice^).
    echo    - deleting only the exe forces a relink but cargo may reuse the CACHED
    echo      build-script output, which holds the stale stamp - so it would
    echo      re-embed the old fingerprint and this guard would loop.
    echo  Deleting .fingerprint removes cargo's evidence of what it built, which
    echo  forces the build script to run again and the stamp to be recomputed from
    echo  this tree. Dependencies stay cached, so this costs seconds, not minutes.
    echo.
    del /f /q target\release\sparq.exe target\release\sparq.pdb 2>nul
    if exist target\release\.fingerprint rd /s /q target\release\.fingerprint
    cargo build --release -p sparq-app --features bootstrap-audio,hal-wasapi,ui-window,instrument-host,streams,streams-net
    if errorlevel 1 (
        echo  REBUILD FAILED - copy this log into a message back.
        exit /b 1
    )
    call :stamp_check
    if errorlevel 1 (
        echo.
        echo  STILL STALE after deleting .fingerprint and rebuilding. Do NOT
        echo  trust this binary. Delete the whole target\ directory, re-run this
        echo  script, and send the log back: that would mean something outside
        echo  target\release\ is being reused, which is worth understanding.
        exit /b 1
    )
    echo  stamp guard: rebuild verified, binary now matches the tree.
)

REM The build is good and the guard agrees with it: record the content of the tree this binary
REM came from, so the next run can tell "nothing changed" from "cargo is about to be fooled".
if defined PY %PY% tools\sync_check.py --freshness-write target\release\.sparq-tree --quiet >nul 2>&1

echo.
target\release\sparq.exe version
echo.
echo  built: %CD%\target\release\sparq.exe
for %%A in (target\release\sparq.exe) do echo  size : %%~zA bytes
echo.
echo  next:  scripts\devices.bat   then   scripts\run.bat
exit /b 0

:stamp_check
REM Exit 0 = the binary's embedded fingerprint matches the tree, OR the guard
REM          itself cannot see the tree (loud note; never condemn on broken evidence).
REM Exit 1 = verified mismatch, or the binary's stamp cannot be read at all.
REM
REM The fingerprint definition MUST mirror crates/sparq-app/build.rs exactly:
REM every *.rs under the FIVE src roots, recursively, "COUNTf/BYTESB". If one
REM changes, change both. Defect #68: this walk shipped with four roots while
REM build.rs had five - from WO-007 on, the guard would have condemned every
REM fresh binary as stale (the cmd mirror of #60, which fixed only build.rs).
REM
REM Pure cmd, pure ASCII (defect #42). The walk uses `dir /s /b /a-d` through
REM for /f - the dullest construct in batch - because a FOR /R variant of this
REM loop counted zero files on a real machine while the same tree compiled 47
REM files seconds earlier (defect #47). A zero count is treated as a GUARD
REM failure, not as staleness: cargo just built from these sources, so they exist.
setlocal EnableDelayedExpansion
set CNT=0
set BYTES=0
for /f "delims=" %%F in ('dir /s /b /a-d "crates\sparq-kernel\src\*.rs" "crates\sparq-audio\src\*.rs" "crates\sparq-ui\src\*.rs" "crates\sparq-app\src\*.rs" "crates\sparq-module-api\src\*.rs" "crates\sparq-music\src\*.rs" 2^>nul') do (
    set /a CNT+=1
    set /a BYTES+=%%~zF
)
if "!CNT!"=="0" (
    echo  stamp guard: counted 0 source files from "%CD%" - the guard cannot see
    echo  the tree. This is a GUARD failure, not a stale binary: cargo just built
    echo  from these sources, so they exist. Trusting the build. Please send this
    echo  log back ^(defect #47 class - the walk is broken on this machine^).
    endlocal & exit /b 0
)
set "CUR=!CNT!f/!BYTES!B"
set "BIN="
set "LINE="
REM Anchor on " src " (space both sides): the version output also contains "newest source", and
REM an unanchored match would grab the wrong field. findstr does the anchoring now, and the
REM loop body is a single set: defect #69 - build.bat died on SATURN with the cmd parser
REM error ". was unexpected at this time." right after a SUCCESSFUL build, before any stamp
REM line could print, and the per-line IF with delayed-expansion substring substitution below
REM was the prime suspect. scripts\probe-stamp.bat still carries the old form, section by
REM section, to name the real culprit; until it reports, treat this as hardened, not proven.
for /f "usebackq tokens=* delims=" %%A in (`target\release\sparq.exe version 2^>nul ^| findstr /c:" src "`) do set "LINE=%%A"
if defined LINE set "BIN=!LINE:* src =!"
if "!BIN!"=="" (
    echo  stamp guard: cannot read the binary stamp - treating as stale
    endlocal & exit /b 1
)
echo  stamp guard: sources !CUR!  -  binary !BIN!
if "!CUR!"=="!BIN!" (
    endlocal & exit /b 0
)
REM Verified mismatch: name both sides so the log says which one is implausible.
echo  stamp guard: MISMATCH. If the source side looks right, the exe is from an
echo  older sync; if the source side is 0f/0B or otherwise implausible, the GUARD
echo  is wrong and the build is fine - say which in the message back.
endlocal & exit /b 1
