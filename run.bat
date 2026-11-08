@echo off
rem Runs Big Star Editor from the source code in this script's folder, on Windows. On Linux and macOS, run.sh does the same.
rem
rem The first run installs what building needs when it is missing, through winget:
rem   - Rust (rustup, with the stable toolchain);
rem   - the Visual Studio Build Tools' C++ compiler and a Windows SDK, which Rust links Windows programs with.
rem Cargo then only recompiles what changed since the last run, so an unchanged tree starts straight away.
rem After building, the debug symbols (.pdb) are removed; Cargo does not track them, so this never causes a recompile.
rem
rem Everything the installers, the build, the cleanup and the editor print goes to target\run.log, replaced on every run.
rem Arguments are passed to the editor, so `run.bat path\to\workspace` opens that workspace.

setlocal
cd /d "%~dp0"
if not exist "target" mkdir "target"
set "LOG=%~dp0target\run.log"
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
set "VSSETUP=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\setup.exe"
set "KITS=%ProgramFiles(x86)%\Windows Kits\10\Lib"
set "VC_COMPONENTS=--add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.26100"

> "%LOG%" echo [%date% %time%] Big Star Editor run started in %CD%

call :ensure_rust || goto :failed
call :ensure_msvc || goto :failed

echo Building...
call :section Build
cargo build --release -p bigstar >> "%LOG%" 2>&1
if errorlevel 1 (
    >> "%LOG%" echo [%date% %time%] Build failed
    echo Build failed.
    goto :failed
)

echo Cleaning up...
call :section Cleanup
for /r "target\release" %%F in (*.pdb) do (
    >> "%LOG%" echo Deleted %%F
    del /q "%%F" >nul 2>&1
)
for %%D in (target\release\examples target\release\incremental target\doc target\tmp target\package) do (
    if exist "%%D" (
        >> "%LOG%" echo Removed %%D
        rd /s /q "%%D"
    )
)

echo Starting the editor...
call :section Editor
cargo run --release -p bigstar -- %* >> "%LOG%" 2>&1
set "CODE=%errorlevel%"
>> "%LOG%" echo [%date% %time%] Editor exited with code %CODE%
if not "%CODE%"=="0" (
    echo The editor exited with code %CODE%.
    goto :failed
)
exit /b 0


rem ---- Rust -----------------------------------------------------------------------------------------------------------

:ensure_rust
where cargo >nul 2>&1 && exit /b 0
rem Installed in an earlier run, but this window's PATH predates it.
if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
    exit /b 0
)
echo Rust is not installed: installing it with winget...
call :section "Rust install"
call :need_winget || exit /b 1
winget install --id Rustlang.Rustup -e --accept-source-agreements --accept-package-agreements
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where cargo >nul 2>&1 || goto :rust_failed
rustup default stable >> "%LOG%" 2>&1
>> "%LOG%" echo Rust installed.
exit /b 0

:rust_failed
>> "%LOG%" echo Rust could not be installed.
echo Rust could not be installed. Install it from https://rustup.rs and run this script again.
exit /b 1


rem ---- C++ build tools ------------------------------------------------------------------------------------------------

:ensure_msvc
call :has_msvc && exit /b 0
call :section "C++ build tools install"
rem Visual Studio or its Build Tools are there without the C++ compiler: add it to that installation.
set "VSPATH="
if exist "%VSWHERE%" for /f "usebackq delims=" %%P in (`"%VSWHERE%" -latest -products * -property installationPath`) do set "VSPATH=%%P"
if not defined VSPATH goto :install_build_tools
echo The C++ build tools are missing: adding them to %VSPATH%...
echo Windows asks for permission, and the Visual Studio Installer shows its progress.
>> "%LOG%" echo Modifying %VSPATH%
powershell -NoProfile -Command "$arguments = 'modify --installPath ' + [char]34 + $env:VSPATH + [char]34 + ' ' + $env:VC_COMPONENTS + ' --passive --norestart'; exit (Start-Process -FilePath $env:VSSETUP -ArgumentList $arguments -Verb RunAs -Wait -PassThru).ExitCode"
>> "%LOG%" echo Visual Studio Installer exited with code %errorlevel%
call :has_msvc && exit /b 0

:install_build_tools
echo The C++ build tools are missing: installing the Visual Studio Build Tools with winget...
echo Windows asks for permission, and the Visual Studio Installer shows its progress.
call :need_winget || exit /b 1
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-source-agreements --accept-package-agreements --override "%VC_COMPONENTS% --passive --wait --norestart"
>> "%LOG%" echo winget exited with code %errorlevel%
call :has_msvc && exit /b 0
>> "%LOG%" echo The C++ build tools could not be installed.
echo The C++ build tools could not be installed. In the Visual Studio Installer, add "Desktop development with C++"
echo to your Visual Studio or Build Tools, then run this script again.
exit /b 1

rem Whether an installation has the x64 C++ compiler, and a Windows SDK has the libraries programs link with.
:has_msvc
if not exist "%VSWHERE%" exit /b 1
set "FOUND="
for /f "usebackq delims=" %%P in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "FOUND=%%P"
if not defined FOUND exit /b 1
if not exist "%KITS%" exit /b 1
for /d %%V in ("%KITS%\10.*") do if exist "%%V\um\x64\kernel32.lib" exit /b 0
exit /b 1


rem ---- Helpers --------------------------------------------------------------------------------------------------------

:need_winget
where winget >nul 2>&1 && exit /b 0
>> "%LOG%" echo winget is not available.
echo winget is not available: install "App Installer" from the Microsoft Store, or install what is missing by hand.
exit /b 1

:section
>> "%LOG%" echo.
>> "%LOG%" echo [%date% %time%] %~1
exit /b 0

:failed
echo The last lines of %LOG%:
powershell -NoProfile -Command "Get-Content -LiteralPath $env:LOG -Tail 30"
pause
exit /b 1
