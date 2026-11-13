@echo off
chcp 65001 >nul
title NeuroVeil VPN Builder

echo ===================================================
echo           NeuroVeil VPN - Сборка релиза
echo ===================================================
echo.

cd /d "%~dp0"

where node >nul 2>nul
if %errorlevel% neq 0 (
    echo [ОШИБКА] Node.js не найден в PATH! Установите Node.js с nodejs.org
    pause
    exit /b 1
)

where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo [ОШИБКА] Rust/Cargo не найден в PATH! Установите Rust с rustup.rs
    pause
    exit /b 1
)

for /f "usebackq delims=" %%v in (`node -p "require('./package.json').version" 2^>nul`) do (
    set "APP_VERSION=%%v"
)

if "%APP_VERSION%"=="" set "APP_VERSION=1.0.3"

echo [ИНФО] Версия проекта: v%APP_VERSION%
echo [ИНФО] Запуск компиляции фронтенда и Rust Tauri...
echo.

call npm run tauri build
if %errorlevel% neq 0 (
    echo.
    echo ===================================================
    echo [ОШИБКА] Сборка завершилась с ошибкой!
    echo ===================================================
    pause
    exit /b %errorlevel%
)

echo.
echo [ИНФО] Сборка успешно завершена!
echo [ИНФО] Копирование и упаковка релизных файлов...

set "EXE_SRC=src-tauri\target\release\neuroveil-vpn.exe"
set "NSIS_SRC=src-tauri\target\release\bundle\nsis\NeuroVeil VPN_%APP_VERSION%_x64-setup.exe"

if exist "%EXE_SRC%" (
    copy /y "%EXE_SRC%" "NeuroVeil.exe" >nul
    copy /y "%EXE_SRC%" "NeuroVeil_v%APP_VERSION%.exe" >nul
    echo   [+] Скопирован: NeuroVeil.exe
    echo   [+] Скопирован: NeuroVeil_v%APP_VERSION%.exe
)

if exist "%NSIS_SRC%" (
    copy /y "%NSIS_SRC%" "NeuroVeil_VPN_v%APP_VERSION%_Setup.exe" >nul
    echo   [+] Скопирован инсталлятор: NeuroVeil_VPN_v%APP_VERSION%_Setup.exe
) else (
    for %%f in ("src-tauri\target\release\bundle\nsis\*setup*.exe") do (
        copy /y "%%f" "NeuroVeil_VPN_v%APP_VERSION%_Setup.exe" >nul
        echo   [+] Скопирован инсталлятор: NeuroVeil_VPN_v%APP_VERSION%_Setup.exe
    )
)

set "PORTABLE_DIR=NeuroVeil-Portable-v%APP_VERSION%"
if not exist "%PORTABLE_DIR%" mkdir "%PORTABLE_DIR%"
if not exist "%PORTABLE_DIR%\data" mkdir "%PORTABLE_DIR%\data"

if exist "%EXE_SRC%" copy /y "%EXE_SRC%" "%PORTABLE_DIR%\NeuroVeil.exe" >nul
if exist "WebView2Loader.dll" copy /y "WebView2Loader.dll" "%PORTABLE_DIR%\" >nul
if exist "wintun.dll" copy /y "wintun.dll" "%PORTABLE_DIR%\" >nul
echo   [+] Обновлена папка: %PORTABLE_DIR%

echo.
echo ===================================================
echo           СБОРКА УСПЕШНО ЗАВЕРШЕНА!
echo ===================================================
echo Готовые файлы в корне проекта:
echo   - NeuroVeil.exe
echo   - NeuroVeil_v%APP_VERSION%.exe
echo   - NeuroVeil_VPN_v%APP_VERSION%_Setup.exe
echo   - %PORTABLE_DIR%\
echo.
pause
