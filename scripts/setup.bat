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
set "SPARQ_LOG=%~dp0..\logs\setup.log"
call :sparq_main %SPARQ_ARGS% > "%SPARQ_LOG%" 2>&1
set "SPARQ_RC=%errorlevel%"
type "%SPARQ_LOG%"
echo.
echo  log saved to %SPARQ_LOG%
if defined SPARQ_DOUBLE_CLICKED pause
exit /b %SPARQ_RC%

:sparq_main
REM ============================================================================
REM  sparq - one-time Windows environment setup
REM
REM  Installs: Rust (stable, MSVC toolchain) and, if missing, the Visual Studio
REM  C++ Build Tools that the MSVC linker needs. Then verifies the toolchain.
REM
REM  Safe to re-run: every step is idempotent and skips what already exists.
REM  Needs: an internet connection, and Administrator for the Build Tools step
REM         (winget will prompt for UAC).
REM
REM  Usage:  double-click, or from a terminal:  scripts\setup.bat
REM ============================================================================
setlocal EnableDelayedExpansion
chcp 850 >nul 2>&1
cd /d "%~dp0.."
set "ROOT=%CD%"
set "CARGO_BIN=%USERPROFILE%\.cargo\bin"
set "PATH=%CARGO_BIN%;%PATH%"

echo.
echo  ============================================
echo   sparq setup - Windows development environment
echo  ============================================
echo   project root : %ROOT%
echo.

REM ---------------------------------------------------------------- step 1: Rust
where cargo >nul 2>&1
if %errorlevel%==0 (
    echo  [1/4] Rust: already installed
    cargo --version
) else (
    echo  [1/4] Rust: not found - installing rustup ^(stable, MSVC toolchain^)
    echo        This downloads ~200 MB. A UAC prompt may appear.
    set "RUSTUP_INIT=%TEMP%\rustup-init.exe"
    powershell -NoProfile -ExecutionPolicy Bypass -Command ^
      "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile $env:TEMP\rustup-init.exe"
    if not exist "%TEMP%\rustup-init.exe" (
        echo.
        echo  ERROR: could not download rustup-init.exe.
        echo  Check your connection, or install manually from https://rustup.rs and re-run.
        goto :fail
    )
    "%TEMP%\rustup-init.exe" -y --default-toolchain stable --default-host x86_64-pc-windows-msvc --profile default
    if errorlevel 1 (
        echo  ERROR: rustup-init failed.
        goto :fail
    )
    if not exist "%CARGO_BIN%\cargo.exe" (
        echo  ERROR: rustup finished but %CARGO_BIN%\cargo.exe is missing.
        goto :fail
    )
    echo        Rust installed. PATH updated for new terminals automatically.
)

REM ------------------------------------------------- step 2: MSVC linker (link.exe)
echo.
echo  [2/4] Checking for the MSVC C++ linker ^(link.exe^)
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
set "HAVE_LINK=0"
if exist "%VSWHERE%" (
    for /f "usebackq tokens=*" %%i in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2^>nul`) do (
        if exist "%%i\VC\Tools\MSVC" set "HAVE_LINK=1"
    )
)
if "%HAVE_LINK%"=="1" (
    echo        Found: Visual Studio C++ tools are installed.
) else (
    echo        Not found. Rust's MSVC toolchain cannot link without them.
    echo.
    echo        About to install "Visual Studio Build Tools 2022" with the C++ workload
    echo        via winget. This is a ~2-6 GB download. Press Ctrl+C now to abort and
    echo        install it yourself later; otherwise press any key to continue.
    pause >nul
    where winget >nul 2>&1
    if errorlevel 1 (
        echo.
        echo  WARNING: winget is not available, so sparq cannot install Build Tools for you.
        echo  Install manually:
        echo     https://visualstudio.microsoft.com/downloads/  -^> "Build Tools for Visual Studio"
        echo     In the installer, tick "Desktop development with C++".
        echo  Then re-run scripts\setup.bat.
        echo.
        echo  Continuing anyway - the rest of the setup is still useful.
    ) else (
        winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended" --accept-source-agreements --accept-package-agreements
        if errorlevel 1 (
            echo  WARNING: the Build Tools install returned an error. It may still have succeeded,
            echo           or you may need to install it manually ^(see the URL above^).
        ) else (
            echo        Build Tools installed.
        )
    )
)

REM ------------------------------------------------------------ step 3: dependencies
echo.
echo  [3/4] Fetching crate dependencies
cargo fetch
if errorlevel 1 (
    echo  WARNING: `cargo fetch` reported an error. The build step will show details.
)

REM ---------------------------------------------------------------- step 4: verify
echo.
echo  [4/4] Verifying
call "%ROOT%\scripts\verify.bat"
if errorlevel 1 goto :fail

echo.
echo  ============================================
echo   setup complete
echo  ============================================
echo   next:  scripts\build.bat      build sparq.exe
echo          scripts\devices.bat    see your audio devices
echo          scripts\run.bat        make sound
echo   full guide: WINDOWS.md
echo.
endlocal
exit /b 0

:fail
echo.
echo  SETUP FAILED - see the message above.
echo  Full guide and manual steps: WINDOWS.md
echo.
endlocal
exit /b 1
