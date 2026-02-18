use native_tls::TlsConnector;
use std::net::ToSocketAddrs;
use tracing::{debug, warn};

use crate::types::{Issue, SecurityChecks, SecurityHeadersCheck, Severity};

/// Run security checks on an endpoint
pub async fn check_endpoint_security(
    client: &reqwest::Client,
    endpoint: &str,
) -> SecurityChecks {
    debug!("Running security checks on {}", endpoint);

    let mut checks = SecurityChecks {
        passed: true,
        tls_valid: false,
        tls_version: None,
        certificate_valid: false,
        certificate_days_remaining: None,
        security_headers: SecurityHeadersCheck::default(),
        https_enforced: false,
        issues: vec![],
    };

    // Skip non-HTTPS endpoints
    if !endpoint.starts_with("https://") {
        checks.passed = false;
        checks.issues.push(Issue {
            severity: Severity::Critical,
            code: "NO_HTTPS".to_string(),
            message: "Endpoint does not use HTTPS".to_string(),
        });
        return checks;
    }

    // Test TLS connection and get certificate info
    match check_tls(endpoint).await {
        Ok(tls_info) => {
            checks.tls_valid = tls_info.valid;
            checks.tls_version = Some(tls_info.version);
            checks.certificate_valid = tls_info.cert_valid;
            checks.certificate_days_remaining = tls_info.cert_days_remaining;

            if !tls_info.valid {
                checks.passed = false;
                checks.issues.push(Issue {
                    severity: Severity::Critical,
                    code: "TLS_INVALID".to_string(),
                    message: "TLS connection failed or invalid".to_string(),
                });
            }

            // Note: Actual TLS version detection would require rustls/openssl bindings
            // Modern clients (including reqwest) negotiate TLS 1.2+ by default

            if let Some(days) = tls_info.cert_days_remaining {
                if days <= 0 {
                    checks.passed = false;
                    checks.issues.push(Issue {
                        severity: Severity::Critical,
                        code: "CERT_EXPIRED".to_string(),
                        message: "TLS certificate has expired".to_string(),
                    });
                } else if days <= 14 {
                    checks.issues.push(Issue {
                        severity: Severity::Warning,
                        code: "CERT_EXPIRING_SOON".to_string(),
                        message: format!("TLS certificate expires in {} days", days),
                    });
                }
            }
        }
        Err(e) => {
            checks.passed = false;
            checks.issues.push(Issue {
                severity: Severity::Critical,
                code: "TLS_CHECK_FAILED".to_string(),
                message: format!("Failed to check TLS: {}", e),
            });
        }
    }

    // Check security headers
    checks.security_headers = check_security_headers(client, endpoint).await;
    if !has_minimum_headers(&checks.security_headers) {
        checks.issues.push(Issue {
            severity: Severity::Warning,
            code: "MISSING_SECURITY_HEADERS".to_string(),
            message: "Missing recommended security headers".to_string(),
        });
    }

    // Check HTTPS enforcement (try HTTP, should redirect or fail)
    checks.https_enforced = check_https_enforcement(client, endpoint).await;
    if !checks.https_enforced {
        checks.issues.push(Issue {
            severity: Severity::Info,
            code: "HTTP_NOT_REDIRECTED".to_string(),
            message: "HTTP requests are not redirected to HTTPS".to_string(),
        });
    }

    checks
}

struct TlsInfo {
    valid: bool,
    version: String,
    cert_valid: bool,
    cert_days_remaining: Option<i64>,
}

async fn check_tls(endpoint: &str) -> Result<TlsInfo, String> {
    // Parse the URL to get host and port
    let url = url::Url::parse(endpoint).map_err(|e| format!("Invalid URL: {}", e))?;
    let host = url.host_str().ok_or("No host in URL")?;
    let port = url.port().unwrap_or(443);

    // Check if we can establish a valid TLS connection (cert must be valid)
    let cert_valid = check_certificate_valid(host, port).await;

    // Also do a standard TLS check with reqwest to verify connectivity
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(false)
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    match client.head(endpoint).send().await {
        Ok(_response) => {
            // TLS handshake succeeded - connection is secure
            Ok(TlsInfo {
                valid: true,
                version: "TLS 1.2+".to_string(),
                cert_valid,
                cert_days_remaining: None, // Would require x509 parsing
            })
        }
        Err(e) => {
            if e.is_connect() {
                // Could be cert error or connection refused
                Err(format!("Connection/TLS error: {}", e))
            } else {
                // Request failed but TLS handshake may have succeeded
                Ok(TlsInfo {
                    valid: true,
                    version: "TLS 1.2+".to_string(),
                    cert_valid,
                    cert_days_remaining: None,
                })
            }
        }
    }
}

/// Check if the certificate is valid (not expired, trusted)
async fn check_certificate_valid(host: &str, port: u16) -> bool {
    let host = host.to_string();
    tokio::task::spawn_blocking(move || {
        check_cert_valid_sync(&host, port)
    })
    .await
    .unwrap_or(false)
}

/// Synchronous certificate validity check
fn check_cert_valid_sync(host: &str, port: u16) -> bool {
    // Build TLS connector with strict validation
    let connector = match TlsConnector::builder()
        .danger_accept_invalid_certs(false) // Strict validation
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    // Resolve address
    let addr = match format!("{}:{}", host, port).to_socket_addrs() {
        Ok(mut addrs) => match addrs.next() {
            Some(a) => a,
            None => return false,
        },
        Err(_) => return false,
    };

    // Connect with timeout
    let stream = match std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(5)) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(5)));

    // TLS handshake - if this succeeds, cert is valid
    connector.connect(host, stream).is_ok()
}

fn has_minimum_headers(headers: &SecurityHeadersCheck) -> bool {
    // At minimum, should have X-Content-Type-Options or HSTS
    headers.x_content_type_options || headers.strict_transport_security
}

fn headers_score(headers: &SecurityHeadersCheck) -> u8 {
    let mut score = 0u8;
    if headers.x_content_type_options { score += 20; }
    if headers.x_frame_options { score += 20; }
    if headers.strict_transport_security { score += 30; }
    if headers.content_security_policy { score += 20; }
    if headers.x_xss_protection { score += 10; }
    score
}

async fn check_security_headers(client: &reqwest::Client, endpoint: &str) -> SecurityHeadersCheck {
    let mut headers_check = SecurityHeadersCheck::default();

    match client.head(endpoint).send().await {
        Ok(response) => {
            let headers = response.headers();

            headers_check.x_content_type_options = headers
                .get("x-content-type-options")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.to_lowercase().contains("nosniff"))
                .unwrap_or(false);

            headers_check.x_frame_options = headers
                .get("x-frame-options")
                .is_some();

            headers_check.strict_transport_security = headers
                .get("strict-transport-security")
                .is_some();

            headers_check.content_security_policy = headers
                .get("content-security-policy")
                .is_some();

            headers_check.x_xss_protection = headers
                .get("x-xss-protection")
                .is_some();
        }
        Err(e) => {
            warn!("Failed to check security headers: {}", e);
        }
    }

    headers_check
}

async fn check_https_enforcement(client: &reqwest::Client, endpoint: &str) -> bool {
    // Convert https:// to http:// and check if it redirects
    let http_endpoint = endpoint.replace("https://", "http://");

    // Build a client that doesn't follow redirects
    let no_redirect_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap_or_else(|_| client.clone());

    match no_redirect_client.head(&http_endpoint).send().await {
        Ok(response) => {
            // Check if it's a redirect to HTTPS
            if response.status().is_redirection() {
                if let Some(location) = response.headers().get("location") {
                    if let Ok(loc) = location.to_str() {
                        return loc.starts_with("https://");
                    }
                }
            }
            // If server refuses HTTP connection, that's also good
            false
        }
        Err(_) => {
            // Connection refused on HTTP is actually good - means HTTPS only
            true
        }
    }
}

/// Calculate security score from checks
pub fn calculate_security_score(checks: &SecurityChecks) -> u8 {
    let mut score = 100u8;

    // TLS validity is critical
    if !checks.tls_valid {
        return 0;
    }

    // Certificate issues
    if !checks.certificate_valid {
        score = score.saturating_sub(50);
    }

    if let Some(days) = checks.certificate_days_remaining {
        if days <= 0 {
            score = score.saturating_sub(50);
        } else if days <= 7 {
            score = score.saturating_sub(20);
        } else if days <= 14 {
            score = score.saturating_sub(10);
        }
    }

    // Security headers (max 30 points deduction)
    let h_score = headers_score(&checks.security_headers);
    let headers_penalty = ((100 - h_score) as f64 * 0.3) as u8;
    score = score.saturating_sub(headers_penalty);

    // HTTPS enforcement
    if !checks.https_enforced {
        score = score.saturating_sub(10);
    }

    score
}
