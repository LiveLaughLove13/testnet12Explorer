@echo off
REM Enhanced seeder for Kaspa Testnet 12 Explorer
REM This script ensures proper data flow from both local node and peer

echo 🌱 Kaspa Testnet 12 Explorer - Enhanced Data Seeder
echo ==========================================================

REM Check if kaspad is running
echo 📡 Checking if kaspad is running...

REM Try to connect to kaspad RPC
curl --version >nul 2>&1
if %errorlevel% equ 0 (
    echo ✅ curl is available
) else (
    echo ❌ curl is not available, please install curl
    pause
    exit /b 1
)

REM Test connection to kaspad on different ports
echo 🔗 Testing connection to kaspad...

REM Try port 16110
curl -s --max-time 5 http://127.0.0.1:16110/ >nul 2>&1
if %errorlevel% equ 0 (
    echo ✅ Kaspad is running on port 16110
    set KASPAD_URL=127.0.0.1:16110
    goto :start_explorer
)

REM Try port 16210
curl -s --max-time 5 http://127.0.0.1:16210/ >nul 2>&1
if %errorlevel% equ 0 (
    echo ✅ Kaspad is running on port 16210
    set KASPAD_URL=127.0.0.1:16210
    goto :start_explorer
)

REM Try port 16310
curl -s --max-time 5 http://127.0.0.1:16310/ >nul 2>&1
if %errorlevel% equ 0 (
    echo ✅ Kaspad is running on port 16310
    set KASPAD_URL=127.0.0.1:16310
    goto :start_explorer
)

echo ❌ Kaspad is not running on any standard port
echo Please start kaspad first:
echo cargo run --release --bin=kaspad -- --utxoindex --testnet --netsuffix=12 --enable-unsynced-mining --listen=0.0.0.0:16311 --addpeer=82.166.83.140 --appdir "D:\testnet12"
echo.
echo 📊 Peer 82.166.83.140 should be accessible for additional data
pause
exit /b 1

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
