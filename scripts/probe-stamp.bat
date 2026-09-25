@echo off
REM probe-stamp.bat - defect #69 root-cause hunt. CLOSED: the killer is named.
REM
REM On SATURN (P1, 2026-09-23) build.bat died with the cmd parser error
REM ". was unexpected at this time." immediately AFTER a successful cargo build, so
REM logs\build.log never reached a single "stamp guard:" line. The hunt ran each suspect
REM statement group in its own child cmd with echo ON; the first device run of this probe
REM named the culprit on section S0a: inside a parenthesised block, the first unescaped `)`
REM in echo text ENDS the block (a same-line `(` does not nest against it), and the leftover
REM `.` is a token the IF parser rejects. The block is parsed on every run whether or not the
REM condition fires - so build.bat died after every successful build from the moment WO-012
REM added that hint line. Fixed in build.bat by caret-escaping; check_text_io.py rule 4 now
REM gates the class. S0a keeps the unescaped original VERBATIM as the reproduction (it must
REM die - that is a successful reproduction, not a new failure); S0b verifies the fixed form.
REM
REM Expected results on a healthy tree:
REM   S0a exits NONZERO with ". was unexpected at this time."  <- the reproduction, by design
REM   S0b exits 0 and prints "escaped block parsed cleanly"
REM   S1  CNT=58 BYTES=860719        (the old four-root walk; quantifies defect #68)
REM   S2  BIN=68f/1070593B           (the as-shipped version loop: INNOCENT - it never was
REM                                    the killer; kept because the record says it was the
REM                                    prime suspect and the exoneration is part of the story)
REM   S3  prints mismatch-as-expected
REM   S4  CNT=68 BYTES=1070593       (the five-root fix, == the recorded baseline; a different
REM                                    count means the tree drifted, e.g. a stray "(2)" copy)
REM   S5  BIN=68f/1070593B           (the hardened findstr capture now shipped in build.bat)
REM
REM Usage:   scripts\probe-stamp.bat          (writes and prints logs\probe.log)
REM Safe:    read-only apart from logs\probe.log. S2/S5 want target\release\sparq.exe;
REM          they report an empty BIN without it, which is itself informative.
setlocal EnableDelayedExpansion
cd /d "%~dp0.."
if not "%~1"=="" goto %~1
if not exist logs mkdir logs >nul 2>&1
set "OUT=logs\probe.log"
echo ===== probe-stamp: defect #69 - CLOSED, kept as the regression probe ===== > "%OUT%"
for %%S in (S0a_buildfail_UNESCAPED S0b_buildfail_ESCAPED S1_dir_four_roots S2_verloop_as_shipped S3_guard_ifs S4_dir_five_roots S5_verloop_findstr) do (
    echo ===== %%S =====>> "%OUT%"
    cmd /v:on /c ""%~f0" %%S" >> "%OUT%" 2>&1
    echo [%%S exited !errorlevel!]>> "%OUT%"
    echo.>> "%OUT%"
)
echo ===== probe complete - send logs\probe.log back =====>> "%OUT%"
type "%OUT%"
exit /b 0

:S0a_buildfail_UNESCAPED
REM Defect #69 as shipped in WO-012: parse-only test (condition false, body never runs - cmd
REM still parses the whole block when it reaches the IF). MUST die with exit != 0.
echo on
if errorlevel 999 (
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
    echo     crates\sparq-app\Cargo.toml (the per-target wgpu features live there).
    echo.
    echo   If the error mentions `alsa` or `pkg-config`:
    echo     you are not on Windows. On Linux install libasound2-dev and pkg-config.
    echo.
    echo   Anything else: copy the whole error into a message back to me.
    exit /b 1
)
echo S0a RESULT: parsed cleanly - THE REPRODUCTION FAILED, investigate
exit /b 0

:S0b_buildfail_ESCAPED
REM The same block with the fix that now ships in build.bat: ^( ^). MUST exit 0.
echo on
if errorlevel 999 (
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
    echo   Anything else: copy the whole error into a message back to me.
    exit /b 1
)
echo S0b RESULT: escaped block parsed cleanly
exit /b 0

:S1_dir_four_roots
REM The four-root dir walk exactly as WO-007 shipped it (pre-fix build.bat; defect #68).
echo on
set CNT=0
set BYTES=0
for /f "delims=" %%F in ('dir /s /b /a-d "crates\sparq-kernel\src\*.rs" "crates\sparq-audio\src\*.rs" "crates\sparq-ui\src\*.rs" "crates\sparq-app\src\*.rs" 2^>nul') do (
    set /a CNT+=1
    set /a BYTES+=%%~zF
)
echo S1 RESULT CNT=!CNT! BYTES=!BYTES!
exit /b 0

:S2_verloop_as_shipped
REM The version-capture loop as WO-007 shipped it: per-line IF with delayed-expansion
REM substring substitution. Exonerated by the first probe run; kept for the record.
echo on
set "BIN="
for /f "usebackq tokens=* delims=" %%A in (`target\release\sparq.exe version 2^>nul`) do (
    set "LINE=%%A"
    if not "!LINE: src =!"=="!LINE!" set "BIN=!LINE:* src =!"
)
echo S2 RESULT BIN=!BIN!
exit /b 0

:S3_guard_ifs
REM The guard's comparison IFs, with plausible values.
echo on
set "CNT=60"
set "BYTES=900000"
set "CUR=!CNT!f/!BYTES!B"
set "BIN=68f/1070593B"
if "!CNT!"=="0" echo S3 never
if "!BIN!"=="" echo S3 never2
if "!CUR!"=="!BIN!" (echo S3 equal) else (echo S3 RESULT mismatch-as-expected CUR=!CUR!)
exit /b 0

:S4_dir_five_roots
REM The FIXED walk: five roots, mirroring build.rs. Expect CNT=68 BYTES=1070593.
echo on
set CNT=0
set BYTES=0
for /f "delims=" %%F in ('dir /s /b /a-d "crates\sparq-kernel\src\*.rs" "crates\sparq-audio\src\*.rs" "crates\sparq-ui\src\*.rs" "crates\sparq-app\src\*.rs" "crates\sparq-module-api\src\*.rs" 2^>nul') do (
    set /a CNT+=1
    set /a BYTES+=%%~zF
)
echo S4 RESULT CNT=!CNT! BYTES=!BYTES! ^(expect 68 and 1070593^)
exit /b 0

:S5_verloop_findstr
REM The FIXED capture: findstr anchors on " src ", loop body is a single set.
echo on
set "BIN="
set "LINE="
for /f "usebackq tokens=* delims=" %%A in (`target\release\sparq.exe version 2^>nul ^| findstr /c:" src "`) do set "LINE=%%A"
if defined LINE set "BIN=!LINE:* src =!"
echo S5 RESULT BIN=!BIN! ^(expect 68f/1070593B^)
exit /b 0
