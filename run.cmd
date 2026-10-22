@echo off
rem LoL Coach launcher.
rem   run.cmd           start the app (builds it first if needed)
rem   run.cmd debug     start with debug logging + dumps, and follow the log here
rem   run.cmd build     rebuild, then start (combine: run.cmd build debug)
setlocal
cd /d "%~dp0"
set "EXE=target\release\lol-coach.exe"
set "LOG=%LOCALAPPDATA%\lol-coach\logs\lol-coach.log"
set DEBUG=
set BUILD=
for %%a in (%*) do (
  if /i "%%a"=="debug" set DEBUG=1
  if /i "%%a"=="build" set BUILD=1
)
if not exist "%EXE%" set BUILD=1

if defined BUILD (
  echo Building LoL Coach...
  if not exist "web\node_modules" call npm --prefix web install || goto :fail
  call web\node_modules\.bin\tauri build --no-bundle || goto :fail
)

tasklist /fi "imagename eq lol-coach.exe" | find /i "lol-coach.exe" >nul && (
  echo LoL Coach is already running; quit it from the tray first to switch modes.
  goto :eof
)

if defined DEBUG (
  set LOL_COACH_DEBUG=1
  rem lets Claude inspect the UI over CDP on 127.0.0.1:9333
  set "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333"
  start "" "%EXE%" --debug
  echo Debug mode: dumps in %LOCALAPPDATA%\lol-coach\debug\^<newest^>\
  echo Following %LOG%  ^(close this window any time; the app keeps running^)
  timeout /t 2 /nobreak >nul
  powershell -NoProfile -Command "Get-Content -Wait -Tail 40 '%LOG%'"
) else (
  start "" "%EXE%"
)
goto :eof

:fail
echo Build failed.
pause
exit /b 1
