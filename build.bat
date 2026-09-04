@echo off
REM ============================================================================
REM  NYEDArch Framework - one-command build (Windows)
REM
REM    build.bat                 build everything, run tests, produce .\release
REM    build.bat --skip-gui      skip the desktop client
REM    build.bat --skip-tests    build only (not recommended)
REM    build.bat --skip-bench    skip the benchmark run
REM    build.bat --clean         remove previous build artifacts first
REM
REM  Produces .\release containing everything intended for an end user.
REM ============================================================================

setlocal EnableDelayedExpansion
set "ROOT=%~dp0"
if "%ROOT:~-1%"=="\" set "ROOT=%ROOT:~0,-1%"
set "RELEASE=%ROOT%\release"

set SKIP_GUI=0
set SKIP_TESTS=0
set SKIP_BENCH=0
set DO_CLEAN=0

:parse
if "%~1"=="" goto parsed
if /I "%~1"=="--skip-gui"    set SKIP_GUI=1&    shift & goto parse
if /I "%~1"=="--skip-tests"  set SKIP_TESTS=1&  shift & goto parse
if /I "%~1"=="--skip-bench"  set SKIP_BENCH=1&  shift & goto parse
if /I "%~1"=="--clean"       set DO_CLEAN=1&    shift & goto parse
if /I "%~1"=="--help"        goto usage
if /I "%~1"=="-h"            goto usage
echo unknown option: %~1  (try --help)
exit /b 2
:parsed

echo.
echo ==============================================
echo   NYEDArch Framework - build
echo   Not Your Everyday Archive
echo ==============================================

REM ------------------------------------------------------------- toolchain ---
echo.
echo ==^> Checking toolchain
where cargo >nul 2>&1
if errorlevel 1 (
    echo     [X] cargo not found. Install Rust from https://rustup.rs
    exit /b 1
)
for /f "delims=" %%v in ('rustc --version') do set "RUSTC_V=%%v"
echo     [ok] !RUSTC_V!

where curl >nul 2>&1
if errorlevel 1 (
    echo     [!] curl not found - remote GitHub builds will not work until it is installed.
    echo         Windows 10 build 1803 and later include curl.exe.
) else (
    echo     [ok] curl present ^(HTTP transport for remote builds^)
)

if "%DO_CLEAN%"=="1" (
    echo.
    echo ==^> Cleaning previous artifacts
    cargo clean --manifest-path "%ROOT%\Cargo.toml" >nul 2>&1
    if exist "%ROOT%\gui\nyedarch-gui\Cargo.toml" cargo clean --manifest-path "%ROOT%\gui\nyedarch-gui\Cargo.toml" >nul 2>&1
    if exist "%RELEASE%" rmdir /s /q "%RELEASE%"
    echo     [ok] cleaned
)

REM ----------------------------------------------------------------- tests ---
set "TEST_SUMMARY=skipped"
if "%SKIP_TESTS%"=="0" (
    echo.
    echo ==^> Running test suite
    cargo test --manifest-path "%ROOT%\Cargo.toml" > "%TEMP%\nyedarch_test.log" 2>&1
    if errorlevel 1 (
        type "%TEMP%\nyedarch_test.log"
        echo     [X] tests failed - refusing to produce a release from a failing build
        exit /b 1
    )
    set "TEST_SUMMARY=all passed"
    echo     [ok] all tests passed
) else (
    echo     [!] tests skipped ^(--skip-tests^)
)

REM ----------------------------------------------------------------- build ---
echo.
echo ==^> Building workspace ^(release profile^)
cargo build --release --manifest-path "%ROOT%\Cargo.toml"
if errorlevel 1 (
    echo     [X] build failed
    exit /b 1
)
echo     [ok] client, installer and libraries built

echo.
echo ==^> Verifying the cryptographic core builds with zero platform surface
cargo build --release -p nyedarch-crypto --no-default-features --manifest-path "%ROOT%\Cargo.toml" >nul 2>&1
if errorlevel 1 (
    echo     [X] crypto core failed to build without default features
    exit /b 1
)
echo     [ok] nyedarch-crypto builds with --no-default-features

set GUI_BUILT=0
if "%SKIP_GUI%"=="0" (
    if exist "%ROOT%\gui\nyedarch-gui\Cargo.toml" (
        echo.
        echo ==^> Building desktop client ^(this takes several minutes^)
        cargo build --release --manifest-path "%ROOT%\gui\nyedarch-gui\Cargo.toml"
        if errorlevel 1 (
            echo     [!] desktop client failed to build.
            echo         Ensure the MSVC build tools and Windows SDK are installed.
            echo         Continuing - the command-line client is fully functional without it.
        ) else (
            set GUI_BUILT=1
            echo     [ok] desktop client built
        )
    )
) else (
    echo     [!] desktop client skipped ^(--skip-gui^)
)

REM ------------------------------------------------------------ benchmarks ---
if "%SKIP_BENCH%"=="0" (
    echo.
    echo ==^> Measuring performance on this machine
    "%ROOT%\target\release\nyedarch-buildtool.exe" bench > "%TEMP%\nyedarch_bench.txt" 2>&1
    if errorlevel 1 (
        echo     [!] benchmark run did not complete; continuing
    ) else (
        echo     [ok] benchmarks recorded
    )
)

REM --------------------------------------------------------------- release ---
echo.
echo ==^> Assembling release directory
if exist "%RELEASE%" rmdir /s /q "%RELEASE%"
mkdir "%RELEASE%\bin"
mkdir "%RELEASE%\docs\license-server"
mkdir "%RELEASE%\docs\decisions"
mkdir "%RELEASE%\runtime-src"

copy /y "%ROOT%\target\release\nyedarch-buildtool.exe" "%RELEASE%\bin\nyedarch.exe" >nul
echo     [ok] bin\nyedarch.exe ^(command-line client^)

REM The installer sits at the top of the release, not in bin, because it is the
REM first thing a user runs and should be impossible to miss.
copy /y "%ROOT%\target\release\nyedarch-install.exe" "%RELEASE%\install.exe" >nul
echo     [ok] install.exe ^(run this first^)

if "%GUI_BUILT%"=="1" (
    copy /y "%ROOT%\gui\nyedarch-gui\target\release\nyedarch-gui.exe" "%RELEASE%\bin\nyedarch-gui.exe" >nul
    echo     [ok] bin\nyedarch-gui.exe ^(desktop client^)
)

REM Runtime sources, vendored into every generated capsule project. Without
REM these the client can seal a package but cannot produce a capsule on a
REM machine that has no NYEDArch source tree.
for %%C in (nyedarch-crypto nyedarch-core nyedarch-package nyedarch-fingerprint nyedarch-platform nyedarch-runtime) do (
    robocopy "%ROOT%\crates\%%C" "%RELEASE%\runtime-src\%%C" /E /XD target .git /NFL /NDL /NJH /NJS /NP >nul
)
echo     [ok] runtime-src\ ^(6 crates, vendored into generated capsules^)

robocopy "%ROOT%\docs" "%RELEASE%\docs" *.md /NFL /NDL /NJH /NJS /NP >nul
robocopy "%ROOT%\docs\decisions" "%RELEASE%\docs\decisions" *.md /NFL /NDL /NJH /NJS /NP >nul
robocopy "%ROOT%\docs\license-server" "%RELEASE%\docs\license-server" *.md /NFL /NDL /NJH /NJS /NP >nul
echo     [ok] docs\

if exist "%ROOT%\README.md" copy /y "%ROOT%\README.md" "%RELEASE%\" >nul
if exist "%ROOT%\docs\EULA.md" copy /y "%ROOT%\docs\EULA.md" "%RELEASE%\EULA.md" >nul
if exist "%ROOT%\NYEDArch_Technical_Documentation.docx" copy /y "%ROOT%\NYEDArch_Technical_Documentation.docx" "%RELEASE%\" >nul
if exist "%ROOT%\NYEDArch_Technical_Documentation.pdf" copy /y "%ROOT%\NYEDArch_Technical_Documentation.pdf" "%RELEASE%\" >nul
if exist "%TEMP%\nyedarch_bench.txt" copy /y "%TEMP%\nyedarch_bench.txt" "%RELEASE%\BENCHMARKS.txt" >nul

if exist "%ROOT%\release-assets\START_HERE.md" (
    copy /y "%ROOT%\release-assets\START_HERE.md" "%RELEASE%\START_HERE.md" >nul
    echo     [ok] START_HERE.md
)

REM ----------------------------------------------------------- build record --
set "BUILDINFO=%RELEASE%\BUILD_INFO.txt"
> "%BUILDINFO%" echo NYEDArch Framework - build record
>>"%BUILDINFO%" echo =================================
>>"%BUILDINFO%" echo.
>>"%BUILDINFO%" echo Built:      %DATE% %TIME%
>>"%BUILDINFO%" echo Host:       Windows %PROCESSOR_ARCHITECTURE%
>>"%BUILDINFO%" echo Toolchain:  !RUSTC_V!
>>"%BUILDINFO%" echo Tests:      !TEST_SUMMARY!
>>"%BUILDINFO%" echo.
>>"%BUILDINFO%" echo Install
>>"%BUILDINFO%" echo -------
>>"%BUILDINFO%" echo   install.exe              install for the current user
>>"%BUILDINFO%" echo   install.exe --verify     check integrity without installing
>>"%BUILDINFO%" echo   install.exe --uninstall  remove a previous installation
>>"%BUILDINFO%" echo.
>>"%BUILDINFO%" echo Contents
>>"%BUILDINFO%" echo --------
>>"%BUILDINFO%" echo   install.exe       installer ^(start here^)
>>"%BUILDINFO%" echo   bin\nyedarch.exe  command-line client
>>"%BUILDINFO%" echo   runtime-src\      runtime crates vendored into generated capsules
>>"%BUILDINFO%" echo   docs\             architecture, security, threat model, EULA
>>"%BUILDINFO%" echo   SHA256SUMS        checksums for everything in this directory
>>"%BUILDINFO%" echo.
>>"%BUILDINFO%" echo Windows note
>>"%BUILDINFO%" echo ------------
>>"%BUILDINFO%" echo   Typing capsule.nyarch into cmd or PowerShell will NOT run it, because
>>"%BUILDINFO%" echo   Windows decides what is executable from the file extension. Use
>>"%BUILDINFO%" echo   "nyedarch run capsule.nyarch", drag it onto the desktop client, or add
>>"%BUILDINFO%" echo   .NYARCH to your PATHEXT.
echo     [ok] BUILD_INFO.txt

REM ------------------------------------------------------------- checksums ---
echo.
echo ==^> Generating checksums
pushd "%RELEASE%"
if exist SHA256SUMS del SHA256SUMS
for /r %%F in (*) do (
    if /I not "%%~nxF"=="SHA256SUMS" (
        set "FP=%%F"
        set "REL=!FP:%RELEASE%\=!"
        set "HASH="
        for /f "delims=" %%H in ('certutil -hashfile "%%F" SHA256 ^| findstr /r "^[0-9a-fA-F][0-9a-fA-F]*$"') do (
            if not defined HASH set "HASH=%%H"
        )
        if defined HASH >> SHA256SUMS echo !HASH!  !REL:\=/!
    )
)
popd
echo     [ok] SHA256SUMS

REM --------------------------------------------------------------- summary ---
echo.
echo ==^> Done
echo     Release directory: %RELEASE%
echo     Tests: !TEST_SUMMARY!
if "%GUI_BUILT%"=="0" echo     [!] Desktop client not included - see messages above
echo.
echo     Give the whole release\ directory to the end user.
echo     They should run install.exe, then read START_HERE.md.
echo.
endlocal
exit /b 0

:usage
echo   build.bat                 build everything, run tests, produce .\release
echo   build.bat --skip-gui      skip the desktop client
echo   build.bat --skip-tests    build only ^(not recommended^)
echo   build.bat --skip-bench    skip the benchmark run
echo   build.bat --clean         remove previous build artifacts first
exit /b 0
