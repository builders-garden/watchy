use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

use crate::arweave::{irys::sign_report, IrysClient};
use crate::audit::{generate_markdown_report, metadata, AuditEngine};
use crate::blockchain::registry::RegistryClient;
use crate::blockchain::reputation::ReputationClient;
use crate::chains::{get_chain, get_rpc_url, supported_chain_ids, ChainType};
use crate::ipfs::IpfsClient;
use crate::store::AuditJob;
use crate::types::{AuditRequest, AuditStatus, WatchyError};
use crate::AppState;

// =============================================================================
// TESTNET-ONLY MODE
// =============================================================================
// Set to `false` to enable mainnet audits (Base: 8453, Ethereum: 1)
// Currently restricted to testnets to prevent accidental mainnet transactions
const TESTNET_ONLY: bool = true;

/// Chain IDs allowed when TESTNET_ONLY is true
const ALLOWED_TESTNET_CHAINS: &[u64] = &[
    84532,    // Base Sepolia
    11155111, // Sepolia
];

/// Check if a chain is allowed for audits
fn is_chain_allowed(chain_id: u64) -> bool {
    if TESTNET_ONLY {
        ALLOWED_TESTNET_CHAINS.contains(&chain_id)
    } else {
        true // All configured chains are allowed
    }
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub supported_chains: Vec<u64>,
    pub default_chain: u64,
    pub storage: String,
    pub wallet_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer_address: Option<String>,
}

/// GET /health
pub async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        supported_chains: supported_chain_ids(),
        default_chain: state.config.default_chain_id,
        storage: if state.audit_store.has_redis() {
            "redis".to_string()
        } else {
            "memory".to_string()
        },
        wallet_mode: state.config.key_mode().as_str().to_string(),
        signer_address: state.config.signer_address().map(|s| s.to_string()),
    })
}

#[derive(Serialize)]
pub struct AuditCreatedResponse {
    pub audit_id: String,
    pub chain_id: u64,
    pub chain_name: String,
    pub status: AuditStatus,
    pub created_at: u64,
    pub estimated_completion: u64,
}

/// POST /audit
pub async fn request_audit(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AuditRequest>,
) -> Result<(StatusCode, Json<AuditCreatedResponse>), WatchyError> {
    // Validate agent_id
    if request.agent_id == 0 {
        return Err(WatchyError::InvalidRequest(
            "agent_id must be greater than 0".to_string(),
        ));
    }

    // Resolve chain_id (use request or default)
    let chain_id = request.chain_id.unwrap_or(state.config.default_chain_id);

    // Look up chain config
    let chain = get_chain(chain_id).ok_or_else(|| {
        WatchyError::InvalidRequest(format!(
            "Unsupported chain_id: {}. Supported: {:?}",
            chain_id,
            supported_chain_ids()
        ))
    })?;

    // Check if chain type is supported
    if chain.chain_type != ChainType::Evm {
        return Err(WatchyError::InvalidRequest(format!(
            "Chain {} ({}) is not yet supported for audits. Only EVM chains are supported.",
            chain.name, chain_id
        )));
    }

    // Check if chain is allowed (testnet-only mode)
    if !is_chain_allowed(chain_id) {
        return Err(WatchyError::InvalidRequest(format!(
            "Chain {} ({}) is not enabled. Currently only testnets are allowed: Base Sepolia (84532), Sepolia (11155111)",
            chain.name, chain_id
        )));
    }

    // Check if registry is deployed on this chain
    if chain.registry_address.is_none() {
        return Err(WatchyError::InvalidRequest(format!(
            "EIP-8004 registry not yet deployed on {} (chain_id: {})",
            chain.name, chain_id
        )));
    }

    info!(
        "Audit requested for agent {} on {} (chain_id: {}, registry: {})",
        request.agent_id,
        chain.name,
        chain_id,
        chain.registry_address.unwrap()
    );

    // Create job in store
    let audit_id = state.audit_store.create_job(request.agent_id, chain_id).await;
    let now = chrono::Utc::now().timestamp() as u64;

    info!("Created audit job: {}", audit_id);

    // Spawn background task to run the audit
    let state_clone = state.clone();
    let audit_id_clone = audit_id.clone();
    let agent_id = request.agent_id;

    tokio::spawn(async move {
        run_audit_job(state_clone, audit_id_clone, agent_id, chain_id).await;
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(AuditCreatedResponse {
            audit_id,
            chain_id,
            chain_name: chain.name.to_string(),
            status: AuditStatus::Pending,
            created_at: now,
            estimated_completion: now + 30, // ~30 seconds estimate
        }),
    ))
}

/// Background job runner for audits
///
/// Flow (Option A):
/// 1. Run audit → get report
/// 2. Generate markdown report
/// 3. Upload MD to Arweave → get md_arweave_url
/// 4. Add MD URL to report (report_markdown_url)
/// 5. Sign the JSON report
/// 6. Upload JSON to Arweave → get json_arweave_url
/// 7. Submit on-chain feedback with json_arweave_url as feedbackURI
async fn run_audit_job(state: Arc<AppState>, audit_id: String, agent_id: u64, chain_id: u64) {
    info!(
        "Starting audit job {} for agent {} on chain {}",
        audit_id, agent_id, chain_id
    );

    // Update status to in_progress
    state
        .audit_store
        .update_status(&audit_id, AuditStatus::InProgress)
        .await;

    // Create audit engine and request
    let engine = AuditEngine::new(state.clone());
    let request = AuditRequest {
        agent_id,
        chain_id: Some(chain_id),
    };

    // Run the audit
    match engine.run_audit(&request).await {
        Ok(mut report) => {
            info!(
                "Audit {} completed. Overall score: {}",
                audit_id, report.scores.overall
            );

            // Fetch metadata for the report (we need the name)
            let agent_metadata = metadata::fetch_metadata(
                &state.http_client,
                &report.agent.metadata_uri,
            )
            .await
            .ok();

            // Upload to Arweave and submit on-chain feedback (if private key is configured)
            if let Some(private_key) = state.config.private_key() {
                match IrysClient::new(Some(private_key)) {
                    Ok(irys) => {
                        let md_filename = format!("watchy-audit-{}-{}.md", agent_id, audit_id);
                        let json_filename = format!("watchy-audit-{}-{}.json", agent_id, audit_id);

                        // Step 1: Generate and upload Markdown FIRST
                        let markdown = generate_markdown_report(&report, agent_metadata.as_ref());
                        match irys.upload_markdown(&markdown, &md_filename).await {
                            Ok(md_result) => {
                                info!("Markdown uploaded to Arweave: {}", md_result.arweave_url);
                                // Step 2: Add MD URL to report
                                report.set_markdown_url(&md_result.arweave_url);
                            }
                            Err(e) => {
                                error!("Failed to upload MD to Irys: {}", e);
                            }
                        }

                        // Step 3: Serialize report to JSON (now includes MD URL)
                        match serde_json::to_value(&report) {
                            Ok(mut report_json) => {
                                // Step 4: Sign the report
                                match sign_report(&report_json, private_key).await {
                                    Ok(signature) => {
                                        info!(
                                            "Report signed: {}...{}",
                                            &signature[..10],
                                            &signature[signature.len() - 8..]
                                        );

                                        // Add signature to JSON
                                        if let Some(obj) = report_json.as_object_mut() {
                                            obj.insert(
                                                "signature".to_string(),
                                                serde_json::json!(signature),
                                            );
                                        }

                                        // Step 5: Upload signed JSON to Arweave
                                        match irys.upload_json(&report_json, &json_filename).await {
                                            Ok(json_result) => {
                                                info!(
                                                    "JSON report uploaded to Arweave: {}",
                                                    json_result.arweave_url
                                                );
                                                report.set_json_url(&json_result.arweave_url);

                                                // Step 6: Submit on-chain feedback
                                                // IMPORTANT: Use report_json (the uploaded JSON) for hash computation
                                                // to ensure feedbackHash matches the content at feedbackURI
                                                let chain = get_chain(chain_id);
                                                let rpc_url = get_rpc_url(chain_id);

                                                if let (Some(chain), Some(rpc), Some(rep_addr)) =
                                                    (chain, rpc_url, chain.and_then(|c| c.reputation_address))
                                                {
                                                    info!(
                                                        "Submitting on-chain feedback to {} ({})",
                                                        chain.name, rep_addr
                                                    );

                                                    match ReputationClient::new(&rpc, rep_addr, Some(private_key)) {
                                                        Ok(rep_client) => {
                                                            let endpoint = report.endpoint.as_deref();

                                                            match rep_client
                                                                .submit_feedback(
                                                                    agent_id,
                                                                    report.scores.overall,
                                                                    "starred",
                                                                    "",
                                                                    endpoint,
                                                                    &json_result.arweave_url,
                                                                    &report_json, // Use the exact JSON that was uploaded
                                                                )
                                                                .await
                                                            {
                                                                Ok(tx_hash) => {
                                                                    info!(
                                                                        "On-chain feedback submitted: {} (tx: {})",
                                                                        json_result.arweave_url, tx_hash
                                                                    );
                                                                    report.set_feedback_tx(chain_id, &tx_hash);
                                                                }
                                                                Err(e) => {
                                                                    error!("Failed to submit on-chain feedback: {}", e);
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            error!("Failed to create reputation client: {}", e);
                                                        }
                                                    }
                                                } else {
                                                    info!(
                                                        "No reputation registry on chain {}, skipping on-chain feedback",
                                                        chain_id
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                error!("Failed to upload JSON to Irys: {}", e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to sign report: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to serialize report: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to create Irys client: {}", e);
                    }
                }
            } else {
                info!("No private key configured, skipping Arweave upload and on-chain feedback");
            }

            // Optional IPFS upload (legacy, if configured separately)
            if let Some(ref api_key) = state.config.ipfs_api_key {
                let ipfs_client =
                    IpfsClient::new(state.config.ipfs_api_url.clone(), Some(api_key.clone()));

                match serde_json::to_value(&report) {
                    Ok(report_json) => {
                        let filename = format!("watchy-audit-{}.json", audit_id);
                        match ipfs_client.upload_json(&report_json, &filename).await {
                            Ok(cid) => {
                                info!("Audit report uploaded to IPFS: {}", cid);
                            }
                            Err(e) => {
                                error!("Failed to upload to IPFS: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to serialize report for IPFS: {}", e);
                    }
                }
            }

            // Store result
            state.audit_store.set_result(&audit_id, report).await;
        }
        Err(e) => {
            error!("Audit {} failed: {}", audit_id, e);
            state.audit_store.set_error(&audit_id, e.to_string()).await;
        }
    }
}

/// Response for GET /audit/:id
#[derive(Serialize)]
pub struct AuditStatusResponse {
    pub audit_id: String,
    pub agent_id: u64,
    pub status: AuditStatus,
    pub created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<AuditResultSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct AuditResultSummary {
    pub scores: ScoresSummary,
    pub issues_count: IssuesCount,
}

#[derive(Serialize)]
pub struct ScoresSummary {
    pub overall: u8,
    pub metadata: u8,
    pub onchain: u8,
    pub endpoint_availability: u8,
    pub endpoint_performance: u8,
}

#[derive(Serialize)]
pub struct IssuesCount {
    pub critical: u32,
    pub error: u32,
    pub warning: u32,
    pub info: u32,
}

impl From<&AuditJob> for AuditStatusResponse {
    fn from(job: &AuditJob) -> Self {
        let result = job.result.as_ref().map(|r| {
            let issues = r.count_issues();
            AuditResultSummary {
                scores: ScoresSummary {
                    overall: r.scores.overall,
                    metadata: r.scores.metadata,
                    onchain: r.scores.onchain,
                    endpoint_availability: r.scores.endpoint_availability,
                    endpoint_performance: r.scores.endpoint_performance,
                },
                issues_count: IssuesCount {
                    critical: issues.critical,
                    error: issues.error,
                    warning: issues.warning,
                    info: issues.info,
                },
            }
        });

        Self {
            audit_id: job.id.clone(),
            agent_id: job.agent_id,
            status: job.status.clone(),
            created_at: job.created_at,
            completed_at: job.completed_at,
            result,
            error: job.error.clone(),
        }
    }
}

/// GET /audit/:audit_id
pub async fn get_audit(
    State(state): State<Arc<AppState>>,
    Path(audit_id): Path<String>,
) -> Result<Json<AuditStatusResponse>, WatchyError> {
    info!("Getting audit {}", audit_id);

    match state.audit_store.get_job(&audit_id).await {
        Some(job) => Ok(Json(AuditStatusResponse::from(&job))),
        None => Err(WatchyError::AuditNotFound(audit_id)),
    }
}

/// GET /audit/:audit_id/report
pub async fn get_audit_report(
    State(state): State<Arc<AppState>>,
    Path(audit_id): Path<String>,
) -> Result<Json<serde_json::Value>, WatchyError> {
    info!("Getting audit report for {}", audit_id);

    match state.audit_store.get_job(&audit_id).await {
        Some(job) => {
            if let Some(report) = job.result {
                Ok(Json(serde_json::to_value(report).unwrap_or_default()))
            } else if job.status == AuditStatus::Failed {
                Err(WatchyError::Internal(format!(
                    "Audit failed: {}",
                    job.error.unwrap_or_default()
                )))
            } else {
                // Still in progress
                Err(WatchyError::InvalidRequest(
                    "Audit not yet completed".to_string(),
                ))
            }
        }
        None => Err(WatchyError::AuditNotFound(audit_id)),
    }
}

#[derive(Deserialize)]
pub struct ListAuditsQuery {
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_limit() -> u32 {
    10
}

/// GET /agents/:registry/:agent_id/audits
pub async fn list_agent_audits(
    State(_state): State<Arc<AppState>>,
    Path((registry, agent_id)): Path<(String, u64)>,
    Query(query): Query<ListAuditsQuery>,
) -> Result<Json<serde_json::Value>, WatchyError> {
    info!(
        "Listing audits for agent {} on {} (limit={}, offset={})",
        agent_id, registry, query.limit, query.offset
    );

    // TODO: Implement listing from store (filter by agent_id)
    Ok(Json(serde_json::json!({
        "agent_id": agent_id,
        "registry": registry,
        "audits": [],
        "total": 0,
        "limit": query.limit,
        "offset": query.offset
    })))
}

// =============================================================================
// ADMIN ENDPOINTS (protected by ADMIN_API_KEY)
// =============================================================================

/// Request body for registering a new agent
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterAgentRequest {
    /// Chain ID to register on (default: config default_chain_id)
    pub chain_id: Option<u64>,
}

/// Response for agent registration
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterAgentResponse {
    pub agent_id: u64,
    pub chain_id: u64,
    pub chain_name: String,
    pub registry: String,
    pub tx_hash: String,
    pub owner: String,
}

/// POST /admin/register - Register a new EIP-8004 agent
///
/// Mints a new agent NFT with empty URI. Use /admin/set-uri to set the metadata.
/// Uses the TEE wallet (derived from mnemonic) to sign the transaction.
pub async fn register_agent(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RegisterAgentRequest>,
) -> Result<(StatusCode, Json<RegisterAgentResponse>), WatchyError> {
    let chain_id = request.chain_id.unwrap_or(state.config.default_chain_id);

    // Check if chain is allowed (testnet-only mode)
    if !is_chain_allowed(chain_id) {
        return Err(WatchyError::InvalidRequest(format!(
            "Chain {} is not enabled. Currently only testnets are allowed.",
            chain_id
        )));
    }

    // Get chain config
    let chain = get_chain(chain_id).ok_or_else(|| {
        WatchyError::InvalidRequest(format!("Unsupported chain_id: {}", chain_id))
    })?;

    let registry_address = chain.registry_address.ok_or_else(|| {
        WatchyError::InvalidRequest(format!(
            "No registry deployed on {} (chain_id: {})",
            chain.name, chain_id
        ))
    })?;

    let rpc_url = get_rpc_url(chain_id).ok_or_else(|| {
        WatchyError::InvalidRequest(format!("No RPC URL for chain {}", chain_id))
    })?;

    // Get the TEE wallet private key
    let private_key = state.config.private_key().ok_or_else(|| {
        WatchyError::Internal("No wallet configured (MNEMONIC or PRIVATE_KEY required)".to_string())
    })?;

    let signer_address = state.config.signer_address().ok_or_else(|| {
        WatchyError::Internal("Could not derive signer address".to_string())
    })?;

    info!(
        "Registering new agent on {} ({}) with signer {}",
        chain.name, chain_id, signer_address
    );

    // Create registry client and register
    let registry = RegistryClient::new(&rpc_url, registry_address)?;
    let (agent_id, tx_hash) = registry.register_agent(private_key).await?;

    info!(
        "Agent {} registered on {} (tx: {})",
        agent_id, chain.name, tx_hash
    );

    Ok((
        StatusCode::CREATED,
        Json(RegisterAgentResponse {
            agent_id,
            chain_id,
            chain_name: chain.name.to_string(),
            registry: registry_address.to_string(),
            tx_hash,
            owner: signer_address.to_string(),
        }),
    ))
}

/// Request body for updating an agent's URI
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentUriRequest {
    /// The agent token ID to update
    pub agent_id: u64,
    /// The URI to set (e.g., "data:application/json;base64,..." or IPFS/Arweave URL)
    pub uri: String,
    /// Chain ID (default: config default_chain_id)
    pub chain_id: Option<u64>,
}

/// Response for URI update
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentUriResponse {
    pub agent_id: u64,
    pub chain_id: u64,
    pub chain_name: String,
    pub tx_hash: String,
    pub uri: String,
}

/// POST /admin/set-uri - Update an agent's metadata URI
///
/// Updates the metadata URI for an existing agent. The caller must be the owner
/// or an approved operator of the agent. Uses TEE wallet for signing.
///
/// The URI can be:
/// - A base64 data URI: "data:application/json;base64,eyJ0eXBlIjoi..."
/// - An IPFS URL: "ipfs://Qm..."
/// - An Arweave URL: "https://arweave.net/..."
pub async fn set_agent_uri(
    State(state): State<Arc<AppState>>,
    Json(request): Json<UpdateAgentUriRequest>,
) -> Result<Json<UpdateAgentUriResponse>, WatchyError> {
    let chain_id = request.chain_id.unwrap_or(state.config.default_chain_id);

    // Check if chain is allowed
    if !is_chain_allowed(chain_id) {
        return Err(WatchyError::InvalidRequest(format!(
            "Chain {} is not enabled. Currently only testnets are allowed.",
            chain_id
        )));
    }

    // Get chain config
    let chain = get_chain(chain_id).ok_or_else(|| {
        WatchyError::InvalidRequest(format!("Unsupported chain_id: {}", chain_id))
    })?;

    let registry_address = chain.registry_address.ok_or_else(|| {
        WatchyError::InvalidRequest(format!(
            "No registry deployed on {} (chain_id: {})",
            chain.name, chain_id
        ))
    })?;

    let rpc_url = get_rpc_url(chain_id).ok_or_else(|| {
        WatchyError::InvalidRequest(format!("No RPC URL for chain {}", chain_id))
    })?;

    // Get the TEE wallet private key
    let private_key = state.config.private_key().ok_or_else(|| {
        WatchyError::Internal("No wallet configured (MNEMONIC or PRIVATE_KEY required)".to_string())
    })?;

    info!(
        "Updating URI for agent {} on {} ({}) - URI length: {} bytes",
        request.agent_id, chain.name, chain_id, request.uri.len()
    );

    // Create registry client and update URI
    let registry = RegistryClient::new(&rpc_url, registry_address)?;
    let tx_hash = registry
        .set_agent_uri(request.agent_id, &request.uri, private_key)
        .await?;

    info!(
        "Agent {} URI updated on {} (tx: {})",
        request.agent_id, chain.name, tx_hash
    );

    Ok(Json(UpdateAgentUriResponse {
        agent_id: request.agent_id,
        chain_id,
        chain_name: chain.name.to_string(),
        tx_hash,
        uri: request.uri,
    }))
}
