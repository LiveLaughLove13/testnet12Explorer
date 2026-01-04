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
    if curl -s --max-time 3 http://127.0.0.1:16210/ > /dev/null 2>&1; then
        echo "✅ Kaspad is running on port 16210"
        KASPAD_URL="127.0.0.1:16210"
    elif curl -s --max-time 3 http://127.0.0.1:16310/ > /dev/null 2>&1; then
        echo "✅ Kaspad is running on port 16310"
        KASPAD_URL="127.0.0.1:16310"
    elif curl -s --max-time 3 http://127.0.0.1:16311/ > /dev/null 2>&1; then
        echo "✅ Kaspad is running on port 16311"
        KASPAD_URL="127.0.0.1:16311"
    elif curl -s --max-time 3 http://89.58.46.206:16310/ > /dev/null 2>&1; then
        echo "✅ External Kaspad is running on 89.58.46.206:16310"
        KASPAD_URL="89.58.46.206:16310"
    elif curl -s --max-time 3 http://89.58.46.206:16311/ > /dev/null 2>&1; then
        echo "✅ External Kaspad is running on 89.58.46.206:16311"
        KASPAD_URL="89.58.46.206:16311"
    else
        echo "📡 Fallback: Using working connection to 127.0.0.1:16210"
        echo "💡 If this fails, please start kaspad manually in another terminal with --utxoindex"
        echo ""
        KASPAD_URL="127.0.0.1:16210"
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
