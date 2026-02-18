use tracing::debug;

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
    "under construction",
    "wip",
    "work in progress",
    "coming soon",
    "not yet available",
    "sample description",
    "default description",
    "add description",
    "enter description",
    "your description",
    "description goes here",
    "[description]",
    "<description>",
    "n/a",
    "none",
    "empty",
    "blank",
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
    let valid = true;

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
    let desc = metadata.description.as_deref().unwrap_or("");

    // Look for email patterns (simple but more accurate pattern)
    let has_email = check_has_email(desc);

    // Look for URLs that might be support/contact pages
    let desc_lower = desc.to_lowercase();
    let contact_keywords = [
        "support", "contact", "help", "discord", "telegram",
        "twitter", "github", "email", "mailto:", "x.com",
        "@twitter", "@discord", "t.me/", "discord.gg/"
    ];
    let has_contact_url = contact_keywords.iter().any(|kw| desc_lower.contains(kw));

    // Check author info for contact
    let has_author_contact = metadata.author.as_ref().map(|a| {
        a.url.is_some() || a.twitter.is_some()
    }).unwrap_or(false);

    // Check if web service might have contact
    let has_web = metadata.services.iter().any(|s| s.name.to_lowercase() == "web");

    has_email || has_contact_url || has_author_contact || has_web
}

/// Check if text contains a valid email pattern
fn check_has_email(text: &str) -> bool {
    // Simple email pattern: something@something.something
    // Must have: local part, @, domain, ., tld
    let words: Vec<&str> = text.split_whitespace().collect();

    for word in words {
        // Strip common punctuation from the word
        let cleaned = word.trim_matches(|c: char| c.is_ascii_punctuation() && c != '@' && c != '.' && c != '-' && c != '_');

        if let Some(at_pos) = cleaned.find('@') {
            let (local, domain) = cleaned.split_at(at_pos);
            let domain = &domain[1..]; // Skip the @

            // Local part must be non-empty and reasonable
            if local.is_empty() || local.len() > 64 {
                continue;
            }

            // Domain must have at least one dot and valid structure
            if let Some(dot_pos) = domain.rfind('.') {
                let tld = &domain[dot_pos + 1..];
                let domain_part = &domain[..dot_pos];

                // TLD must be 2-10 chars, domain part must be non-empty
                if !domain_part.is_empty()
                    && tld.len() >= 2
                    && tld.len() <= 10
                    && tld.chars().all(|c| c.is_ascii_alphabetic())
                {
                    return true;
                }
            }
        }
    }

    false
}

/// Timeout for x402 test requests in seconds
const X402_TEST_TIMEOUT_SECS: u64 = 10;

async fn check_x402_support(client: &reqwest::Client, metadata: &AgentMetadata) -> X402Check {
    let mut check = X402Check::default();

    // Find all MCP/A2A endpoints to test
    let test_endpoints: Vec<&str> = metadata.services.iter()
        .filter(|s| {
            let name = s.name.to_lowercase();
            (name == "mcp" || name == "a2a") && s.endpoint.is_some()
        })
        .filter_map(|s| s.endpoint.as_deref())
        .filter(|e| e.starts_with("http"))
        .collect();

    if test_endpoints.is_empty() {
        check.error = Some("No testable endpoint found for x402 verification".to_string());
        return check;
    }

    debug!("Testing x402 support on {} endpoints", test_endpoints.len());

    let mut valid_count = 0;
    let mut errors = vec![];

    for endpoint in &test_endpoints {
        debug!("Testing x402 support at {}", endpoint);

        // Send request without payment credentials
        let result = client
            .get(*endpoint)
            .timeout(std::time::Duration::from_secs(X402_TEST_TIMEOUT_SECS))
            .send()
            .await;

        match result {
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

                    // Update check fields with first valid response
                    if payment_address.is_some() && check.payment_address.is_none() {
                        check.has_payment_address = true;
                        check.has_payment_amount = payment_amount.is_some();
                        check.has_payment_network = payment_network.is_some();
                        check.payment_address = payment_address;
                        check.payment_amount = payment_amount;
                        check.payment_network = payment_network;
                        valid_count += 1;
                    } else if payment_address.is_none() {
                        errors.push(format!("{}: 402 response missing payment headers", endpoint));
                    } else {
                        valid_count += 1;
                    }
                } else if status.is_success() {
                    // Endpoint is free despite claiming x402 support
                    errors.push(format!(
                        "{}: returned {} but claims x402Support=true",
                        endpoint, status.as_u16()
                    ));
                } else if status.as_u16() == 401 {
                    // Auth required but not 402
                    errors.push(format!("{}: requires auth (401) not payment (402)", endpoint));
                } else {
                    errors.push(format!("{}: unexpected status {}", endpoint, status.as_u16()));
                }
            }
            Err(e) => {
                errors.push(format!("{}: {}", endpoint, e));
            }
        }
    }

    // Valid if at least one endpoint returns proper 402
    check.valid = valid_count > 0;

    if !check.valid && !errors.is_empty() {
        check.error = Some(errors.join("; "));
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
