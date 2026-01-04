@echo off
echo Starting Kaspa Testnet 12 Explorer - Standalone Version
echo.
echo Make sure your Kaspa testnet 12 node is running with:
echo cargo run --release --bin=kaspad -- --utxoindex --testnet --netsuffix=12 --enable-unsynced-mining --listen=0.0.0.0:16311 --addpeer=82.166.83.140 --appdir "D:\testnet12"
echo.
echo Starting explorer on http://localhost:3000
echo.
cd D:\kaspa-testnet12-explorer
cargo run --release -- --kaspad-url 127.0.0.1:16210 --port 3000
pause
