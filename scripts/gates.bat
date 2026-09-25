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
set "SPARQ_LOG=%~dp0..\logs\gates.log"
call :sparq_main %SPARQ_ARGS% > "%SPARQ_LOG%" 2>&1
set "SPARQ_RC=%errorlevel%"
type "%SPARQ_LOG%"
echo.
echo  log saved to %SPARQ_LOG%
if defined SPARQ_DOUBLE_CLICKED pause
exit /b %SPARQ_RC%

:sparq_main
REM ============================================================================
REM  sparq - build + run every gate, exactly as CI does.
REM  This is the "is it still good?" command. Run it before and after changes.
REM  Usage:  scripts\gates.bat
REM ============================================================================
setlocal EnableDelayedExpansion
cd /d "%~dp0.."
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
set FAILED=

where cargo >nul 2>&1
if errorlevel 1 (
    echo  cargo not found on PATH. Run scripts\setup.bat first.
    exit /b 1
)

REM Zip-synced trees carry the ARCHIVE's timestamps: freshly-synced sources can look
REM older than cached build artifacts, and cargo then re-serves stale clippy/test
REM results without recompiling (defect #41 - a gates run once "passed" against the
REM previous sync's code, which is the worst possible outcome for a gate).
REM
REM The remedy is to delete cargo's fingerprints, NOT to touch sources. Touching was
REM tried and does not work: a touch sets mtime to NOW, but the fingerprint files
REM already record a comparable NOW from a build minutes earlier, so cargo still
REM concludes "fresh" - observed as two consecutive 0.03 s "builds" beside a
REM verified stale binary (defect #49). Deleting .fingerprint removes cargo's
REM evidence of what it built, so every gate below really recompiles this tree.
REM Cost: one full rebuild of the first-party crates per gates run. That is the
REM correct price for a gate table that means something.
if exist target\release\.fingerprint rd /s /q target\release\.fingerprint
if exist target\debug\.fingerprint rd /s /q target\debug\.fingerprint
echo  note: cleared cargo fingerprints so every gate below measures THIS tree.

call :run "rustfmt"            "cargo fmt --all --check"
call :run "clippy default"     "cargo clippy --workspace --all-targets -- -D warnings"
call :run "clippy audio+hal"    "cargo clippy -p sparq-app --features bootstrap-audio,hal-wasapi --all-targets -- -D warnings"
call :run "tests"              "cargo test --workspace"
call :run "golden reference"   "cargo test --release -p sparq-audio --test golden"
call :run "release build"      "cargo build --release -p sparq-app --features bootstrap-audio,hal-wasapi,ui-window"
REM Version stamp + selftest belong in this log: tools/log_check.py reads the stamp, the
REM golden hash, the rt-discipline allocation count, the reopen-leak balance and the
REM realtime figure from gates.log, and on Linux the `just gates` recipe has always
REM produced them there. Without these two stages every one of those fields is
REM permanently MISSING on Windows - an unanswered question, per log_check's own rule.
REM `version` first, so the stamp lands in gates.log even when build.log does not.
call :run "version stamp"      "target\release\sparq.exe version"
call :run "selftest + golden"  "target\release\sparq.exe selftest --golden"
REM The layout audit is a gate like any other (WO-012): the breakpoint x DPI x mode
REM matrix, the 44 px floor, DPI invariance, and synthetic gestures through the real
REM recogniser. It runs headless - no GPU, no window, no excuses.
call :run "ui layout audit"    "target\release\sparq.exe ui --audit"

set HAVE_PY=0
where python >nul 2>&1 && set HAVE_PY=1
where py >nul 2>&1 && set HAVE_PY=1
if "!HAVE_PY!"=="1" (
    where python >nul 2>&1 && set PY=python || set PY=py
    call :run "tools text I/O"       "!PY! tools\check_text_io.py"
    call :run "tokens up to date"   "!PY! tools\token_gen.py --check --quiet"
    call :run "design conformance"  "!PY! tools\token_audit.py"
    call :run "unsafe allowlist"    "!PY! tools\unsafe_audit.py"
) else (
    echo.
    echo  [SKIP] python gates: no `python` or `py` on PATH ^(install Python 3.11+^)
)

echo.
if not "!FAILED!"=="" (
    echo  ============================================
    echo   GATES FAILED:!FAILED!
    echo  ============================================
    echo  Copy the failing section above into a message back to me.
    exit /b 1
)
echo  ============================================
echo   all gates passed
echo  ============================================
exit /b 0

:run
echo.
echo  ---- %~1 ----
%~2
if errorlevel 1 (
    echo  [FAIL] %~1
    set "FAILED=!FAILED! [%~1]"
) else (
    echo  [ ok ] %~1
)
exit /b 0
