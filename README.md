# Kaspa Testnet 12 Explorer - Standalone

A standalone web explorer for Kaspa Testnet 12 that can run independently without requiring the full rusty-kaspa repository.

## Features

- Real-time block explorer for Kaspa Testnet 12
- Address balance lookup with UTXO details
- Mempool monitoring
- Modern web interface with Tailwind CSS
- RESTful API endpoints
- Real-time auto-updating blocks and mempool
- Explorer is standalone, but requires access to a running kaspad node

## Prerequisites

- Rust 1.82.0 or higher
- A running kaspad node with gRPC enabled
- `curl` available on PATH (required for `seeder.bat` port detection on Windows)

## Running Kaspa Testnet 12

Start your Kaspa testnet 12 node with the following command:

```bash
cargo run --release --bin=kaspad -- \
  --utxoindex \
  --testnet \
  --netsuffix=12 \
  --enable-unsynced-mining \
  --listen=0.0.0.0:16311 \
  --addpeer=82.166.83.140 \
  --appdir "D:\testnet12"
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

3. **Run the explorer (recommended)**:
```bat
seeder.bat
```

`seeder.bat` will:

- Detect a running kaspad instance on common ports
- Start the explorer with the detected kaspad URL
- Print instructions for starting kaspad manually if it is not running

4. **Run the explorer (manual)**:
```bash
cargo run --release -- --kaspad-url 127.0.0.1:16210 --port 3000
```

## Configuration Options

- `--port`: Port to run the explorer web server on (default: 3000)
- `--kaspad-url`: Kaspad RPC server URL (default: 127.0.0.1:16110)

## API Endpoints

- `GET /api/info` - Network information and connection status
- `GET /api/blocks` - Latest blocks
- `GET /api/mempool` - Current mempool state
- `GET /api/address/:address` - Address balance and UTXO details
- `GET /api/peers` - Peer connection information

## Accessing the Explorer

Once running, open your web browser and navigate to:
- http://localhost:3000

The explorer will automatically connect to your Kaspa node and display:
- Network status and information
- Latest blocks with timestamps and difficulty
- Mempool size and pending transactions
- Address balance lookup with UTXO details
- Real-time updates (blocks and mempool)

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
