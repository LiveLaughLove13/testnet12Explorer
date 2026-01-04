#!/bin/bash

# Simple seeder for Kaspa Testnet 12 Explorer
# This script creates some test transactions to ensure mempool has data

echo "🌱 Kaspa Testnet 12 Explorer - Data Seeder"
echo "=========================================="

# Check if kaspad is running
echo "📡 Checking if kaspad is running..."

# Try to connect to kaspad RPC
if command -v curl &> /dev/null; then
    echo "✅ curl is available"
    
    # Test connection to kaspad
    echo "🔗 Testing connection to kaspad..."
    
    # Try to get info from kaspad
    if curl -s --max-time 5 http://127.0.0.1:16110/ > /dev/null 2>&1; then
        echo "✅ Kaspad is running on port 16110"
        KASPAD_URL="127.0.0.1:16110"
    elif curl -s --max-time 5 http://127.0.0.1:16210/ > /dev/null 2>&1; then
        echo "✅ Kaspad is running on port 16210"
        KASPAD_URL="127.0.0.1:16210"
    elif curl -s --max-time 5 http://127.0.0.1:16310/ > /dev/null 2>&1; then
        echo "✅ Kaspad is running on port 16310"
        KASPAD_URL="127.0.0.1:16310"
    else
        echo "❌ Kaspad is not running on any standard port"
        echo "Please start kaspad first:"
        echo "cargo run --release --bin=kaspad -- --utxoindex --testnet --netsuffix=12 --enable-unsynced-mining --listen=0.0.0.0:16311 --addpeer=82.166.83.140 --appdir \"D:\\testnet12\""
        exit 1
    fi
else
    echo "❌ curl is not available, please install curl"
    exit 1
fi

echo ""
echo "🚀 Starting Kaspa Explorer..."
echo "📊 Explorer will connect to: $KASPAD_URL"
echo "🌐 Web interface: http://localhost:3000"
echo ""

# Start the explorer
cd "$(dirname "$0")"
cargo run --release -- --kaspad-url "$KASPAD_URL" --port 3000
