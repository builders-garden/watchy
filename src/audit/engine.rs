use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::chains::{get_chain, ChainType};
use crate::types::{
    AgentMetadata, AuditReport, AuditRequest, CheckResult, Issue,
    RecommendedFieldsCheck, Severity, WatchyError,
};
use crate::AppState;

use super::consistency::{self, EndpointResponses};
use super::{content, endpoints, metadata, onchain, security};

pub struct AuditEngine {
    state: Arc<AppState>,
}

impl AuditEngine {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Run a full audit for an agent
    pub async fn run_audit(&self, request: &AuditRequest) -> Result<AuditReport, WatchyError> {
        // Resolve chain_id
        let chain_id = request.chain_id.unwrap_or(self.state.config.default_chain_id);

        // Get chain config
        let chain = get_chain(chain_id).ok_or_else(|| {
            WatchyError::InvalidRequest(format!("Unsupported chain_id: {}", chain_id))
        })?;

        // Validate chain type
        if chain.chain_type != ChainType::Evm {
            return Err(WatchyError::InvalidRequest(format!(
                "Chain {} is not an EVM chain",
                chain.name
            )));
        }

        // Get registry address
        let registry_address = chain.registry_address.ok_or_else(|| {
            WatchyError::InvalidRequest(format!(
                "No registry deployed on {} (chain_id: {})",
                chain.name, chain_id
            ))
        })?;

        let registry_full = format!("eip155:{}:{}", chain_id, registry_address);

        info!(
            "Starting audit for agent {} on {} ({})",
            request.agent_id, chain.name, registry_full
        );

        // Phase 1: Fetch on-chain data
        let onchain_data = onchain::fetch_onchain_data(
            chain_id,
            request.agent_id,
            registry_address,
        )
        .await?;

        // Get signer address if private key is configured
        let signer_address = self.get_signer_address();

        let mut report = AuditReport::new(
            request.agent_id,
            chain_id,
            registry_address,
            onchain_data.metadata_uri.clone(),
            signer_address.as_deref(),
        );
        report.block_number = onchain_data.block_number;
        report.agent.owner = Some(onchain_data.owner.clone());

        // Phase 2: Fetch off-chain metadata
        let metadata_result = metadata::fetch_metadata(
            &self.state.http_client,
            &onchain_data.metadata_uri,
        )
        .await;

        let agent_metadata = match metadata_result {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to fetch metadata: {}", e);
                report.checks.metadata.issues.push(Issue {
                    severity: Severity::Critical,
                    code: "METADATA_FETCH_FAILED".to_string(),
                    message: format!("Failed to fetch metadata: {}", e),
                });
                report.scores.metadata = 0;
                report.calculate_overall_score();
                return Ok(report);
            }
        };

        // Phase 3: Validate metadata
        self.validate_metadata(&mut report, &agent_metadata, request.agent_id, &registry_full);

        // Phase 4: Verify on-chain consistency
        self.verify_onchain(&mut report, &onchain_data);

        // Phase 5: Test endpoints and collect responses
        let endpoint_responses = self.test_endpoints(&mut report, &agent_metadata).await;

        // Phase 6: Security checks (on first HTTPS endpoint)
        self.run_security_checks(&mut report, &agent_metadata).await;

        // Phase 7: Consistency checks
        self.run_consistency_checks(&mut report, &agent_metadata, &endpoint_responses).await;

        // Phase 8: Content quality checks
        self.run_content_checks(&mut report, &agent_metadata).await;

        // Calculate final scores
        report.calculate_overall_score();

        info!(
            "Audit completed for agent {}. Overall score: {}",
            request.agent_id, report.scores.overall
        );

        Ok(report)
    }

    fn validate_metadata(
        &self,
        report: &mut AuditReport,
        metadata: &AgentMetadata,
        agent_id: u64,
        registry: &str,
    ) {
        let mut score: u8 = 100;
        let checks = &mut report.checks.metadata;

        // Check required fields
        checks.required_fields = CheckResult {
            passed: metadata.has_required_fields(),
            details: serde_json::json!({
                "type": metadata.metadata_type.is_some(),
                "name": metadata.name.is_some(),
                "description": metadata.description.is_some(),
                "image": metadata.image.is_some(),
                "registrations": !metadata.registrations.is_empty(),
            }),
        };

        if !checks.required_fields.passed {
            score = score.saturating_sub(40);
            checks.issues.push(Issue {
                severity: Severity::Critical,
                code: "MISSING_REQUIRED_FIELDS".to_string(),
                message: "One or more required fields are missing".to_string(),
            });
        }

        // Check type field
        checks.type_field = CheckResult {
            passed: metadata.has_valid_type(),
            details: serde_json::json!({
                "expected": crate::types::EIP8004_TYPE,
                "actual": metadata.metadata_type,
            }),
        };

        if !checks.type_field.passed {
            score = score.saturating_sub(20);
            checks.issues.push(Issue {
                severity: Severity::Critical,
                code: "INVALID_TYPE".to_string(),
                message: "Type field doesn't match EIP-8004 specification".to_string(),
            });
        }

        // Check registration matches
        if metadata.find_registration(agent_id, registry).is_none() {
            score = score.saturating_sub(20);
            checks.issues.push(Issue {
                severity: Severity::Critical,
                code: "REGISTRATION_MISMATCH".to_string(),
                message: format!(
                    "No registration found for agent {} in {}",
                    agent_id, registry
                ),
            });
        }

        // Check recommended fields
        let mut missing_recommended = vec![];
        if metadata.active.is_none() {
            missing_recommended.push("active".to_string());
        }
        if metadata.services.is_empty() {
            missing_recommended.push("services".to_string());
        }
        if metadata.supported_trust.is_empty() {
            missing_recommended.push("supportedTrust".to_string());
        }
        if metadata.updated_at.is_none() {
            missing_recommended.push("updatedAt".to_string());
        }

        checks.recommended_fields = RecommendedFieldsCheck {
            passed: missing_recommended.is_empty(),
            missing: missing_recommended.clone(),
        };

        if !missing_recommended.is_empty() {
            score = score.saturating_sub(10);
            for field in &missing_recommended {
                checks.issues.push(Issue {
                    severity: Severity::Warning,
                    code: format!("MISSING_{}", field.to_uppercase()),
                    message: format!("Recommended field '{}' is missing", field),
                });
            }
        }

        // URL validation (simplified - would do actual HTTP checks in production)
        checks.urls_valid = CheckResult {
            passed: true, // TODO: actual validation
            details: serde_json::Value::Null,
        };

        checks.passed = score >= 60;
        report.scores.metadata = score;
    }

    fn verify_onchain(&self, report: &mut AuditReport, onchain_data: &onchain::OnchainData) {
        let mut score: u8 = 100;
        let checks = &mut report.checks.onchain;

        checks.agent_exists = onchain_data.exists;
        if !checks.agent_exists {
            score = 0;
            checks.issues.push(Issue {
                severity: Severity::Critical,
                code: "AGENT_NOT_FOUND".to_string(),
                message: "Agent does not exist on-chain".to_string(),
            });
        }

        checks.uri_matches = true; // We fetched from on-chain URI, so it matches
        checks.wallet_set = onchain_data.wallet.is_some();

        if !checks.wallet_set {
            score = score.saturating_sub(20);
            checks.issues.push(Issue {
                severity: Severity::Warning,
                code: "NO_WALLET".to_string(),
                message: "Agent wallet is not set".to_string(),
            });
        }

        checks.passed = score >= 60;
        report.scores.onchain = score;
    }

    async fn test_endpoints(&self, report: &mut AuditReport, metadata: &AgentMetadata) -> EndpointResponses {
        let mut total_reachable = 0;
        let mut total_endpoints = 0;
        let mut total_latency_score = 0u64;

        // Collect endpoint responses for consistency checks
        let mut a2a_response: Option<serde_json::Value> = None;
        let mut mcp_response: Option<serde_json::Value> = None;
        let mut oasf_response: Option<serde_json::Value> = None;

        for service in &metadata.services {
            let Some(endpoint) = &service.endpoint else {
                continue;
            };

            // Skip non-HTTP endpoints
            if !endpoint.starts_with("http") {
                continue;
            }

            total_endpoints += 1;

            let (check, response) = endpoints::test_endpoint_with_response(
                &self.state.http_client,
                &service.name,
                endpoint,
                service,
            )
            .await;

            if check.reachable {
                total_reachable += 1;
            }

            // Calculate latency score
            if let Some(latency) = &check.latency {
                total_latency_score += latency_to_score(latency.p95);
            }

            // Store responses for consistency checks
            match service.name.to_lowercase().as_str() {
                "a2a" => a2a_response = response,
                "mcp" => mcp_response = response,
                "oasf" => oasf_response = response,
                _ => {}
            }

            report.checks.endpoints.push(check);
        }

        // Calculate availability score
        if total_endpoints > 0 {
            report.scores.endpoint_availability =
                ((total_reachable as f64 / total_endpoints as f64) * 100.0) as u8;

            // Calculate performance score (average latency score)
            if total_reachable > 0 {
                report.scores.endpoint_performance =
                    (total_latency_score / total_reachable as u64) as u8;
            }
        } else {
            // No testable endpoints
            report.scores.endpoint_availability = 100; // Not penalized
            report.scores.endpoint_performance = 100;
        }

        EndpointResponses::from_json_responses(
            a2a_response.as_ref(),
            mcp_response.as_ref(),
            oasf_response.as_ref(),
        )
    }

    async fn run_security_checks(&self, report: &mut AuditReport, metadata: &AgentMetadata) {
        debug!("Running security checks");

        // Find first HTTPS endpoint to test
        let test_endpoint = metadata.services.iter()
            .filter_map(|s| s.endpoint.as_ref())
            .find(|e| e.starts_with("https://"));

        if let Some(endpoint) = test_endpoint {
            let checks = security::check_endpoint_security(&self.state.http_client, endpoint).await;
            report.scores.security = security::calculate_security_score(&checks);
            report.checks.security = checks;
        } else {
            // No HTTPS endpoints - critical security issue
            report.scores.security = 0;
            report.checks.security.issues.push(Issue {
                severity: Severity::Critical,
                code: "NO_HTTPS_ENDPOINTS".to_string(),
                message: "No HTTPS endpoints found".to_string(),
            });
        }
    }

    async fn run_consistency_checks(
        &self,
        report: &mut AuditReport,
        metadata: &AgentMetadata,
        endpoint_responses: &EndpointResponses,
    ) {
        debug!("Running consistency checks");

        let checks = consistency::check_consistency(
            &self.state.http_client,
            metadata,
            endpoint_responses,
        )
        .await;

        report.scores.consistency = consistency::calculate_consistency_score(&checks);
        report.checks.consistency = checks;
    }

    async fn run_content_checks(&self, report: &mut AuditReport, metadata: &AgentMetadata) {
        debug!("Running content quality checks");

        let checks = content::check_content(&self.state.http_client, metadata).await;
        report.scores.content = content::calculate_content_score(&checks);
        report.checks.content = checks;
    }

    /// Get the signer address from the configured wallet
    fn get_signer_address(&self) -> Option<String> {
        self.state.config.signer_address().map(|s| s.to_string())
    }
}

fn latency_to_score(p95_ms: u64) -> u64 {
    match p95_ms {
        0..=200 => 100,
        201..=500 => 80,
        501..=1000 => 60,
        1001..=2000 => 40,
        2001..=5000 => 20,
        _ => 0,
    }
}
