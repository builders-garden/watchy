use tracing::{debug, warn};

use crate::types::{AgentMetadata, ContentChecks, DescriptionQuality, Issue, Severity, X402Check};

/// Minimum description length for quality check
const MIN_DESCRIPTION_LENGTH: usize = 50;

/// Placeholder texts that indicate incomplete metadata
const PLACEHOLDER_TEXTS: &[&str] = &[
    "todo",
    "tbd",
    "placeholder",
    "description here",
    "lorem ipsum",
    "test agent",
    "example agent",
    "insert description",
];

/// Run content quality checks on metadata
pub async fn check_content(
    client: &reqwest::Client,
    metadata: &AgentMetadata,
) -> ContentChecks {
    debug!("Running content quality checks");

    let mut checks = ContentChecks {
        passed: true,
        description_quality: DescriptionQuality::default(),
        valid_skill_taxonomy: true,
        has_contact_info: false,
        x402_valid: None,
        issues: vec![],
    };

    // Check description quality
    checks.description_quality = check_description_quality(metadata, &mut checks.issues);

    // Check skill taxonomy (OASF paths)
    checks.valid_skill_taxonomy = check_skill_taxonomy(metadata, &mut checks.issues);

    // Check for contact/support info
    checks.has_contact_info = check_contact_info(metadata);
    if !checks.has_contact_info {
        checks.issues.push(Issue {
            severity: Severity::Info,
            code: "NO_CONTACT_INFO".to_string(),
            message: "No contact or support information provided".to_string(),
        });
    }

    // Check x402 support if claimed
    if metadata.x402_support.unwrap_or(false) {
        checks.x402_valid = Some(check_x402_support(client, metadata).await);
        if let Some(x402_check) = &checks.x402_valid {
            if !x402_check.valid {
                checks.issues.push(Issue {
                    severity: Severity::Warning,
                    code: "X402_INVALID".to_string(),
                    message: x402_check.error.clone().unwrap_or_else(|| "x402 check failed".to_string()),
                });
            }
        }
    }

    // Overall pass/fail
    checks.passed = checks.description_quality.score >= 60
        && checks.valid_skill_taxonomy
        && (checks.x402_valid.as_ref().map(|x| x.valid).unwrap_or(true));

    checks
}

fn check_description_quality(metadata: &AgentMetadata, issues: &mut Vec<Issue>) -> DescriptionQuality {
    let description = metadata.description.as_deref().unwrap_or("");
    let length = description.len();
    let lower_desc = description.to_lowercase();

    let has_placeholder = PLACEHOLDER_TEXTS
        .iter()
        .any(|p| lower_desc.contains(p));

    // Check if description is meaningful (not just repeated words, has some variety)
    let words: Vec<&str> = description.split_whitespace().collect();
    let unique_words: std::collections::HashSet<&str> = words.iter().cloned().collect();
    let word_variety = if words.is_empty() {
        0.0
    } else {
        unique_words.len() as f64 / words.len() as f64
    };

    let is_meaningful = !has_placeholder
        && length >= MIN_DESCRIPTION_LENGTH
        && word_variety > 0.4
        && words.len() >= 8;

    let mut score = 100u8;

    if length < MIN_DESCRIPTION_LENGTH {
        score = score.saturating_sub(40);
        issues.push(Issue {
            severity: Severity::Warning,
            code: "DESCRIPTION_TOO_SHORT".to_string(),
            message: format!(
                "Description is {} characters (minimum {} recommended)",
                length, MIN_DESCRIPTION_LENGTH
            ),
        });
    }

    if has_placeholder {
        score = score.saturating_sub(30);
        issues.push(Issue {
            severity: Severity::Warning,
            code: "DESCRIPTION_PLACEHOLDER".to_string(),
            message: "Description appears to contain placeholder text".to_string(),
        });
    }

    if !is_meaningful && !has_placeholder && length >= MIN_DESCRIPTION_LENGTH {
        score = score.saturating_sub(20);
        issues.push(Issue {
            severity: Severity::Info,
            code: "DESCRIPTION_LOW_QUALITY".to_string(),
            message: "Description has low word variety or appears auto-generated".to_string(),
        });
    }

    DescriptionQuality {
        score,
        length,
        has_placeholder,
        is_meaningful,
    }
}

fn check_skill_taxonomy(metadata: &AgentMetadata, issues: &mut Vec<Issue>) -> bool {
    let mut valid = true;

    // Known OASF top-level domains
    let known_domains = [
        "agent_orchestration",
        "tool_interaction",
        "natural_language_processing",
        "data_processing",
        "web_interaction",
        "file_management",
        "communication",
        "development",
        "security",
        "blockchain",
        "creative",
        "analysis",
    ];

    for service in &metadata.services {
        if service.name.to_lowercase() == "a2a" {
            for skill in &service.a2a_skills {
                // Check if skill follows OASF taxonomy (path format)
                if skill.contains('/') {
                    // Extract top-level domain
                    let domain = skill.split('/').next().unwrap_or("");
                    if !known_domains.contains(&domain) {
                        issues.push(Issue {
                            severity: Severity::Info,
                            code: "UNKNOWN_SKILL_DOMAIN".to_string(),
                            message: format!(
                                "Skill '{}' uses unknown domain '{}' (not in OASF taxonomy)",
                                skill, domain
                            ),
                        });
                        // Don't fail for unknown domains, just warn
                    }
                }
            }
        }
    }

    valid
}

fn check_contact_info(metadata: &AgentMetadata) -> bool {
    // Check for contact info in description or dedicated fields
    let desc = metadata.description.as_deref().unwrap_or("").to_lowercase();

    // Look for email patterns
    let has_email = desc.contains('@') && desc.contains('.');

    // Look for URLs that might be support/contact pages
    let has_contact_url = desc.contains("support")
        || desc.contains("contact")
        || desc.contains("help")
        || desc.contains("discord")
        || desc.contains("telegram")
        || desc.contains("twitter")
        || desc.contains("github");

    // Check if web service might have contact
    let has_web = metadata.services.iter().any(|s| s.name.to_lowercase() == "web");

    has_email || has_contact_url || has_web
}

async fn check_x402_support(client: &reqwest::Client, metadata: &AgentMetadata) -> X402Check {
    let mut check = X402Check::default();

    // Find a paid endpoint to test
    let test_endpoint = metadata.services.iter()
        .filter(|s| s.endpoint.is_some())
        .find(|s| {
            s.name.to_lowercase() == "mcp" || s.name.to_lowercase() == "a2a"
        })
        .and_then(|s| s.endpoint.as_ref());

    let Some(endpoint) = test_endpoint else {
        check.error = Some("No testable endpoint found for x402 verification".to_string());
        return check;
    };

    debug!("Testing x402 support at {}", endpoint);

    // Send request without payment credentials
    match client.get(endpoint).send().await {
        Ok(response) => {
            let status = response.status();
            let headers = response.headers();

            // Check for 402 Payment Required
            if status.as_u16() == 402 {
                check.returns_402 = true;

                // Check for required payment headers
                // Standard x402 headers (various implementations use different headers)
                let payment_address = headers
                    .get("x-payment-address")
                    .or_else(|| headers.get("x-402-address"))
                    .or_else(|| headers.get("www-authenticate"))
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                let payment_amount = headers
                    .get("x-payment-amount")
                    .or_else(|| headers.get("x-402-amount"))
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                let payment_network = headers
                    .get("x-payment-network")
                    .or_else(|| headers.get("x-402-network"))
                    .or_else(|| headers.get("x-chain-id"))
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                check.has_payment_address = payment_address.is_some();
                check.has_payment_amount = payment_amount.is_some();
                check.has_payment_network = payment_network.is_some();
                check.payment_address = payment_address;
                check.payment_amount = payment_amount;
                check.payment_network = payment_network;

                // Valid x402 requires at least address
                check.valid = check.has_payment_address;

                if !check.valid {
                    check.error = Some("402 response missing required payment headers".to_string());
                }
            } else if status.is_success() {
                // Endpoint is free despite claiming x402 support
                check.error = Some(format!(
                    "Endpoint returned {} but metadata claims x402Support=true",
                    status.as_u16()
                ));
            } else if status.as_u16() == 401 {
                // Auth required but not 402
                check.error = Some("Endpoint requires auth (401) but not payment (402)".to_string());
            } else {
                check.error = Some(format!("Unexpected status code: {}", status.as_u16()));
            }
        }
        Err(e) => {
            check.error = Some(format!("Failed to test x402: {}", e));
        }
    }

    check
}

/// Calculate content quality score
pub fn calculate_content_score(checks: &ContentChecks) -> u8 {
    let mut score = 0u8;

    // Description quality (40 points max)
    score += (checks.description_quality.score as f64 * 0.4) as u8;

    // Skill taxonomy (20 points)
    if checks.valid_skill_taxonomy {
        score += 20;
    }

    // Contact info (15 points)
    if checks.has_contact_info {
        score += 15;
    }

    // x402 validity (25 points if claimed)
    if let Some(x402) = &checks.x402_valid {
        if x402.valid {
            score += 25;
        }
    } else {
        // Not claiming x402, give points anyway
        score += 25;
    }

    score
}
