use serde::{Deserialize, Serialize};

/// Audit request from API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRequest {
    pub agent_id: u64,
    /// Chain ID (e.g., 8453 for Base, 1 for Ethereum)
    /// If not provided, uses default chain from config
    pub chain_id: Option<u64>,
}

/// Audit status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

/// Full audit report (uploaded to Arweave)
/// This also serves as the off-chain feedback file per EIP-8004 Reputation spec
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditReport {
    // ===== FEEDBACK REQUIRED FIELDS (EIP-8004 Reputation) =====
    /// Registry address in CAIP-10 format (e.g., "eip155:8453:0x8004...")
    pub agent_registry: String,
    /// Agent token ID
    pub agent_id: u64,
    /// Watchy's address in CAIP-10 format
    pub client_address: String,
    /// ISO 8601 timestamp
    pub created_at: String,
    /// Feedback value (overall score 0-100)
    pub value: i128,
    /// Decimal places for value (0 for integer scores)
    pub value_decimals: u8,

    // ===== FEEDBACK OPTIONAL FIELDS =====
    /// Feedback tag1 (e.g., "auditScore")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag1: Option<String>,
    /// Feedback tag2 (e.g., "infrastructure")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag2: Option<String>,
    /// Primary endpoint tested
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,

    // ===== AUDIT REPORT FIELDS =====
    pub version: String,
    pub auditor: AuditorInfo,
    pub timestamp: u64,
    pub block_number: u64,
    pub agent: AgentInfo,
    pub scores: Scores,
    pub checks: Checks,

    // ===== LINKS =====
    /// URL to markdown report on Arweave
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_markdown_url: Option<String>,
    /// URL to JSON report on Arweave
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_json_url: Option<String>,
    /// Signature of the report
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,

    // ===== ON-CHAIN FEEDBACK =====
    /// Chain ID where feedback was submitted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback_chain_id: Option<u64>,
    /// Transaction hash of the feedback submission
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback_tx_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditorInfo {
    pub name: String,
    pub address: Option<String>,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_id: u64,
    pub registry: String,
    pub metadata_uri: String,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scores {
    pub overall: u8,
    pub metadata: u8,
    pub onchain: u8,
    pub endpoint_availability: u8,
    pub endpoint_performance: u8,
    #[serde(default)]
    pub security: u8,
    #[serde(default)]
    pub consistency: u8,
    #[serde(default)]
    pub content: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checks {
    pub metadata: MetadataChecks,
    pub onchain: OnchainChecks,
    pub endpoints: Vec<EndpointCheck>,
    #[serde(default)]
    pub security: SecurityChecks,
    #[serde(default)]
    pub consistency: ConsistencyChecks,
    #[serde(default)]
    pub content: ContentChecks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataChecks {
    pub passed: bool,
    pub required_fields: CheckResult,
    pub type_field: CheckResult,
    pub urls_valid: CheckResult,
    pub recommended_fields: RecommendedFieldsCheck,
    #[serde(default)]
    pub issues: Vec<Issue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnchainChecks {
    pub passed: bool,
    pub agent_exists: bool,
    pub uri_matches: bool,
    pub wallet_set: bool,
    #[serde(default)]
    pub issues: Vec<Issue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointCheck {
    pub service: String,
    pub endpoint: String,
    pub reachable: bool,
    pub valid_schema: Option<bool>,
    pub skills_match: Option<bool>,
    pub latency: Option<LatencyMetrics>,
    pub error: Option<String>,
    #[serde(default)]
    pub issues: Vec<Issue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyMetrics {
    pub p50: u64,
    pub p95: u64,
    pub p99: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub passed: bool,
    #[serde(default)]
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedFieldsCheck {
    pub passed: bool,
    #[serde(default)]
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub severity: Severity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    Error,
    Warning,
    Info,
}

/// Security checks for endpoints
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityChecks {
    pub passed: bool,
    pub tls_valid: bool,
    pub tls_version: Option<String>,
    pub certificate_valid: bool,
    pub certificate_days_remaining: Option<i64>,
    pub security_headers: SecurityHeadersCheck,
    pub https_enforced: bool,
    #[serde(default)]
    pub issues: Vec<Issue>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityHeadersCheck {
    pub x_content_type_options: bool,
    pub x_frame_options: bool,
    pub strict_transport_security: bool,
    pub content_security_policy: bool,
    pub x_xss_protection: bool,
}

/// Consistency checks across metadata and endpoints
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConsistencyChecks {
    pub passed: bool,
    pub name_consistent: bool,
    pub skills_consistent: bool,
    pub version_consistent: bool,
    pub image_accessible: bool,
    #[serde(default)]
    pub issues: Vec<Issue>,
}

/// Content quality checks
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContentChecks {
    pub passed: bool,
    pub description_quality: DescriptionQuality,
    pub valid_skill_taxonomy: bool,
    pub has_contact_info: bool,
    pub x402_valid: Option<X402Check>,
    #[serde(default)]
    pub issues: Vec<Issue>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DescriptionQuality {
    pub score: u8,
    pub length: usize,
    pub has_placeholder: bool,
    pub is_meaningful: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct X402Check {
    pub valid: bool,
    pub returns_402: bool,
    pub has_payment_address: bool,
    pub has_payment_amount: bool,
    pub has_payment_network: bool,
    pub payment_address: Option<String>,
    pub payment_amount: Option<String>,
    pub payment_network: Option<String>,
    pub error: Option<String>,
}

/// API response for audit status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStatusResponse {
    pub audit_id: String,
    pub status: AuditStatus,
    pub agent_id: u64,
    pub registry: String,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    pub failed_at: Option<u64>,
    pub progress: Option<AuditProgress>,
    pub result: Option<AuditResult>,
    pub error: Option<AuditError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditProgress {
    pub phase: String,
    pub completed_steps: u8,
    pub total_steps: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResult {
    pub scores: Scores,
    pub issues_count: IssueCount,
    pub ipfs_cid: String,
    pub report_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueCount {
    pub critical: u32,
    pub error: u32,
    pub warning: u32,
    pub info: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditError {
    pub code: String,
    pub message: String,
}

impl AuditReport {
    /// Create a new audit report
    ///
    /// # Arguments
    /// * `agent_id` - The agent token ID
    /// * `chain_id` - The chain ID (e.g., 8453 for Base)
    /// * `registry_address` - The registry contract address
    /// * `metadata_uri` - The agent's metadata URI
    /// * `client_address` - Watchy's signer address (optional)
    pub fn new(
        agent_id: u64,
        chain_id: u64,
        registry_address: &str,
        metadata_uri: String,
        client_address: Option<&str>,
    ) -> Self {
        let now = chrono::Utc::now();
        let registry_full = format!("eip155:{}:{}", chain_id, registry_address);
        let client_full = client_address
            .map(|addr| format!("eip155:{}:{}", chain_id, addr))
            .unwrap_or_default();

        Self {
            // Feedback required fields
            agent_registry: registry_full.clone(),
            agent_id,
            client_address: client_full,
            created_at: now.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            value: 0, // Will be set after scoring
            value_decimals: 0,

            // Feedback optional fields
            tag1: Some("auditScore".to_string()),
            tag2: Some("infrastructure".to_string()),
            endpoint: None, // Will be set if endpoints exist

            // Audit report fields
            version: "1.0.0".to_string(),
            auditor: AuditorInfo {
                name: "watchy".to_string(),
                address: client_address.map(|s| s.to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            timestamp: now.timestamp() as u64,
            block_number: 0,
            agent: AgentInfo {
                agent_id,
                registry: registry_full,
                metadata_uri,
                owner: None,
            },
            scores: Scores {
                overall: 0,
                metadata: 0,
                onchain: 0,
                endpoint_availability: 0,
                endpoint_performance: 0,
                security: 0,
                consistency: 0,
                content: 0,
            },
            checks: Checks {
                metadata: MetadataChecks {
                    passed: false,
                    required_fields: CheckResult {
                        passed: false,
                        details: serde_json::Value::Null,
                    },
                    type_field: CheckResult {
                        passed: false,
                        details: serde_json::Value::Null,
                    },
                    urls_valid: CheckResult {
                        passed: false,
                        details: serde_json::Value::Null,
                    },
                    recommended_fields: RecommendedFieldsCheck {
                        passed: false,
                        missing: vec![],
                    },
                    issues: vec![],
                },
                onchain: OnchainChecks {
                    passed: false,
                    agent_exists: false,
                    uri_matches: false,
                    wallet_set: false,
                    issues: vec![],
                },
                endpoints: vec![],
                security: SecurityChecks::default(),
                consistency: ConsistencyChecks::default(),
                content: ContentChecks::default(),
            },

            // Links
            report_markdown_url: None,
            report_json_url: None,
            signature: None,

            // On-chain feedback
            feedback_chain_id: None,
            feedback_tx_hash: None,
        }
    }

    /// Calculate overall score from component scores
    /// Weights: availability 35%, performance 20%, security 10%, metadata 15%, onchain 10%, consistency 5%, content 5%
    pub fn calculate_overall_score(&mut self) {
        self.scores.overall = (
            self.scores.endpoint_availability as f64 * 0.35
            + self.scores.endpoint_performance as f64 * 0.20
            + self.scores.security as f64 * 0.10
            + self.scores.metadata as f64 * 0.15
            + self.scores.onchain as f64 * 0.10
            + self.scores.consistency as f64 * 0.05
            + self.scores.content as f64 * 0.05
        ) as u8;

        // Also set the feedback value
        self.value = self.scores.overall as i128;
    }

    /// Set the primary endpoint for feedback
    pub fn set_primary_endpoint(&mut self, endpoint: &str) {
        self.endpoint = Some(endpoint.to_string());
    }

    /// Set the markdown report URL
    pub fn set_markdown_url(&mut self, url: &str) {
        self.report_markdown_url = Some(url.to_string());
    }

    pub fn set_json_url(&mut self, url: &str) {
        self.report_json_url = Some(url.to_string());
    }

    pub fn set_feedback_tx(&mut self, chain_id: u64, tx_hash: &str) {
        self.feedback_chain_id = Some(chain_id);
        self.feedback_tx_hash = Some(tx_hash.to_string());
    }

    /// Count issues by severity
    pub fn count_issues(&self) -> IssueCount {
        let mut count = IssueCount {
            critical: 0,
            error: 0,
            warning: 0,
            info: 0,
        };

        let all_issues = self
            .checks
            .metadata
            .issues
            .iter()
            .chain(self.checks.onchain.issues.iter())
            .chain(self.checks.endpoints.iter().flat_map(|e| e.issues.iter()))
            .chain(self.checks.security.issues.iter())
            .chain(self.checks.consistency.issues.iter())
            .chain(self.checks.content.issues.iter());

        for issue in all_issues {
            match issue.severity {
                Severity::Critical => count.critical += 1,
                Severity::Error => count.error += 1,
                Severity::Warning => count.warning += 1,
                Severity::Info => count.info += 1,
            }
        }

        count
    }
}
