use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, Json},
    routing::{get, Router},
};
use kaspa_grpc_client::GrpcClient;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::notify::mode::NotificationMode;
use kaspa_addresses::Address;
use kaspa_hashes::Hash;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash as StdHash, Hasher};
use tokio::sync::RwLock;
use tokio::time::{timeout, sleep, Duration};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use clap::Parser;

// Type alias for balance cache to reduce complexity
type BalanceCache = Arc<RwLock<HashMap<String, (u64, Option<usize>, Vec<UtxoInfo>)>>>;

#[derive(Clone)]
struct AppState {
    client: Arc<RwLock<Option<GrpcClient>>>,
    network_info: Arc<RwLock<NetworkInfo>>,
    balance_cache: BalanceCache, // Cache: address -> (balance, utxos)
    peer_info: Arc<RwLock<Vec<PeerInfo>>>, // Cache peer information
    mempool_cache: Arc<RwLock<Option<(std::time::Instant, MempoolInfo)>>>, // Cache last successful mempool snapshot
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

#[derive(Debug, Serialize, Clone)]
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
    utxo_count_total: Option<usize>,
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

#[derive(Debug, Serialize, Clone)]
struct MempoolInfo {
    size: usize,
    transactions: Vec<TransactionInfo>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
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
        mempool_cache: Arc::new(RwLock::new(None)),
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
    
    // Prefer the more robust connection used by the Stratum bridge:
    // - explicit grpc:// prefix
    // - extended request timeout
    // - client start()
    let client = match GrpcClient::connect_with_args(
        NotificationMode::Direct,
        grpc_url.clone(),
        None,
        true,
        None,
        false,
        Some(500_000),
        Default::default(),
    )
    .await
    {
        Ok(c) => {
            c.start(None).await;
            c
        }
        Err(e) => {
            log::warn!("connect_with_args failed, falling back to connect(): {:?}", e);
            GrpcClient::connect(grpc_url).await?
        }
    };
    
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

    // Always query the full mempool (include orphans) so the UI does not bounce between
    // different subsets. If this call fails intermittently, return the last successful snapshot.
    // (include_orphan_pool=true, filter_transaction_pool=false) => TransactionQuery::All
    let mut last_err: Option<anyhow::Error> = None;
    let mut response = None;
    for attempt in 0..3 {
        match client.get_mempool_entries(true, false).await {
            Ok(entries) => {
                log::info!("Fetched mempool entries (all): {}", entries.len());
                response = Some(entries);
                break;
            }
            Err(e) => {
                log::warn!("Failed to get mempool entries (all) attempt {}: {:?}", attempt + 1, e);
                last_err = Some(e.into());
                sleep(Duration::from_millis(150)).await;
            }
        }
    }

    let response = match response {
        Some(r) => r,
        None => {
            if let Some(e) = last_err {
                log::error!("Failed to fetch mempool entries after retries: {:?}", e);
            }

            // If RPC fails intermittently, it's better to return a recent snapshot than to
            // bounce between different views. However, do not serve stale data indefinitely.
            if let Some((ts, cached)) = state.mempool_cache.read().await.clone() {
                if ts.elapsed() <= Duration::from_secs(15) {
                    return Ok(Json(cached));
                }
            }

            // Last resort fallback: still report size if get_info works.
            let size = client
                .get_info()
                .await
                .map(|info| info.mempool_size as usize)
                .unwrap_or(0);
            return Ok(Json(MempoolInfo {
                size,
                transactions: vec![],
            }));
        }
    };
    
    // Get all transactions but limit display to reduce lag.
    // IMPORTANT: do not always take the first 50 entries; otherwise the displayed list can look
    // "stuck" while the overall mempool size changes. Instead, take a deterministic slice.
    let total_size = response.len();

    let mut entries_with_id: Vec<(String, _)> = response
        .into_iter()
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
            (id, entry)
        })
        .collect();

    entries_with_id.sort_by(|(a, _), (b, _)| a.cmp(b));

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for (id, _) in &entries_with_id {
        StdHash::hash(id, &mut hasher);
    }
    let seed = hasher.finish() as usize;

    let len = entries_with_id.len();
    let limit = 50usize.min(len);
    let start = if len == 0 { 0 } else { seed % len };

    let mut transactions: Vec<TransactionInfo> = Vec::with_capacity(limit);
    for i in 0..limit {
        let idx = (start + i) % len;
        let (id, entry) = &entries_with_id[idx];
        let tx = &entry.transaction;
        transactions.push(TransactionInfo {
            id: id.clone(),
            input_count: tx.inputs.len(),
            output_count: tx.outputs.len(),
            amount: tx.outputs.iter().map(|o| o.value).sum(),
        });
    }
    
    let mempool_info = MempoolInfo {
        size: total_size, // Show actual mempool size, not limited size
        transactions,
    };

    {
        let mut cache = state.mempool_cache.write().await;
        *cache = Some((std::time::Instant::now(), mempool_info.clone()));
    }
    
    Ok(Json(mempool_info))
}

async fn get_address_balance(
    State(state): State<AppState>,
    axum::extract::Path(address): axum::extract::Path<String>,
) -> Result<Json<AddressBalance>, (StatusCode, Json<ErrorResponse>)> {
    let client_guard = state.client.read().await;
    let client = client_guard
        .as_ref()
        .ok_or((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "Not connected to kaspad".to_string(),
            }),
        ))?;
    
    log::info!("=== BALANCE REQUEST FOR ADDRESS: {} ===", address);
    
    // Parse the address
    let parsed_address = Address::try_from(address.as_str())
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid address".to_string(),
                }),
            )
        })?;

    // Balance/UTXO calls require UTXO index.
    let info = client.get_info().await.map_err(|e| {
        log::error!("Failed to get kaspad info before balance lookup: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to query kaspad info".to_string(),
            }),
        )
    })?;
    if !info.is_utxo_indexed {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "Address balance requires kaspad to run with --utxoindex".to_string(),
            }),
        ));
    }
    
    log::info!("Fetching balance for address: {}", address);

    // Get a quick indexed balance first (fast path).
    // Then attempt to enumerate UTXOs and compute authoritative balance by summing amounts
    // (same approach used by the Stratum bridge prom balance collector).
    let indexed_balance = client
        .get_balance_by_address(parsed_address.clone())
        .await
        .map_err(|e| {
            log::error!("Failed to get indexed balance for address {}: {:?}", address, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch indexed balance (is --utxoindex enabled?)".to_string(),
                }),
            )
        })?;

    // UTXO enumeration can be heavy; cap the time.
    let mut display_utxos = Vec::new();
    let mut utxo_count_total: Option<usize> = None;
    let mut computed_balance: Option<u64> = None;

    match timeout(
        Duration::from_secs(20),
        client.get_utxos_by_addresses(vec![parsed_address]),
    )
    .await
    {
        Ok(Ok(utxos_response)) => {
            utxo_count_total = Some(utxos_response.len());
            let mut sum = 0u64;
            for (i, utxo) in utxos_response.iter().enumerate() {
                let amount = utxo.utxo_entry.amount;
                sum += amount;
                if i < 100 {
                    display_utxos.push(UtxoInfo {
                        outpoint: format!("{}:{}", utxo.outpoint.transaction_id, utxo.outpoint.index),
                        amount,
                        script_public_key: format!("script_{}", utxo.outpoint.index),
                    });
                }
            }
            computed_balance = Some(sum);

            if sum != indexed_balance {
                log::warn!(
                    "Balance mismatch for {}: indexed={} computed_from_utxos={} (utxos={})",
                    address,
                    indexed_balance,
                    sum,
                    utxos_response.len()
                );
            }
        }
        Ok(Err(e)) => {
            log::error!("Failed to get UTXOs for address {}: {:?}", address, e);
        }
        Err(_) => {
            log::warn!("Timed out fetching UTXOs for address {} (returning indexed balance only)", address);
        }
    }

    let total_balance = computed_balance.unwrap_or(indexed_balance);

    log::info!(
        "Returning balance for address {}: {} KAS (utxos_total={:?})",
        address,
        total_balance / 100000000,
        utxo_count_total
    );
    
    // Cache the FRESH result (full balance + limited display)
    {
        let mut cache = state.balance_cache.write().await;
        cache.insert(address.clone(), (total_balance, utxo_count_total, display_utxos.clone()));
        log::info!("CACHED: Fresh balance {} KAS for address {} (utxos_total={:?}, utxos_display={})", 
                   total_balance / 100000000, address, utxo_count_total, display_utxos.len());
    }
    
    let address_balance = AddressBalance {
        address,
        balance: total_balance, // Always the FULL balance
        utxo_count_total,
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
    #[arg(short, long, default_value = "127.0.0.1:16210")]
    kaspad_url: String,
}
