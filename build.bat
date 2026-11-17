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

if "%APP_VERSION%"=="" set "APP_VERSION=1.0.8"

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
echo [ИНФО] Обновление основного исполняемого файла...

set "EXE_SRC=src-tauri\target\release\neuroveil-vpn.exe"

:: Удаляем старые версионные дубликаты, чтобы не засорять корень проекта
del /q "NeuroVeil_v*.exe" 2>nul
del /q "NeuroVeil_VPN_v*_Setup.exe" 2>nul

if exist "%EXE_SRC%" (
    copy /y "%EXE_SRC%" "NeuroVeil.exe" >nul
    echo   [+] Обновлен: NeuroVeil.exe (v%APP_VERSION%)
)

echo.
echo ===================================================
echo           СБОРКА УСПЕШНО ЗАВЕРШЕНА!
echo ===================================================
echo Готовый файл в корне проекта:
echo   - NeuroVeil.exe (v%APP_VERSION%)
echo.
pause
