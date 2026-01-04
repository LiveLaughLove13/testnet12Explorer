@echo off
REM Enhanced seeder for Kaspa Testnet 12 Explorer
REM This script ensures proper data flow from both local node and peer

echo 🌱 Kaspa Testnet 12 Explorer - Enhanced Data Seeder
echo ==========================================================
echo 📡 Checking if kaspad is running...

REM Check if curl is available
curl --version >nul 2>&1
if %errorlevel% equ 0 (
    echo ✅ curl is available
) else (
    echo ❌ curl is not available, please install curl
    pause
    exit /b 1
)

echo 🔗 Testing connection to kaspad...

REM Try direct connection to known kaspad ports
echo 📡 Testing 127.0.0.1:16210 (working port from start-explorer.bat)...
curl -s --max-time 3 http://127.0.0.1:16210/ >nul 2>&1
if %errorlevel% equ 0 (
    echo ✅ Kaspad is running on port 16210
    set KASPAD_URL=127.0.0.1:16210
    goto :start_explorer
)

echo 📡 Testing 127.0.0.1:16310...
curl -s --max-time 3 http://127.0.0.1:16310/ >nul 2>&1
if %errorlevel% equ 0 (
    echo ✅ Kaspad is running on port 16310
    set KASPAD_URL=127.0.0.1:16310
    goto :start_explorer
)

echo 📡 Testing 127.0.0.1:16311...
curl -s --max-time 3 http://127.0.0.1:16311/ >nul 2>&1
if %errorlevel% equ 0 (
    echo ✅ Kaspad is running on port 16311
    set KASPAD_URL=127.0.0.1:16311
    goto :start_explorer
)

echo 📡 Testing external 89.58.46.206:16310...
curl -s --max-time 3 http://89.58.46.206:16310/ >nul 2>&1
if %errorlevel% equ 0 (
    echo ✅ External Kaspad is running on 89.58.46.206:16310
    set KASPAD_URL=89.58.46.206:16310
    goto :start_explorer
)

echo 📡 Testing external 89.58.46.206:16311...
curl -s --max-time 3 http://89.58.46.206:16311/ >nul 2>&1
if %errorlevel% equ 0 (
    echo ✅ External Kaspad is running on 89.58.46.206:16311
    set KASPAD_URL=89.58.46.206:16311
    goto :start_explorer
)

echo 📡 Fallback: Using working connection to 127.0.0.1:16210
echo 💡 This is the same URL that works in start-explorer.bat
echo 💡 If this fails, please start kaspad manually in another terminal:
echo    cd D:\rusty-kaspa-covpp\rusty-kaspa
echo    cargo run --release --bin kaspad -- --utxoindex --testnet --netsuffix=12 --enable-unsynced-mining --listen=0.0.0.0:16210 --addpeer=82.166.83.140 --appdir "D:\testnet12"
echo.
set KASPAD_URL=127.0.0.1:16210
goto :start_explorer

:start_explorer
echo.
echo 🚀 Starting Enhanced Kaspa Explorer...
echo 📊 Explorer will connect to: %KASPAD_URL%
echo 👥 Peer data: 82.166.83.140:16311
echo 🌐 Web interface: http://localhost:3000
echo 📈 Features: Balance Cache + Peer Info + Accurate Mempool
echo.

REM Start explorer
cd /d "%~dp0"
cargo run --release -- --kaspad-url "%KASPAD_URL%" --port 3000
