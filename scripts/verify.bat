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
chcp 65001 >nul 2>&1
if not exist "%~dp0..\logs" mkdir "%~dp0..\logs" >nul 2>&1
set "SPARQ_LOG=%~dp0..\logs\verify.log"
call :sparq_main %SPARQ_ARGS% > "%SPARQ_LOG%" 2>&1
set "SPARQ_RC=%errorlevel%"
type "%SPARQ_LOG%"
echo.
echo  log saved to %SPARQ_LOG%
if defined SPARQ_DOUBLE_CLICKED pause
exit /b %SPARQ_RC%

:sparq_main
REM ============================================================================
REM  sparq - verify the toolchain can actually build and run.
REM  Called by setup.bat; also useful on its own after any toolchain change.
REM  Usage:  scripts\verify.bat
REM ============================================================================
setlocal EnableDelayedExpansion
cd /d "%~dp0.."
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
set FAILED=

echo  --- toolchain ---
call :chk "rustc --version"                 rustc --version
call :chk "cargo --version"                 cargo --version
call :chk "active toolchain"                rustup show active-toolchain
rustc -vV | findstr /B "host:"

echo.
echo  --- linker ---
echo fn main(){} > "%TEMP%\sparq_linkcheck.rs"
rustc -o "%TEMP%\sparq_linkcheck.exe" "%TEMP%\sparq_linkcheck.rs" >nul 2>&1
if errorlevel 1 (
    echo  [FAIL] rustc could not link a trivial program.
    echo         This means the MSVC C++ Build Tools are missing or not visible.
    echo         Fix: run scripts\setup.bat, or install "Build Tools for Visual Studio"
    echo         with the "Desktop development with C++" workload, then open a NEW terminal.
    set "FAILED=!FAILED! [linker]"
) else (
    echo  [ ok ] link.exe reachable
)
del "%TEMP%\sparq_linkcheck.rs" "%TEMP%\sparq_linkcheck.exe" >nul 2>&1

echo.
echo  --- builds ---
call :chk "sparq-kernel (no dependencies)"  cargo build -q -p sparq-kernel
call :chk "sparq-app + audio backend"       cargo build -q --release -p sparq-app --features bootstrap-audio

echo.
echo  --- offline audio gates (no sound card needed) ---
call :chk "selftest (Phase 0 gate table)"   cargo run -q -p sparq-app -- selftest --golden
call :chk "allocation probe"                cargo run -q --release -p sparq-app --example probe_alloc

echo.
echo  --- python gates (optional) ---
set PY=
where python >nul 2>&1 && set PY=python
if not defined PY where py >nul 2>&1 && set PY=py
if defined PY (
    !PY! --version
    call :chk "design token conformance"    !PY! tools\token_audit.py
    call :chk "unsafe allowlist"            !PY! tools\unsafe_audit.py
    call :chk "tokens up to date"           !PY! tools\token_gen.py --check --quiet
) else (
    echo  [SKIP] no `python` or `py` on PATH - install Python 3.11+ to run the design gates
)

echo.
if not "!FAILED!"=="" (
    echo  VERIFY FAILED:!FAILED!
    exit /b 1
)
echo  VERIFY: toolchain is working. Next: scripts\devices.bat then scripts\run.bat
exit /b 0

:chk
set "LABEL=%~1"
shift
%1 %2 %3 %4 %5 %6 %7 %8 %9
if errorlevel 1 (
    echo  [FAIL] %LABEL%
    set "FAILED=!FAILED! [%LABEL%]"
) else (
    echo  [ ok ] %LABEL%
)
exit /b 0
