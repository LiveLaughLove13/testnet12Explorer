use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, Json},
    routing::{get, Router},
};
use kaspa_grpc_client::GrpcClient;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_addresses::Address;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use clap::Parser;

#[derive(Clone)]
struct AppState {
    client: Arc<RwLock<Option<GrpcClient>>>,
    network_info: Arc<RwLock<NetworkInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NetworkInfo {
    server_url: String,
    network: String,
    is_connected: bool,
}

#[derive(Debug, Serialize)]
struct BlockInfo {
    hash: String,
    level: u64,
    parents: String,
    transactions: Vec<String>, // Simplified for now
    timestamp: i64,
    difficulty: f64,
}

#[derive(Debug, Serialize)]
struct TransactionInfo {
    id: String,
    inputs: Vec<String>,
    outputs: Vec<String>,
    amount: u64,
}

#[derive(Debug, Serialize)]
struct AddressBalance {
    address: String,
    balance: u64,
    utxos: Vec<UtxoInfo>,
}

#[derive(Debug, Serialize)]
struct UtxoInfo {
    outpoint: String,
    amount: u64,
    script_public_key: String,
}

#[derive(Debug, Serialize)]
struct MempoolInfo {
    size: usize,
    transactions: Vec<TransactionInfo>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let cli = Cli::parse();
    
    let network_info = NetworkInfo {
        server_url: cli.kaspad_url.clone(),
        network: "testnet-12".to_string(),
        is_connected: false,
    };

    let state = AppState {
        client: Arc::new(RwLock::new(None)),
        network_info: Arc::new(RwLock::new(network_info)),
    };

    // Connect to kaspad
    if let Err(e) = connect_to_kaspad(&state, &cli.kaspad_url).await {
        log::error!("Failed to connect to kaspad: {}", e);
    }

    // Create router
    let app = Router::new()
        .route("/", get(index))
        .route("/api/info", get(get_network_info))
        .route("/api/blocks", get(get_blocks))
        .route("/api/blocks/:hash", get(get_block))
        .route("/api/mempool", get(get_mempool))
        .route("/api/tx/:id", get(get_transaction))
        .route("/api/address/:address", get(get_address_balance))
        .nest_service("/static", ServeDir::new("static"))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], cli.port));
    log::info!("Starting explorer on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn connect_to_kaspad(state: &AppState, url: &str) -> anyhow::Result<()> {
    log::info!("Connecting to kaspad at: {}", url);
    
    // Always use grpc:// for gRPC connections
    let grpc_url = if url.starts_with("grpc://") {
        url.to_string()
    } else {
        format!("grpc://{}", url.replace("http://", "").replace("https://", ""))
    };
    
    log::info!("Using gRPC URL: {}", grpc_url);
    
    let client = GrpcClient::connect(grpc_url).await?;
    
    // Test connection
    let info = client.get_info().await?;
    log::info!("Connected to kaspad: {:?}", info);
    
    // Update state
    {
        let mut client_guard = state.client.write().await;
        *client_guard = Some(client);
    }
    
    {
        let mut network_info = state.network_info.write().await;
        network_info.is_connected = true;
    }
    
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn get_network_info(State(state): State<AppState>) -> Json<NetworkInfo> {
    let network_info = state.network_info.read().await;
    Json(network_info.clone())
}

async fn get_blocks(State(state): State<AppState>) -> Result<Json<Vec<BlockInfo>>, StatusCode> {
    let client_guard = state.client.read().await;
    let client = client_guard.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    // Limit to last 20 blocks to reduce data transfer
    let response = client.get_blocks(None, true, false).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let blocks: Vec<BlockInfo> = response.blocks.into_iter()
        .take(20) // Limit to 20 blocks
        .map(|block| BlockInfo {
            hash: block.header.hash.to_string(),
            level: block.header.daa_score,
            parents: format!("{} parents", block.header.parents_by_level.len()), // Simplified
            transactions: vec![], // Simplified for now
            timestamp: block.header.timestamp as i64,
            difficulty: block.header.bits as f64,
        })
        .collect();
    
    Ok(Json(blocks))
}

async fn get_block(
    State(state): State<AppState>,
    axum::extract::Path(hash): axum::extract::Path<String>,
) -> Result<Json<BlockInfo>, StatusCode> {
    let client_guard = state.client.read().await;
    let client = client_guard.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let hash = hash.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let block = client.get_block(hash, false).await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    
    let block_info = BlockInfo {
        hash: block.header.hash.to_string(),
        level: block.header.daa_score,
        parents: block.header.parents_by_level.iter().map(|p| format!("{:?}", p)).collect(),
        transactions: vec![], // Simplified for now
        timestamp: block.header.timestamp as i64,
        difficulty: block.header.bits as f64,
    };
    
    Ok(Json(block_info))
}

async fn get_mempool(State(state): State<AppState>) -> Result<Json<MempoolInfo>, StatusCode> {
    let client_guard = state.client.read().await;
    let client = client_guard.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    // Try to get mempool entries with different parameter combinations
    let response = match client.get_mempool_entries(false, true).await {
        Ok(response) => response,
        Err(e) => {
            log::error!("Failed to get mempool entries with (false, true): {:?}", e);
            // Try alternative parameters
            match client.get_mempool_entries(true, false).await {
                Ok(response) => response,
                Err(e2) => {
                    log::error!("Failed to get mempool entries with (true, false): {:?}", e2);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        }
    };
    
    // Limit to first 50 transactions to reduce lag
    let transactions: Vec<TransactionInfo> = response.into_iter()
        .take(50) // Limit to 50 transactions
        .enumerate()
        .map(|(i, entry)| TransactionInfo {
            id: format!("tx-{}", i),
            inputs: vec![format!("{} inputs", entry.transaction.inputs.len())], // Simplified
            outputs: vec![format!("{} outputs", entry.transaction.outputs.len())], // Simplified
            amount: entry.transaction.outputs.iter().map(|o| o.value).sum(),
        })
        .collect();
    
    let mempool_info = MempoolInfo {
        size: transactions.len(),
        transactions,
    };
    
    Ok(Json(mempool_info))
}

async fn get_transaction(
    State(_state): State<AppState>,
    axum::extract::Path(_id): axum::extract::Path<String>,
) -> Result<Json<TransactionInfo>, StatusCode> {
    // This is a placeholder - would need to implement transaction lookup
    Err(StatusCode::NOT_IMPLEMENTED)
}

async fn get_address_balance(
    State(state): State<AppState>,
    axum::extract::Path(address): axum::extract::Path<String>,
) -> Result<Json<AddressBalance>, StatusCode> {
    let client_guard = state.client.read().await;
    let client = client_guard.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    // Parse the address
    let parsed_address = Address::try_from(address.as_str())
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Get UTXOs for the address
    let utxos_response = client.get_utxos_by_addresses(vec![parsed_address]).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let mut total_balance = 0u64;
    // Limit UTXOs to first 100 to reduce lag
    let utxos: Vec<UtxoInfo> = utxos_response.into_iter()
        .take(100) // Limit to 100 UTXOs
        .map(|utxo| {
            let amount = utxo.utxo_entry.amount;
            total_balance += amount;
            UtxoInfo {
                outpoint: format!("{}:{}", utxo.outpoint.transaction_id, utxo.outpoint.index),
                amount,
                script_public_key: format!("script_{}", utxo.outpoint.index), // Simplified
            }
        })
        .collect();
    
    let address_balance = AddressBalance {
        address,
        balance: total_balance,
        utxos,
    };
    
    Ok(Json(address_balance))
}

#[derive(clap::Parser)]
#[command(name = "kaspa-testnet12-explorer")]
#[command(about = "Kaspa Testnet 12 Block Explorer - Standalone")]
struct Cli {
    /// Port to run the explorer on
    #[arg(short, long, default_value = "3000")]
    port: u16,
    
    /// Kaspad RPC server URL
    #[arg(short, long, default_value = "127.0.0.1:16110")]
    kaspad_url: String,
}
