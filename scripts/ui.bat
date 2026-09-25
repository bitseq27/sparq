@echo off
REM Capture the arguments BEFORE any `call` (see build.bat for the full story: this was a real bug).
set "SPARQ_ARGS=%*"
if defined CMD_CLICK_STARTED set "SPARQ_DOUBLE_CLICKED=1"
REM UTF-8 for the binary's output; this file itself stays pure ASCII (defect #42, check_text_io rule 3).
chcp 65001 >nul 2>&1
if not exist "%~dp0..\logs" mkdir "%~dp0..\logs" >nul 2>&1
set "SPARQ_LOG=%~dp0..\logs\ui.log"
call :sparq_main %SPARQ_ARGS% > "%SPARQ_LOG%" 2>&1
set "SPARQ_RC=%errorlevel%"
type "%SPARQ_LOG%"
echo.
echo  log saved to %SPARQ_LOG%
if defined SPARQ_DOUBLE_CLICKED pause
exit /b %SPARQ_RC%

:sparq_main
REM ============================================================================
REM  sparq - open the WO-012 shell prototype window.
REM
REM  Usage:
REM    scripts\ui.bat                  open the window (touch/mouse/pen live)
REM    scripts\ui.bat --audit          the headless layout gate (no window)
REM    scripts\ui.bat --headless 120   headless frame statistics (no window)
REM    scripts\ui.bat --contrast       start in the high-contrast theme
REM
REM  Notes:
REM   - Over RDP, wgpu falls back to the WARP software rasteriser: the shell
REM     will open and run, but slowly. The 60 fps acceptance number belongs to
REM     the PHYSICAL machine (see docs/ui/windows-dpi-notes.md).
REM   - Touch works over RDP only if the RDP client forwards touch; a mouse
REM     exercises the same gesture recogniser (a mouse IS a pointer).
REM   - Esc closes the window.
REM ============================================================================
setlocal
cd /d "%~dp0.."
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

if not exist "target\release\sparq.exe" (
    echo  target\release\sparq.exe not found - run scripts\build.bat first.
    exit /b 1
)

target\release\sparq.exe ui %SPARQ_ARGS%
set RC=%errorlevel%
if "%RC%"=="2" (
    echo.
    echo  exit 2 usually means this binary was built without the ui-window feature.
    echo  Rebuild with:  scripts\build.bat   ^(it passes --features ...,ui-window^)
)
exit /b %RC%
