use chrono::{DateTime, Utc};
use std::path::Path;
use tokio::fs;
use tracing::info;

use crate::types::{AgentMetadata, AuditReport, WatchyError};

/// Generate a markdown report from audit results
pub fn generate_markdown_report(
    report: &AuditReport,
    metadata: Option<&AgentMetadata>,
) -> String {
    let timestamp = DateTime::<Utc>::from_timestamp(report.timestamp as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let agent_name = metadata
        .and_then(|m| m.name.as_deref())
        .unwrap_or("Unknown");

    let mut md = String::new();

    // ========== HEADER ==========
    md.push_str(&format!(
        r#"# Watchy Audit Report

## Agent #{} - {}

**Overall Score: {}/100** {}

**Audited on {} | Block #{}**

---

"#,
        report.agent.agent_id,
        agent_name,
        report.scores.overall,
        score_emoji(report.scores.overall),
        timestamp,
        format_number(report.block_number)
    ));

    // ========== DISCLAIMER ==========
    md.push_str(r#"## What This Audit Covers

> **Important:** This audit verifies the *infrastructure and metadata* of an EIP-8004 agent. It does **NOT** test the actual functionality of the agent's tools or skills.

### What We Test

| Category | What We Check | What We DON'T Check |
|----------|---------------|---------------------|
| **Endpoints** | Reachability, latency, valid JSON response | Actual tool execution or correctness |
| **Schema** | Response structure matches expected format | Business logic or output quality |
| **Security** | TLS certificates, security headers | Authentication flows, access control |
| **Metadata** | Fields present, URLs valid, registration matches | Content accuracy or truthfulness |
| **Consistency** | Names/versions match across endpoints | Semantic equivalence of skills |

**Think of this audit as a "health check" - we verify the agent is properly configured and responsive, not that it performs its tasks correctly.**

---

"#);

    // ========== SCORE BREAKDOWN ==========
    md.push_str("## Score Breakdown\n\n");

    md.push_str(&format!(
        r#"### Overall: {}/100 {}

| Component | Score | Weight |
|-----------|-------|--------|
| Endpoint Availability | {}/100 | 35% |
| Endpoint Performance | {}/100 | 20% |
| Security | {}/100 | 10% |
| Metadata | {}/100 | 15% |
| On-chain | {}/100 | 10% |
| Consistency | {}/100 | 5% |
| Content | {}/100 | 5% |

### Verdict

{}

{}

---

"#,
        report.scores.overall,
        score_emoji(report.scores.overall),
        report.scores.endpoint_availability,
        report.scores.endpoint_performance,
        report.scores.security,
        report.scores.metadata,
        report.scores.onchain,
        report.scores.consistency,
        report.scores.content,
        verdict_text(report.scores.overall),
        verdict_explanation(report.scores.overall)
    ));

    // ========== AGENT IDENTITY ==========
    md.push_str("## Agent Identity\n\n");
    md.push_str("*Verified on-chain registration information*\n\n");

    md.push_str("| Property | Value |\n");
    md.push_str("|----------|-------|\n");
    md.push_str(&format!("| **Agent ID** | `{}` |\n", report.agent.agent_id));
    md.push_str(&format!("| **Name** | {} |\n", agent_name));
    md.push_str(&format!("| **Registry** | `{}` |\n", report.agent.registry));
    if let Some(owner) = &report.agent.owner {
        md.push_str(&format!("| **Owner** | `{}` |\n", owner));
    }
    md.push_str(&format!("| **Metadata URI** | `{}` |\n", report.agent.metadata_uri));
    md.push_str("\n---\n\n");

    // ========== WHAT THE AGENT CLAIMS ==========
    if let Some(m) = metadata {
        md.push_str("## What The Agent Claims\n\n");
        md.push_str("*Information declared in the agent's metadata (not verified for accuracy)*\n\n");

        if let Some(desc) = &m.description {
            md.push_str(&format!("> {}\n\n", desc));
        }

        md.push_str("| Property | Value |\n");
        md.push_str("|----------|-------|\n");

        if let Some(image) = &m.image {
            md.push_str(&format!("| **Image** | [View]({}) |\n", image));
        }
        if let Some(active) = m.active {
            md.push_str(&format!("| **Active** | {} |\n", if active { "Yes" } else { "No" }));
        }
        if !m.supported_trust.is_empty() {
            md.push_str(&format!("| **Trust Mechanisms** | {} |\n", m.supported_trust.join(", ")));
        }
        if let Some(x402) = m.x402_support {
            md.push_str(&format!("| **Paid (x402)** | {} |\n", if x402 { "Yes" } else { "No" }));
        }
        md.push_str("\n");

        // Services
        if !m.services.is_empty() {
            md.push_str("### Declared Services\n\n");

            for service in &m.services {
                let emoji = match service.name.to_lowercase().as_str() {
                    "mcp" => "🔧",
                    "a2a" => "🤝",
                    "oasf" => "📋",
                    "web" => "🌐",
                    _ => "📡",
                };

                md.push_str(&format!("#### {} **{}**", emoji, service.name));
                if let Some(version) = &service.version {
                    md.push_str(&format!(" (v{})", version));
                }
                md.push_str("\n\n");

                if let Some(endpoint) = &service.endpoint {
                    md.push_str(&format!("**Endpoint:** `{}`\n\n", endpoint));
                }
                if !service.mcp_tools.is_empty() {
                    md.push_str(&format!("**Tools:** `{}`\n\n", service.mcp_tools.join("`, `")));
                }
                if !service.mcp_prompts.is_empty() {
                    md.push_str(&format!("**Prompts:** `{}`\n\n", service.mcp_prompts.join("`, `")));
                }
                if !service.a2a_skills.is_empty() {
                    md.push_str("**Skills:**\n");
                    for skill in &service.a2a_skills {
                        md.push_str(&format!("- `{}`\n", skill));
                    }
                    md.push_str("\n");
                }
            }
        }
        md.push_str("---\n\n");
    }

    // ========== AUDIT RESULTS ==========
    md.push_str("## Detailed Audit Results\n\n");

    // ----- On-chain -----
    md.push_str("### 1. On-chain Verification\n\n");
    md.push_str("*Checks that the agent exists in the EIP-8004 registry and has proper configuration*\n\n");

    md.push_str(&format!("**Score: {}/100**\n\n", report.scores.onchain));

    md.push_str("| Check | Result | Description |\n");
    md.push_str("|-------|--------|-------------|\n");
    md.push_str(&format!(
        "| Agent Exists | {} | Token ID exists in registry contract |\n",
        pass_fail(report.checks.onchain.agent_exists)
    ));
    md.push_str(&format!(
        "| Metadata URI | {} | IPFS/Arweave URI is set on-chain |\n",
        pass_fail(report.checks.onchain.uri_matches)
    ));
    md.push_str(&format!(
        "| Wallet Configured | {} | Agent has a payment wallet set |\n",
        pass_fail(report.checks.onchain.wallet_set)
    ));
    md.push_str("\n");

    // ----- Metadata -----
    md.push_str("### 2. Metadata Compliance\n\n");
    md.push_str("*Validates the agent's metadata follows the EIP-8004 specification*\n\n");

    md.push_str(&format!("**Score: {}/100**\n\n", report.scores.metadata));

    md.push_str("| Check | Result | Description |\n");
    md.push_str("|-------|--------|-------------|\n");
    md.push_str(&format!(
        "| Required Fields | {} | `type`, `name`, `description`, `image`, `registrations` |\n",
        pass_fail(report.checks.metadata.required_fields.passed)
    ));
    md.push_str(&format!(
        "| Type Field | {} | Matches `https://eips.ethereum.org/EIPS/eip-8004#registration-v1` |\n",
        pass_fail(report.checks.metadata.type_field.passed)
    ));
    md.push_str(&format!(
        "| Recommended Fields | {} | `active`, `services`, `supportedTrust`, `updatedAt` |\n",
        pass_fail(report.checks.metadata.recommended_fields.passed)
    ));
    md.push_str("\n");

    // ----- Endpoints -----
    if !report.checks.endpoints.is_empty() {
        md.push_str("### 3. Endpoint Testing\n\n");
        md.push_str("*Tests if declared service endpoints are reachable and respond with valid schemas*\n\n");

        md.push_str(&format!(
            "**Availability: {}/100** | **Performance: {}/100**\n\n",
            report.scores.endpoint_availability,
            report.scores.endpoint_performance
        ));

        md.push_str("> **Note:** We only verify that endpoints respond with the expected JSON structure. ");
        md.push_str("We do NOT execute tools, send tasks, or verify output correctness.\n\n");

        for endpoint in &report.checks.endpoints {
            let status_emoji = if endpoint.reachable { "🟢" } else { "🔴" };

            md.push_str(&format!("#### {} {}\n\n", status_emoji, endpoint.service));
            md.push_str(&format!("`{}`\n\n", endpoint.endpoint));

            md.push_str("| Metric | Value |\n");
            md.push_str("|--------|-------|\n");
            md.push_str(&format!("| Reachable | {} |\n", if endpoint.reachable { "Yes" } else { "No" }));

            if let Some(valid) = endpoint.valid_schema {
                md.push_str(&format!("| Valid Schema | {} |\n", if valid { "Yes" } else { "No" }));
            }
            if let Some(matches) = endpoint.skills_match {
                md.push_str(&format!("| Skills Match | {} |\n", if matches { "Yes" } else { "No" }));
            }
            if let Some(latency) = &endpoint.latency {
                md.push_str(&format!("| Latency (p50) | {}ms |\n", latency.p50));
                md.push_str(&format!("| Latency (p95) | {}ms |\n", latency.p95));
                md.push_str(&format!("| Performance | {} |\n", latency_rating(latency.p95)));
            }
            md.push_str("\n");
        }
    }

    // ----- Security -----
    md.push_str("### 4. Security Analysis\n\n");
    md.push_str("*Checks TLS configuration and security headers on HTTPS endpoints*\n\n");

    md.push_str(&format!("**Score: {}/100**\n\n", report.scores.security));

    md.push_str("> **Note:** This checks transport security only. We do not audit the agent's code, ");
    md.push_str("authentication mechanisms, or data handling practices.\n\n");

    md.push_str("| Check | Result | Why It Matters |\n");
    md.push_str("|-------|--------|----------------|\n");
    md.push_str(&format!(
        "| TLS Valid | {} | Encrypted connection, trusted certificate |\n",
        pass_fail(report.checks.security.tls_valid)
    ));
    md.push_str(&format!(
        "| Certificate Valid | {} | Not expired or self-signed |\n",
        pass_fail(report.checks.security.certificate_valid)
    ));
    md.push_str(&format!(
        "| HTTPS Enforced | {} | HTTP requests redirect to HTTPS |\n",
        pass_fail(report.checks.security.https_enforced)
    ));
    md.push_str(&format!(
        "| X-Content-Type-Options | {} | Prevents MIME-sniffing attacks |\n",
        pass_fail(report.checks.security.security_headers.x_content_type_options)
    ));
    md.push_str(&format!(
        "| Strict-Transport-Security | {} | Forces HTTPS for future requests |\n",
        pass_fail(report.checks.security.security_headers.strict_transport_security)
    ));
    md.push_str(&format!(
        "| Content-Security-Policy | {} | Prevents XSS attacks |\n",
        pass_fail(report.checks.security.security_headers.content_security_policy)
    ));
    md.push_str("\n");

    // ----- Consistency -----
    md.push_str("### 5. Consistency Analysis\n\n");
    md.push_str("*Verifies that information is consistent across metadata and endpoint responses*\n\n");

    md.push_str(&format!("**Score: {}/100**\n\n", report.scores.consistency));

    md.push_str("| Check | Result | What We Compare |\n");
    md.push_str("|-------|--------|------------------|\n");
    md.push_str(&format!(
        "| Name Consistent | {} | Metadata name vs A2A/MCP response names |\n",
        pass_fail(report.checks.consistency.name_consistent)
    ));
    md.push_str(&format!(
        "| Skills Consistent | {} | Declared skills vs actual endpoint skills |\n",
        pass_fail(report.checks.consistency.skills_consistent)
    ));
    md.push_str(&format!(
        "| Version Consistent | {} | Declared versions vs endpoint versions |\n",
        pass_fail(report.checks.consistency.version_consistent)
    ));
    md.push_str(&format!(
        "| Image Accessible | {} | Agent image URL returns valid image |\n",
        pass_fail(report.checks.consistency.image_accessible)
    ));
    md.push_str("\n");

    // ----- Content -----
    md.push_str("### 6. Content Quality\n\n");
    md.push_str("*Evaluates the quality and completeness of metadata content*\n\n");

    md.push_str(&format!("**Score: {}/100**\n\n", report.scores.content));

    md.push_str("| Check | Result | Details |\n");
    md.push_str("|-------|--------|----------|\n");
    md.push_str(&format!(
        "| Description Quality | {}/100 | Length: {} chars, Meaningful: {} |\n",
        report.checks.content.description_quality.score,
        report.checks.content.description_quality.length,
        if report.checks.content.description_quality.is_meaningful { "Yes" } else { "No" }
    ));
    md.push_str(&format!(
        "| Valid Skill Taxonomy | {} | Skills follow OASF naming conventions |\n",
        pass_fail(report.checks.content.valid_skill_taxonomy)
    ));
    md.push_str(&format!(
        "| Contact Info | {} | Has support/contact information |\n",
        pass_fail(report.checks.content.has_contact_info)
    ));

    if let Some(x402) = &report.checks.content.x402_valid {
        md.push_str(&format!(
            "| x402 Payment Flow | {} | Returns 402 with payment headers |\n",
            pass_fail(x402.valid)
        ));
    }
    md.push_str("\n---\n\n");

    // ========== ISSUES ==========
    let issues = report.count_issues();
    let total_issues = issues.critical + issues.error + issues.warning + issues.info;

    if total_issues > 0 {
        md.push_str("## Issues Found\n\n");

        if issues.critical > 0 {
            md.push_str(&format!("- 🔴 **{} Critical** - Must be fixed\n", issues.critical));
        }
        if issues.error > 0 {
            md.push_str(&format!("- 🟠 **{} Errors** - Should be fixed\n", issues.error));
        }
        if issues.warning > 0 {
            md.push_str(&format!("- 🟡 **{} Warnings** - Consider fixing\n", issues.warning));
        }
        if issues.info > 0 {
            md.push_str(&format!("- 🔵 **{} Info** - For your information\n", issues.info));
        }
        md.push_str("\n");

        md.push_str("### All Issues\n\n");
        md.push_str("| Severity | Code | Message |\n");
        md.push_str("|----------|------|----------|\n");

        let all_issues = report.checks.metadata.issues.iter()
            .chain(report.checks.onchain.issues.iter())
            .chain(report.checks.endpoints.iter().flat_map(|e| e.issues.iter()))
            .chain(report.checks.security.issues.iter())
            .chain(report.checks.consistency.issues.iter())
            .chain(report.checks.content.issues.iter());

        for issue in all_issues {
            let severity_emoji = match issue.severity {
                crate::types::Severity::Critical => "🔴",
                crate::types::Severity::Error => "🟠",
                crate::types::Severity::Warning => "🟡",
                crate::types::Severity::Info => "🔵",
            };
            md.push_str(&format!("| {} | `{}` | {} |\n", severity_emoji, issue.code, issue.message));
        }

        md.push_str("\n---\n\n");
    }

    // ========== FOOTER ==========
    md.push_str(&format!(
        r#"## About This Report

This report was automatically generated by **Watchy v{}**, an EIP-8004 agent auditing service.

### Limitations

- This audit checks **infrastructure only**, not agent behavior or output quality
- Endpoint tests verify **reachability and schema**, not functional correctness
- Security checks cover **transport layer** only, not application security
- Metadata validation checks **format**, not content truthfulness

### Learn More

- [EIP-8004 Specification](https://eips.ethereum.org/EIPS/eip-8004)
- [Watchy Documentation](https://github.com/anthropics/watchy)

---

*Report generated by Watchy - EIP-8004 Agent Audit Service*
"#,
        report.auditor.version
    ));

    md
}

fn score_emoji(score: u8) -> &'static str {
    match score {
        90..=100 => "🏆",
        75..=89 => "✅",
        60..=74 => "⚠️",
        40..=59 => "🟠",
        _ => "🔴",
    }
}

fn verdict_text(score: u8) -> &'static str {
    match score {
        90..=100 => "**Excellent** - Agent passes all critical checks",
        75..=89 => "**Good** - Agent passes most checks with minor issues",
        60..=74 => "**Fair** - Agent has issues that should be addressed",
        40..=59 => "**Poor** - Agent has significant problems",
        _ => "**Critical** - Agent fails multiple critical checks",
    }
}

fn verdict_explanation(score: u8) -> &'static str {
    match score {
        90..=100 => "This agent has excellent infrastructure. All endpoints are reachable, metadata is complete, and security basics are in place. Safe to interact with from an infrastructure standpoint.",
        75..=89 => "This agent has solid infrastructure with some minor issues. Consider reviewing the warnings below, but the agent is generally well-configured.",
        60..=74 => "This agent has noticeable issues that may affect reliability. Review the problems listed and consider whether they impact your use case.",
        40..=59 => "This agent has significant infrastructure problems. Proceed with caution and review all issues carefully.",
        _ => "This agent has critical problems that make it unreliable. We recommend against interacting with this agent until issues are resolved.",
    }
}

fn pass_fail(passed: bool) -> &'static str {
    if passed { "✅ Pass" } else { "❌ Fail" }
}

fn latency_rating(p95_ms: u64) -> &'static str {
    match p95_ms {
        0..=200 => "🟢 Excellent",
        201..=500 => "🟢 Good",
        501..=1000 => "🟡 Fair",
        1001..=2000 => "🟠 Slow",
        _ => "🔴 Very Slow",
    }
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, c);
    }
    result
}

/// Save markdown report to file
pub async fn save_report(
    report: &AuditReport,
    metadata: Option<&AgentMetadata>,
    reports_dir: &Path,
) -> Result<String, WatchyError> {
    // Ensure reports directory exists
    fs::create_dir_all(reports_dir)
        .await
        .map_err(|e| WatchyError::Internal(format!("Failed to create reports dir: {}", e)))?;

    let agent_name = metadata
        .and_then(|m| m.name.as_deref())
        .unwrap_or("unknown")
        .to_lowercase()
        .replace(' ', "-");

    let filename = format!("agent-{}-{}.md", report.agent.agent_id, agent_name);
    let filepath = reports_dir.join(&filename);

    let markdown = generate_markdown_report(report, metadata);

    fs::write(&filepath, &markdown)
        .await
        .map_err(|e| WatchyError::Internal(format!("Failed to write report: {}", e)))?;

    info!("Report saved to {}", filepath.display());

    Ok(filepath.to_string_lossy().to_string())
}
