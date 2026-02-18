use tracing::{debug, warn};

use crate::types::{AgentMetadata, ConsistencyChecks, Issue, Severity};

/// Run consistency checks across metadata and endpoint responses
pub async fn check_consistency(
    client: &reqwest::Client,
    metadata: &AgentMetadata,
    endpoint_responses: &EndpointResponses,
) -> ConsistencyChecks {
    debug!("Running consistency checks");

    let mut checks = ConsistencyChecks {
        passed: true,
        name_consistent: true,
        skills_consistent: true,
        version_consistent: true,
        image_accessible: false,
        issues: vec![],
    };

    // Check name consistency across endpoints
    let metadata_name = metadata.name.as_deref().unwrap_or("");

    if let Some(a2a_name) = &endpoint_responses.a2a_name {
        if !names_match(metadata_name, a2a_name) {
            checks.name_consistent = false;
            checks.issues.push(Issue {
                severity: Severity::Warning,
                code: "NAME_MISMATCH_A2A".to_string(),
                message: format!(
                    "Metadata name '{}' doesn't match A2A agent card name '{}'",
                    metadata_name, a2a_name
                ),
            });
        }
    }

    if let Some(mcp_name) = &endpoint_responses.mcp_name {
        if !names_match(metadata_name, mcp_name) {
            checks.name_consistent = false;
            checks.issues.push(Issue {
                severity: Severity::Warning,
                code: "NAME_MISMATCH_MCP".to_string(),
                message: format!(
                    "Metadata name '{}' doesn't match MCP manifest name '{}'",
                    metadata_name, mcp_name
                ),
            });
        }
    }

    // Check skills/tools consistency
    checks.skills_consistent = check_skills_consistency(metadata, endpoint_responses, &mut checks.issues);

    // Check version consistency
    checks.version_consistent = check_version_consistency(metadata, endpoint_responses, &mut checks.issues);

    // Check if image is accessible
    if let Some(image_url) = &metadata.image {
        checks.image_accessible = check_image_accessible(client, image_url).await;
        if !checks.image_accessible {
            checks.issues.push(Issue {
                severity: Severity::Warning,
                code: "IMAGE_INACCESSIBLE".to_string(),
                message: format!("Agent image URL is not accessible: {}", image_url),
            });
        }
    }

    // Overall pass/fail
    checks.passed = checks.name_consistent && checks.skills_consistent && checks.image_accessible;

    checks
}

/// Endpoint responses collected during endpoint testing
#[derive(Debug, Default)]
pub struct EndpointResponses {
    pub a2a_name: Option<String>,
    pub a2a_skills: Vec<String>,
    pub a2a_version: Option<String>,
    pub mcp_name: Option<String>,
    pub mcp_tools: Vec<String>,
    pub mcp_version: Option<String>,
    pub oasf_skills: Vec<String>,
    pub oasf_version: Option<String>,
}

impl EndpointResponses {
    pub fn from_json_responses(
        a2a_response: Option<&serde_json::Value>,
        mcp_response: Option<&serde_json::Value>,
        oasf_response: Option<&serde_json::Value>,
    ) -> Self {
        let mut responses = Self::default();

        if let Some(a2a) = a2a_response {
            responses.a2a_name = a2a.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
            responses.a2a_version = a2a.get("version").and_then(|v| v.as_str()).map(|s| s.to_string());

            if let Some(skills) = a2a.get("skills").and_then(|v| v.as_array()) {
                responses.a2a_skills = skills
                    .iter()
                    .filter_map(|s| {
                        // Skills can be objects with "id" or strings
                        s.get("id")
                            .and_then(|v| v.as_str())
                            .or_else(|| s.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect();
            }
        }

        if let Some(mcp) = mcp_response {
            responses.mcp_name = mcp.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
            responses.mcp_version = mcp
                .get("protocolVersion")
                .or_else(|| mcp.get("version"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if let Some(tools) = mcp.get("tools").and_then(|v| v.as_array()) {
                responses.mcp_tools = tools
                    .iter()
                    .filter_map(|t| {
                        t.get("name")
                            .and_then(|v| v.as_str())
                            .or_else(|| t.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect();
            }
        }

        if let Some(oasf) = oasf_response {
            responses.oasf_version = oasf.get("version").and_then(|v| v.as_str()).map(|s| s.to_string());

            if let Some(skills) = oasf.get("skills").and_then(|v| v.as_array()) {
                responses.oasf_skills = skills
                    .iter()
                    .filter_map(|s| s.as_str().map(|s| s.to_string()))
                    .collect();
            }
        }

        responses
    }
}

fn names_match(name1: &str, name2: &str) -> bool {
    // Case-insensitive comparison, ignoring whitespace differences
    let n1 = name1.trim().to_lowercase();
    let n2 = name2.trim().to_lowercase();
    n1 == n2
}

fn check_skills_consistency(
    metadata: &AgentMetadata,
    responses: &EndpointResponses,
    issues: &mut Vec<Issue>,
) -> bool {
    let mut consistent = true;

    // Get declared skills from metadata services
    for service in &metadata.services {
        match service.name.to_lowercase().as_str() {
            "a2a" => {
                if !service.a2a_skills.is_empty() && !responses.a2a_skills.is_empty() {
                    // Check if declared skills are present in actual response
                    for declared in &service.a2a_skills {
                        let found = responses.a2a_skills.iter().any(|actual| {
                            skills_match(declared, actual)
                        });
                        if !found {
                            consistent = false;
                            issues.push(Issue {
                                severity: Severity::Warning,
                                code: "A2A_SKILL_NOT_FOUND".to_string(),
                                message: format!(
                                    "Declared A2A skill '{}' not found in agent card",
                                    declared
                                ),
                            });
                        }
                    }
                }
            }
            "mcp" => {
                if !service.mcp_tools.is_empty() && !responses.mcp_tools.is_empty() {
                    for declared in &service.mcp_tools {
                        if !responses.mcp_tools.contains(declared) {
                            consistent = false;
                            issues.push(Issue {
                                severity: Severity::Warning,
                                code: "MCP_TOOL_NOT_FOUND".to_string(),
                                message: format!(
                                    "Declared MCP tool '{}' not found in manifest",
                                    declared
                                ),
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    consistent
}

/// Strict skill matching for consistency checks
/// Matches if:
/// - Exact match (case-insensitive)
/// - OASF taxonomy path match: last segments match
fn skills_match(declared: &str, actual: &str) -> bool {
    let declared_lower = declared.to_lowercase();
    let actual_lower = actual.to_lowercase();

    // Exact match
    if declared_lower == actual_lower {
        return true;
    }

    // OASF taxonomy path matching
    let declared_segments: Vec<&str> = declared_lower.split('/').collect();
    let actual_segments: Vec<&str> = actual_lower.split('/').collect();

    // Check if last segments match
    if let (Some(declared_last), Some(actual_last)) = (declared_segments.last(), actual_segments.last()) {
        if declared_last == actual_last {
            return true;
        }
    }

    // Check if declared is a suffix of actual or vice versa (for paths)
    if declared_segments.len() > 1 && actual_segments.len() > 1 {
        let min_len = declared_segments.len().min(actual_segments.len());
        let declared_suffix: Vec<&str> = declared_segments.iter().rev().take(min_len).cloned().collect();
        let actual_suffix: Vec<&str> = actual_segments.iter().rev().take(min_len).cloned().collect();
        if declared_suffix == actual_suffix {
            return true;
        }
    }

    false
}

fn check_version_consistency(
    metadata: &AgentMetadata,
    responses: &EndpointResponses,
    issues: &mut Vec<Issue>,
) -> bool {
    let mut consistent = true;

    for service in &metadata.services {
        let declared_version = service.version.as_deref();

        match service.name.to_lowercase().as_str() {
            "a2a" => {
                if let (Some(declared), Some(actual)) = (declared_version, &responses.a2a_version) {
                    if !versions_match(declared, actual) {
                        consistent = false;
                        issues.push(Issue {
                            severity: Severity::Info,
                            code: "A2A_VERSION_MISMATCH".to_string(),
                            message: format!(
                                "Declared A2A version '{}' doesn't match actual '{}'",
                                declared, actual
                            ),
                        });
                    }
                }
            }
            "mcp" => {
                if let (Some(declared), Some(actual)) = (declared_version, &responses.mcp_version) {
                    if !versions_match(declared, actual) {
                        consistent = false;
                        issues.push(Issue {
                            severity: Severity::Info,
                            code: "MCP_VERSION_MISMATCH".to_string(),
                            message: format!(
                                "Declared MCP version '{}' doesn't match actual '{}'",
                                declared, actual
                            ),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    consistent
}

/// Semantic version matching with flexibility
/// Matches if:
/// - Exact match (case-insensitive)
/// - Version with/without 'v' prefix: "v1.0" matches "1.0"
/// - Major.minor matches major.minor.patch: "1.0" matches "1.0.0"
/// - Versions with same major.minor: "1.0.1" matches "1.0.2" (compatible)
fn versions_match(declared: &str, actual: &str) -> bool {
    let d = normalize_version(declared);
    let a = normalize_version(actual);

    // Exact match after normalization
    if d == a {
        return true;
    }

    // Parse into components
    let d_parts: Vec<u32> = d.split('.').filter_map(|s| s.parse().ok()).collect();
    let a_parts: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();

    if d_parts.is_empty() || a_parts.is_empty() {
        return false;
    }

    // Major version must match
    if d_parts[0] != a_parts[0] {
        return false;
    }

    // If only major is declared, match any minor/patch
    if d_parts.len() == 1 {
        return true;
    }

    // Minor version must match if both have it
    if d_parts.len() >= 2 && a_parts.len() >= 2 {
        if d_parts[1] != a_parts[1] {
            return false;
        }
    }

    // Patch can differ (compatible versions)
    true
}

/// Normalize version string: remove 'v' prefix, trim whitespace
fn normalize_version(version: &str) -> String {
    version
        .trim()
        .to_lowercase()
        .strip_prefix('v')
        .unwrap_or(version.trim())
        .to_string()
}

async fn check_image_accessible(client: &reqwest::Client, image_url: &str) -> bool {
    match client.head(image_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                // Check content-type is an image
                if let Some(content_type) = response.headers().get("content-type") {
                    if let Ok(ct) = content_type.to_str() {
                        return ct.starts_with("image/");
                    }
                }
                // No content-type but successful response - assume OK
                true
            } else {
                false
            }
        }
        Err(e) => {
            warn!("Failed to check image accessibility: {}", e);
            false
        }
    }
}

/// Calculate consistency score
pub fn calculate_consistency_score(checks: &ConsistencyChecks) -> u8 {
    let mut score = 100u8;

    if !checks.name_consistent {
        score = score.saturating_sub(20);
    }

    if !checks.skills_consistent {
        score = score.saturating_sub(30);
    }

    if !checks.version_consistent {
        score = score.saturating_sub(10);
    }

    if !checks.image_accessible {
        score = score.saturating_sub(15);
    }

    score
}
