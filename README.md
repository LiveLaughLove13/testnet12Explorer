# Kaspa Testnet 12 Explorer - Standalone

A standalone web explorer for Kaspa Testnet 12 that can run independently without requiring the full rusty-kaspa repository.

## Features

- Real-time block explorer for Kaspa Testnet 12
- Address balance lookup with UTXO details
- Mempool monitoring
- Modern web interface with Tailwind CSS
- RESTful API endpoints
- Auto-refresh functionality
- Completely standalone - no need to clone rusty-kaspa repository

## Prerequisites

- Rust 1.82.0 or higher
- A running Kaspa node with RPC enabled

## Running Kaspa Testnet 12

Start your Kaspa testnet 12 node with the following command:

```bash
cargo run --release --bin=kaspad -- --utxoindex --testnet --netsuffix=12 --enable-unsynced-mining --listen=0.0.0.0:16311 --addpeer=82.166.83.140 --appdir "D:\testnet12"
```

## Installation and Usage

1. **Navigate to the standalone explorer directory**:
```bash
cd D:\kaspa-testnet12-explorer
```

2. **Build the explorer**:
```bash
cargo build --release
```

3. **Run the explorer**:
```bash
cargo run --release -- --kaspad-url 127.0.0.1:16210 --port 3000
```

## Configuration Options

- `--port`: Port to run the explorer web server on (default: 3000)
- `--kaspad-url`: Kaspad RPC server URL (default: 127.0.0.1:16110)

## API Endpoints

- `GET /api/info` - Network information and connection status
- `GET /api/blocks` - Latest blocks
- `GET /api/blocks/:hash` - Specific block details
- `GET /api/mempool` - Current mempool state
- `GET /api/address/:address` - Address balance and UTXO details
- `GET /api/tx/:id` - Transaction details (placeholder)

## Accessing the Explorer

Once running, open your web browser and navigate to:
- http://localhost:3000

The explorer will automatically connect to your Kaspa node and display:
- Network status and information
- Latest blocks with timestamps and difficulty
- Mempool size and pending transactions
- Address balance lookup with UTXO details
- Real-time updates every 30 seconds

## Standalone Features

This version is completely independent and:

- ✅ **No rusty-kaspa repository required**
- ✅ **Uses Git dependencies** for Kaspa libraries
- ✅ **Self-contained project structure**
- ✅ **Easy to deploy and share**
- ✅ **Same functionality as the integrated version**

## Project Structure

```
kaspa-testnet12-explorer/
├── Cargo.toml              # Project configuration with Git dependencies
├── src/
│   └── main.rs             # Main application code
├── static/
│   └── index.html          # Web frontend
└── README.md               # This file
```

## Development

The explorer connects to any Kaspa node via RPC and provides a clean, modern interface for exploring the Kaspa blockchain on testnet 12. The standalone version uses Git dependencies to pull the required Kaspa libraries, making it completely independent of the main repository.
