use std::time::Instant;
use tracing::{debug, warn};

use crate::types::{EndpointCheck, Issue, LatencyMetrics, Service, ServiceType, Severity};

/// Test a service endpoint
#[allow(dead_code)]
pub async fn test_endpoint(
    client: &reqwest::Client,
    service_name: &str,
    endpoint: &str,
    service: &Service,
) -> EndpointCheck {
    debug!("Testing {} endpoint: {}", service_name, endpoint);

    let service_type = ServiceType::from(service_name);

    let mut check = EndpointCheck {
        service: service_name.to_string(),
        endpoint: endpoint.to_string(),
        reachable: false,
        valid_schema: None,
        skills_match: None,
        latency: None,
        error: None,
        issues: vec![],
    };

    // Measure latency with multiple requests
    let latencies = measure_latency(client, endpoint, 3).await;

    if latencies.is_empty() {
        check.error = Some("Connection failed".to_string());
        check.issues.push(Issue {
            severity: Severity::Critical,
            code: "ENDPOINT_UNREACHABLE".to_string(),
            message: format!("{} endpoint is not reachable", service_name),
        });
        return check;
    }

    check.reachable = true;
    check.latency = Some(calculate_percentiles(&latencies));

    // Validate response based on service type
    match service_type {
        ServiceType::A2A => {
            validate_a2a(client, endpoint, service, &mut check).await;
        }
        ServiceType::MCP => {
            validate_mcp(client, endpoint, service, &mut check).await;
        }
        ServiceType::OASF => {
            validate_oasf(client, endpoint, service, &mut check).await;
        }
        ServiceType::Web => {
            // Web endpoints just need to be reachable with valid TLS
            check.valid_schema = Some(true);
        }
        _ => {
            // Unknown service types - just check reachability
        }
    }

    // Check for high latency
    if let Some(latency) = &check.latency {
        if latency.p95 > 2000 {
            check.issues.push(Issue {
                severity: Severity::Warning,
                code: "HIGH_LATENCY".to_string(),
                message: format!("Endpoint p95 latency is {}ms (> 2000ms)", latency.p95),
            });
        }
    }

    check
}

/// Test a service endpoint and return both the check and the raw JSON response
pub async fn test_endpoint_with_response(
    client: &reqwest::Client,
    service_name: &str,
    endpoint: &str,
    service: &Service,
) -> (EndpointCheck, Option<serde_json::Value>) {
    debug!("Testing {} endpoint: {}", service_name, endpoint);

    let service_type = ServiceType::from(service_name);

    let mut check = EndpointCheck {
        service: service_name.to_string(),
        endpoint: endpoint.to_string(),
        reachable: false,
        valid_schema: None,
        skills_match: None,
        latency: None,
        error: None,
        issues: vec![],
    };

    // Measure latency with multiple requests
    let latencies = measure_latency(client, endpoint, 3).await;

    if latencies.is_empty() {
        check.error = Some("Connection failed".to_string());
        check.issues.push(Issue {
            severity: Severity::Critical,
            code: "ENDPOINT_UNREACHABLE".to_string(),
            message: format!("{} endpoint is not reachable", service_name),
        });
        return (check, None);
    }

    check.reachable = true;
    check.latency = Some(calculate_percentiles(&latencies));

    // Validate response based on service type and capture JSON
    let json_response = match service_type {
        ServiceType::A2A => {
            validate_a2a_with_response(client, endpoint, service, &mut check).await
        }
        ServiceType::MCP => {
            validate_mcp_with_response(client, endpoint, service, &mut check).await
        }
        ServiceType::OASF => {
            validate_oasf_with_response(client, endpoint, service, &mut check).await
        }
        ServiceType::Web => {
            // Web endpoints just need to be reachable with valid TLS
            check.valid_schema = Some(true);
            None
        }
        _ => None,
    };

    // Check for high latency
    if let Some(latency) = &check.latency {
        if latency.p95 > 2000 {
            check.issues.push(Issue {
                severity: Severity::Warning,
                code: "HIGH_LATENCY".to_string(),
                message: format!("Endpoint p95 latency is {}ms (> 2000ms)", latency.p95),
            });
        }
    }

    (check, json_response)
}

async fn measure_latency(client: &reqwest::Client, endpoint: &str, samples: u32) -> Vec<u64> {
    let mut latencies = vec![];

    for _ in 0..samples {
        let start = Instant::now();
        let result = client.head(endpoint).send().await;

        if result.is_ok() {
            latencies.push(start.elapsed().as_millis() as u64);
        }

        // Small delay between requests
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    latencies
}

fn calculate_percentiles(latencies: &[u64]) -> LatencyMetrics {
    let mut sorted = latencies.to_vec();
    sorted.sort();

    let len = sorted.len();
    if len == 0 {
        return LatencyMetrics {
            p50: 0,
            p95: 0,
            p99: 0,
        };
    }

    LatencyMetrics {
        p50: sorted[len / 2],
        p95: sorted[(len as f64 * 0.95) as usize].min(sorted[len - 1]),
        p99: sorted[(len as f64 * 0.99) as usize].min(sorted[len - 1]),
    }
}

#[allow(dead_code)]
async fn validate_a2a(
    client: &reqwest::Client,
    endpoint: &str,
    service: &Service,
    check: &mut EndpointCheck,
) {
    // Fetch and validate A2A agent card
    let response = match client.get(endpoint).send().await {
        Ok(r) => r,
        Err(e) => {
            check.valid_schema = Some(false);
            check.issues.push(Issue {
                severity: Severity::Error,
                code: "A2A_FETCH_FAILED".to_string(),
                message: format!("Failed to fetch A2A agent card: {}", e),
            });
            return;
        }
    };

    let json: serde_json::Value = match response.json().await {
        Ok(j) => j,
        Err(e) => {
            check.valid_schema = Some(false);
            check.issues.push(Issue {
                severity: Severity::Error,
                code: "INVALID_JSON".to_string(),
                message: format!("A2A endpoint returned invalid JSON: {}", e),
            });
            return;
        }
    };

    // Basic A2A schema validation
    let has_name = json.get("name").and_then(|v| v.as_str()).is_some();
    let has_skills = json.get("skills").is_some() || json.get("capabilities").is_some();

    check.valid_schema = Some(has_name && has_skills);

    if !has_name {
        check.issues.push(Issue {
            severity: Severity::Error,
            code: "A2A_MISSING_NAME".to_string(),
            message: "A2A agent card missing 'name' field".to_string(),
        });
    }

    // Check if declared skills match
    if !service.a2a_skills.is_empty() {
        if let Some(skills) = json.get("skills").and_then(|v| v.as_array()) {
            let actual_skills: Vec<String> = skills
                .iter()
                .filter_map(|s| s.as_str().map(|s| s.to_string()))
                .collect();

            let declared_present = service
                .a2a_skills
                .iter()
                .all(|s| actual_skills.iter().any(|a| a.contains(s) || s.contains(a)));

            check.skills_match = Some(declared_present);

            if !declared_present {
                check.issues.push(Issue {
                    severity: Severity::Warning,
                    code: "A2A_SKILLS_MISMATCH".to_string(),
                    message: "Declared A2A skills don't match agent card".to_string(),
                });
            }
        }
    }
}

#[allow(dead_code)]
async fn validate_mcp(
    client: &reqwest::Client,
    endpoint: &str,
    service: &Service,
    check: &mut EndpointCheck,
) {
    let response = match client.get(endpoint).send().await {
        Ok(r) => r,
        Err(e) => {
            check.valid_schema = Some(false);
            warn!("MCP fetch failed: {}", e);
            return;
        }
    };

    let json: serde_json::Value = match response.json().await {
        Ok(j) => j,
        Err(e) => {
            check.valid_schema = Some(false);
            check.issues.push(Issue {
                severity: Severity::Error,
                code: "INVALID_JSON".to_string(),
                message: format!("MCP endpoint returned invalid JSON: {}", e),
            });
            return;
        }
    };

    // Basic MCP schema validation
    let has_tools = json.get("tools").is_some();
    check.valid_schema = Some(has_tools);

    // Check if declared tools match
    if !service.mcp_tools.is_empty() {
        if let Some(tools) = json.get("tools").and_then(|v| v.as_array()) {
            let actual_tools: Vec<String> = tools
                .iter()
                .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                .collect();

            let declared_present = service
                .mcp_tools
                .iter()
                .all(|t| actual_tools.contains(t));

            check.skills_match = Some(declared_present);

            if !declared_present {
                check.issues.push(Issue {
                    severity: Severity::Warning,
                    code: "MCP_TOOLS_MISMATCH".to_string(),
                    message: "Declared MCP tools don't match manifest".to_string(),
                });
            }
        }
    }
}

#[allow(dead_code)]
async fn validate_oasf(
    client: &reqwest::Client,
    endpoint: &str,
    _service: &Service,
    check: &mut EndpointCheck,
) {
    let response = match client.get(endpoint).send().await {
        Ok(r) => r,
        Err(e) => {
            check.valid_schema = Some(false);
            warn!("OASF fetch failed: {}", e);
            return;
        }
    };

    let json: serde_json::Value = match response.json().await {
        Ok(j) => j,
        Err(e) => {
            check.valid_schema = Some(false);
            check.issues.push(Issue {
                severity: Severity::Error,
                code: "INVALID_JSON".to_string(),
                message: format!("OASF endpoint returned invalid JSON: {}", e),
            });
            return;
        }
    };

    // OASF validation - check for skills/domains
    let has_structure = json.get("skills").is_some() || json.get("domains").is_some();
    check.valid_schema = Some(has_structure);
}

// Variants that return the JSON response for consistency checks

async fn validate_a2a_with_response(
    client: &reqwest::Client,
    endpoint: &str,
    service: &Service,
    check: &mut EndpointCheck,
) -> Option<serde_json::Value> {
    let response = match client.get(endpoint).send().await {
        Ok(r) => r,
        Err(e) => {
            check.valid_schema = Some(false);
            check.issues.push(Issue {
                severity: Severity::Error,
                code: "A2A_FETCH_FAILED".to_string(),
                message: format!("Failed to fetch A2A agent card: {}", e),
            });
            return None;
        }
    };

    let json: serde_json::Value = match response.json().await {
        Ok(j) => j,
        Err(e) => {
            check.valid_schema = Some(false);
            check.issues.push(Issue {
                severity: Severity::Error,
                code: "INVALID_JSON".to_string(),
                message: format!("A2A endpoint returned invalid JSON: {}", e),
            });
            return None;
        }
    };

    // Basic A2A schema validation
    let has_name = json.get("name").and_then(|v| v.as_str()).is_some();
    let has_skills = json.get("skills").is_some() || json.get("capabilities").is_some();

    check.valid_schema = Some(has_name && has_skills);

    if !has_name {
        check.issues.push(Issue {
            severity: Severity::Error,
            code: "A2A_MISSING_NAME".to_string(),
            message: "A2A agent card missing 'name' field".to_string(),
        });
    }

    // Check if declared skills match
    if !service.a2a_skills.is_empty() {
        if let Some(skills) = json.get("skills").and_then(|v| v.as_array()) {
            let actual_skills: Vec<String> = skills
                .iter()
                .filter_map(|s| s.as_str().map(|s| s.to_string()))
                .collect();

            let declared_present = service
                .a2a_skills
                .iter()
                .all(|s| actual_skills.iter().any(|a| a.contains(s) || s.contains(a)));

            check.skills_match = Some(declared_present);

            if !declared_present {
                check.issues.push(Issue {
                    severity: Severity::Warning,
                    code: "A2A_SKILLS_MISMATCH".to_string(),
                    message: "Declared A2A skills don't match agent card".to_string(),
                });
            }
        }
    }

    Some(json)
}

async fn validate_mcp_with_response(
    client: &reqwest::Client,
    endpoint: &str,
    service: &Service,
    check: &mut EndpointCheck,
) -> Option<serde_json::Value> {
    let response = match client.get(endpoint).send().await {
        Ok(r) => r,
        Err(e) => {
            check.valid_schema = Some(false);
            warn!("MCP fetch failed: {}", e);
            return None;
        }
    };

    let json: serde_json::Value = match response.json().await {
        Ok(j) => j,
        Err(e) => {
            check.valid_schema = Some(false);
            check.issues.push(Issue {
                severity: Severity::Error,
                code: "INVALID_JSON".to_string(),
                message: format!("MCP endpoint returned invalid JSON: {}", e),
            });
            return None;
        }
    };

    // Basic MCP schema validation
    let has_tools = json.get("tools").is_some();
    check.valid_schema = Some(has_tools);

    // Check if declared tools match
    if !service.mcp_tools.is_empty() {
        if let Some(tools) = json.get("tools").and_then(|v| v.as_array()) {
            let actual_tools: Vec<String> = tools
                .iter()
                .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                .collect();

            let declared_present = service
                .mcp_tools
                .iter()
                .all(|t| actual_tools.contains(t));

            check.skills_match = Some(declared_present);

            if !declared_present {
                check.issues.push(Issue {
                    severity: Severity::Warning,
                    code: "MCP_TOOLS_MISMATCH".to_string(),
                    message: "Declared MCP tools don't match manifest".to_string(),
                });
            }
        }
    }

    Some(json)
}

async fn validate_oasf_with_response(
    client: &reqwest::Client,
    endpoint: &str,
    _service: &Service,
    check: &mut EndpointCheck,
) -> Option<serde_json::Value> {
    let response = match client.get(endpoint).send().await {
        Ok(r) => r,
        Err(e) => {
            check.valid_schema = Some(false);
            warn!("OASF fetch failed: {}", e);
            return None;
        }
    };

    let json: serde_json::Value = match response.json().await {
        Ok(j) => j,
        Err(e) => {
            check.valid_schema = Some(false);
            check.issues.push(Issue {
                severity: Severity::Error,
                code: "INVALID_JSON".to_string(),
                message: format!("OASF endpoint returned invalid JSON: {}", e),
            });
            return None;
        }
    };

    // OASF validation - check for skills/domains
    let has_structure = json.get("skills").is_some() || json.get("domains").is_some();
    check.valid_schema = Some(has_structure);

    Some(json)
}
