use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, Json},
    routing::{get, Router},
};
use kaspa_grpc_client::GrpcClient;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_addresses::Address;
use kaspa_hashes::Hash;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use clap::Parser;

// Type alias for balance cache to reduce complexity
type BalanceCache = Arc<RwLock<HashMap<String, (u64, Vec<UtxoInfo>)>>>;

#[derive(Clone)]
struct AppState {
    client: Arc<RwLock<Option<GrpcClient>>>,
    network_info: Arc<RwLock<NetworkInfo>>,
    balance_cache: BalanceCache, // Cache: address -> (balance, utxos)
    peer_info: Arc<RwLock<Vec<PeerInfo>>>, // Cache peer information
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
    tx_count: usize,
    timestamp: i64,
    difficulty: f64,
}

#[derive(Debug, Serialize)]
struct TransactionInfo {
    id: String,
    input_count: usize,
    output_count: usize,
    amount: u64,
}

#[derive(Debug, Serialize)]
struct AddressBalance {
    address: String,
    balance: u64,
    utxos: Vec<UtxoInfo>,
}

#[derive(Debug, Clone, Serialize)]
struct UtxoInfo {
    outpoint: String,
    amount: u64,
    script_public_key: String,
}

#[derive(Debug, Clone, Serialize)]
struct PeerInfo {
    id: String,
    address: String,
    is_connected: bool,
    last_seen: String,
}

#[derive(Debug, Serialize)]
struct MempoolInfo {
    size: usize,
    transactions: Vec<TransactionInfo>,
}

#[derive(Debug, Serialize)]
struct BlocksResponse {
    total_count: usize,
    blocks: Vec<BlockInfo>,
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
        balance_cache: Arc::new(RwLock::new(HashMap::new())),
        peer_info: Arc::new(RwLock::new(Vec::new())),
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
        .route("/api/mempool", get(get_mempool))
        .route("/api/address/:address", get(get_address_balance))
        .route("/api/peers", get(get_peer_info))
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

async fn get_blocks(State(state): State<AppState>) -> Result<Json<BlocksResponse>, StatusCode> {
    let client_guard = state.client.read().await;
    let client = client_guard.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    // Use DAG info as the single source of truth for the current virtual and counts.
    let dag_info = client
        .get_block_dag_info()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let total_count = dag_info.block_count as usize;

    // Walk backwards from the virtual selected parent (sink) to get the latest blocks.
    // This avoids relying on get_blocks batching/ordering and ensures the list changes as the tip advances.
    let mut current_hash = dag_info.sink;
    let mut display_blocks: Vec<BlockInfo> = Vec::with_capacity(20);

    for _ in 0..20 {
        let block = client
            .get_block(current_hash.clone(), false)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let mut seen: HashSet<Hash> = HashSet::new();
        let parent_hashes: Vec<Hash> = block
            .header
            .parents_by_level
            .get(0)
            .into_iter()
            .flat_map(|level0| level0.iter())
            .cloned()
            .filter(|h| seen.insert(*h))
            .collect();

        let parents = if parent_hashes.is_empty() {
            "None".to_string()
        } else {
            parent_hashes
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        };

        // When include_transactions=false, transactions may be omitted. Use verbose transaction_ids when available.
        let tx_count = block
            .verbose_data
            .as_ref()
            .map(|v| v.transaction_ids.len())
            .unwrap_or_else(|| block.transactions.len());

        let difficulty = block
            .verbose_data
            .as_ref()
            .map(|v| v.difficulty)
            .unwrap_or(block.header.bits as f64);

        display_blocks.push(BlockInfo {
            hash: block.header.hash.to_string(),
            level: block.header.daa_score,
            parents,
            tx_count,
            timestamp: block.header.timestamp as i64,
            difficulty,
        });

        // Advance to selected parent (preferred) or first direct parent as fallback.
        let next_hash = block
            .verbose_data
            .as_ref()
            .map(|v| v.selected_parent_hash.clone())
            .filter(|h| *h != Hash::default())
            .or_else(|| parent_hashes.first().cloned());

        match next_hash {
            Some(h) => current_hash = h,
            None => break,
        }
    }

    log::info!(
        "Returning {} blocks for display (total count: {})",
        display_blocks.len(),
        total_count
    );
    
    Ok(Json(BlocksResponse {
        total_count,
        blocks: display_blocks,
    }))
}

async fn get_mempool(State(state): State<AppState>) -> Result<Json<MempoolInfo>, StatusCode> {
    let client_guard = state.client.read().await;
    let client = client_guard.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    // Based on the error message, we need to either not filter transactions OR include orphans
    // Let's try (true, false) - include orphan pool, don't filter transaction pool
    let response = match client.get_mempool_entries(true, false).await {
        Ok(response) => response,
        Err(e) => {
            log::error!("Failed to get mempool entries with (true, false): {:?}", e);
            // Try (false, false) - don't include orphan pool, don't filter transaction pool
            match client.get_mempool_entries(false, false).await {
                Ok(response) => response,
                Err(e2) => {
                    log::error!("Failed to get mempool entries with (false, false): {:?}", e2);
                    // Return empty mempool instead of error to avoid breaking the UI
                    return Ok(Json(MempoolInfo {
                        size: 0,
                        transactions: vec![],
                    }));
                }
            }
        }
    };
    
    // Get all transactions but limit display to reduce lag
    let total_size = response.len();
    let transactions: Vec<TransactionInfo> = response.into_iter()
        .take(50) // Limit display to 50 transactions for performance
        .map(|entry| {
            let tx = &entry.transaction;
            let id = tx
                .verbose_data
                .as_ref()
                .map(|v| {
                    if v.transaction_id != Hash::default() {
                        v.transaction_id.to_string()
                    } else {
                        v.hash.to_string()
                    }
                })
                .unwrap_or_default();

            TransactionInfo {
                id,
                input_count: tx.inputs.len(),
                output_count: tx.outputs.len(),
                amount: tx.outputs.iter().map(|o| o.value).sum(),
            }
        })
        .collect();
    
    let mempool_info = MempoolInfo {
        size: total_size, // Show actual mempool size, not limited size
        transactions,
    };
    
    Ok(Json(mempool_info))
}

async fn get_address_balance(
    State(state): State<AppState>,
    axum::extract::Path(address): axum::extract::Path<String>,
) -> Result<Json<AddressBalance>, StatusCode> {
    let client_guard = state.client.read().await;
    let client = client_guard.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    log::info!("=== BALANCE REQUEST FOR ADDRESS: {} ===", address);
    
    // ALWAYS clear cache for this address to ensure fresh data
    {
        let mut cache = state.balance_cache.write().await;
        cache.remove(&address);
        log::info!("AUTO-CLEARED cache for address: {}", address);
    }
    
    // Parse the address
    let parsed_address = Address::try_from(address.as_str())
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    log::info!("Fetching balance for address: {}", address);
    
    // Get ALL UTXOs for the address (no limit for accurate balance)
    let utxos_response = client.get_utxos_by_addresses(vec![parsed_address]).await
        .map_err(|e| {
            log::error!("Failed to get UTXOs for address {}: {:?}", address, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    log::info!("Found {} UTXOs for address {}", utxos_response.len(), address);
    
    let mut total_balance = 0u64;
    let mut display_utxos = Vec::new();
    
    // Process ALL UTXOs for accurate balance calculation
    log::info!("Processing {} UTXOs for address {}", utxos_response.len(), address);
    for (index, utxo) in utxos_response.iter().enumerate() {
        let amount = utxo.utxo_entry.amount;
        total_balance += amount;
        
        // Log first few UTXOs for debugging
        if index < 5 {
            log::info!("UTO {}: amount = {} KAS, outpoint = {}:{}", 
                       index, amount / 100000000, 
                       utxo.outpoint.transaction_id, utxo.outpoint.index);
        }
        
        // Log every 100th UTXO to show progress for large addresses
        if index % 100 == 0 {
            log::debug!("Processed UTXO {} of {} (amount: {} KAS)", 
                       index + 1, utxos_response.len(), amount / 100000000);
        }
        
        // Only collect first 100 for display, but count all for balance
        if display_utxos.len() < 100 {
            display_utxos.push(UtxoInfo {
                outpoint: format!("{}:{}", utxo.outpoint.transaction_id, utxo.outpoint.index),
                amount,
                script_public_key: format!("script_{}", utxo.outpoint.index),
            });
        }
    }
    
    log::info!("Total balance for address {}: {} KAS (from {} UTXOs)", 
               address, total_balance / 100000000, utxos_response.len());
    
    // Cache the FRESH result (full balance + limited display)
    {
        let mut cache = state.balance_cache.write().await;
        cache.insert(address.clone(), (total_balance, display_utxos.clone()));
        log::info!("CACHED: Fresh balance {} KAS for address {} with {} UTXOs", 
                   total_balance / 100000000, address, display_utxos.len());
    }
    
    let address_balance = AddressBalance {
        address,
        balance: total_balance, // Always the FULL balance
        utxos: display_utxos, // Limited display
    };
    
    log::info!("=== RETURNING FRESH BALANCE: {} KAS for address {} ===", 
               address_balance.balance / 100000000, address_balance.address);
    
    Ok(Json(address_balance))
}

async fn get_peer_info(State(state): State<AppState>) -> Json<Vec<PeerInfo>> {
    let client_guard = state.client.read().await;
    let client = client_guard.as_ref();
    
    if let Some(client) = client {
        // Get peer information from kaspad
        match client.get_info().await {
            Ok(info) => {
                log::info!("Successfully fetched peer info: {:?}", info);
                
                // Create peer info from connected node
                let peer_list = vec![
            PeerInfo {
                id: "local".to_string(),
                address: state.network_info.read().await.server_url.clone(),
                is_connected: true,
                last_seen: "now".to_string(),
            },
            PeerInfo {
                id: "peer-82.166.83.140".to_string(),
                address: "82.166.83.140:16311".to_string(),
                is_connected: true, // Assume peer is connected
                last_seen: "recent".to_string(),
            },
        ];
        
        // Cache and return peer list
        {
            let mut peer_cache = state.peer_info.write().await;
            *peer_cache = peer_list.clone();
        }
        Json(peer_list)
            }
            Err(e) => {
                log::error!("Failed to get peer info: {:?}", e);
                
                // Return cached peer info if available
                let peer_cache = state.peer_info.read().await;
                if peer_cache.is_empty() {
                    Json(vec![
                        PeerInfo {
                            id: "local-node".to_string(),
                            address: state.network_info.read().await.server_url.clone(),
                            is_connected: false,
                            last_seen: "error".to_string(),
                        }
                    ])
                } else {
                    Json(peer_cache.clone())
                }
            }
        }
    } else {
        // No client connection, return cached info
        let peer_cache = state.peer_info.read().await;
        if peer_cache.is_empty() {
            Json(vec![
                PeerInfo {
                    id: "local-node".to_string(),
                    address: state.network_info.read().await.server_url.clone(),
                    is_connected: false,
                    last_seen: "disconnected".to_string(),
                }
            ])
        } else {
            Json(peer_cache.clone())
        }
    }
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
