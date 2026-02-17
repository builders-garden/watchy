# Watchy Architecture

## Overview

Watchy is an auditing service for EIP-8004 AI agents deployed on Base. It validates agent metadata, verifies on-chain registration, tests service endpoints, and publishes audit reports to IPFS with reputation feedback on-chain.

## System Architecture

```
                                    ┌─────────────────┐
                                    │   Agent (8004)  │
                                    │                 │
                                    │ Requests audit  │
                                    └────────┬────────┘
                                             │
                                             ▼
┌────────────────────────────────────────────────────────────────────────────┐
│                              WATCHY SERVICE                                 │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                           REST API (Axum)                             │  │
│  │   POST /audit      GET /audit/:id      GET /health                   │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                    │                                        │
│                                    ▼                                        │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                         AUDIT ENGINE                                  │  │
│  │                                                                       │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │  │
│  │  │  Metadata   │  │  On-chain   │  │  Endpoint   │  │   Report    │  │  │
│  │  │  Validator  │  │  Verifier   │  │   Tester    │  │  Generator  │  │  │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘  │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                    │                                        │
│         ┌──────────────────────────┼──────────────────────────┐            │
│         ▼                          ▼                          ▼            │
│  ┌─────────────┐          ┌─────────────┐          ┌─────────────┐         │
│  │    IPFS     │          │    Base     │          │   Agent     │         │
│  │   Client    │          │    RPC      │          │  Endpoints  │         │
│  └─────────────┘          └─────────────┘          └─────────────┘         │
└────────────────────────────────────────────────────────────────────────────┘
         │                          │
         ▼                          ▼
┌─────────────────┐        ┌─────────────────┐
│      IPFS       │        │  Base Blockchain │
│  (Pinata/etc)   │        │   - 8004 Registry│
│                 │        │   - Reputation   │
└─────────────────┘        └─────────────────┘
```

## Audit Process

### Phase 1: Request Intake

```rust
// POST /audit request body
{
  "agent_id": 1434,
  "registry": "0x8004A169FB4a3325136EB29fA0ceB6D2e539a432",
  "audit_type": "single"  // or "recurring"
}
```

### Phase 2: On-chain Data Fetch

1. Call `registry.ownerOf(agentId)` - verify agent exists
2. Call `registry.tokenURI(agentId)` or custom `agentURI(agentId)` - get metadata URI
3. Call `registry.getAgentWallet(agentId)` - get payment wallet
4. Store chain ID, block number for audit reference

### Phase 3: Metadata Fetch & Validation

Fetch JSON from `agentURI` and validate:

#### Required Fields (Critical)
| Field | Validation |
|-------|------------|
| `type` | Must equal `https://eips.ethereum.org/EIPS/eip-8004#registration-v1` |
| `name` | Non-empty string, max 256 chars |
| `description` | Non-empty string, max 2048 chars |
| `image` | Valid URL, must be accessible, valid image MIME type |
| `registrations` | Non-empty array |
| `registrations[].agentId` | Must match requested agentId |
| `registrations[].agentRegistry` | Format `eip155:{chainId}:{address}`, must match |

#### Recommended Fields (Warning if missing)
| Field | Validation |
|-------|------------|
| `active` | Boolean |
| `services` | Non-empty array with at least one service |
| `supportedTrust` | Array of valid trust types |
| `updatedAt` | Unix timestamp, not in future |

#### Service Validation
| Service Type | Required Fields | Validation |
|--------------|-----------------|------------|
| `A2A` | `endpoint`, `version` | Valid URL, version semver-ish |
| `MCP` | `endpoint`, `version` | Valid URL, version date format |
| `OASF` | `endpoint`, `version` | Valid URL |
| `web` | `endpoint` | Valid URL |

### Phase 4: Endpoint Testing

For each service endpoint:

#### A2A Endpoints
1. Fetch `{endpoint}` (should be agent-card.json location)
2. Validate A2A Agent Card schema:
   ```json
   {
     "name": "string",
     "description": "string",
     "skills": [...],
     "capabilities": {...}
   }
   ```
3. Check declared `a2aSkills` match card's skills
4. Measure latency (3 requests, calculate p50/p95)

#### MCP Endpoints
1. Fetch endpoint or well-known location
2. Validate MCP manifest structure:
   ```json
   {
     "name": "string",
     "version": "string",
     "tools": [...],
     "prompts": [...]
   }
   ```
3. Check declared `mcpTools`/`mcpPrompts` exist in manifest
4. Measure latency

#### OASF Endpoints
1. Fetch OASF descriptor
2. Validate skills/domains format (can be string or object with name/id)
3. Check consistency with metadata declaration

#### Web Endpoints
1. HTTP HEAD/GET request
2. Check TLS certificate validity
3. Check response status (2xx expected)
4. Measure latency

### Phase 5: Scoring

#### Score Categories (0-100 each)

**Metadata Score**
| Check | Weight | Points |
|-------|--------|--------|
| Required fields present | 40% | 0 or 40 |
| Type field correct | 20% | 0 or 20 |
| URLs valid & accessible | 20% | 0-20 |
| Recommended fields present | 10% | 0-10 |
| No format errors | 10% | 0-10 |

**On-chain Score**
| Check | Weight | Points |
|-------|--------|--------|
| Agent exists | 40% | 0 or 40 |
| URI matches | 30% | 0 or 30 |
| Wallet set | 20% | 0 or 20 |
| Registration consistency | 10% | 0 or 10 |

**Endpoint Availability Score**
| Check | Weight | Points |
|-------|--------|--------|
| Per endpoint: reachable | 60% | proportional |
| Per endpoint: valid response | 40% | proportional |

**Endpoint Performance Score**
| Latency (p95) | Points |
|---------------|--------|
| < 200ms | 100 |
| < 500ms | 80 |
| < 1000ms | 60 |
| < 2000ms | 40 |
| < 5000ms | 20 |
| >= 5000ms | 0 |

**Overall Score**
```
overall = (metadata * 0.30) + (onchain * 0.25) + (availability * 0.25) + (performance * 0.20)
```

### Phase 6: Report Generation

```json
{
  "version": "1.0.0",
  "auditor": {
    "name": "watchy",
    "address": "0x...",
    "version": "0.1.0"
  },
  "timestamp": 1708180000,
  "blockNumber": 12345678,
  "agent": {
    "agentId": 1434,
    "registry": "eip155:8453:0x8004A169FB4a3325136EB29fA0ceB6D2e539a432",
    "metadataURI": "https://example.com/agent.json",
    "owner": "0x..."
  },
  "scores": {
    "overall": 85,
    "metadata": 90,
    "onchain": 100,
    "endpointAvailability": 80,
    "endpointPerformance": 70
  },
  "checks": {
    "metadata": {
      "passed": true,
      "requiredFields": { "passed": true, "details": {} },
      "typeField": { "passed": true },
      "urlsValid": { "passed": true, "details": {} },
      "recommendedFields": {
        "passed": false,
        "missing": ["updatedAt"]
      },
      "issues": [
        {
          "severity": "warning",
          "code": "MISSING_UPDATED_AT",
          "message": "Recommended field 'updatedAt' is missing"
        }
      ]
    },
    "onchain": {
      "passed": true,
      "agentExists": true,
      "uriMatches": true,
      "walletSet": true,
      "issues": []
    },
    "endpoints": [
      {
        "service": "A2A",
        "endpoint": "https://example.com/.well-known/agent-card.json",
        "reachable": true,
        "validSchema": true,
        "skillsMatch": true,
        "latency": {
          "p50": 120,
          "p95": 340,
          "p99": 890
        },
        "issues": []
      },
      {
        "service": "MCP",
        "endpoint": "https://example.com/mcp",
        "reachable": false,
        "error": "Connection timeout",
        "issues": [
          {
            "severity": "critical",
            "code": "ENDPOINT_UNREACHABLE",
            "message": "MCP endpoint is not reachable"
          }
        ]
      }
    ]
  },
  "signature": "0x..."
}
```

### Phase 7: IPFS Upload

1. Serialize report to JSON
2. Upload to IPFS via configured provider
3. Return CID (e.g., `bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi`)

### Phase 8: Reputation Submission

Submit to reputation registry (when payment is implemented):
- `agentId`: The audited agent
- `score`: Overall score (0-100, normalized to contract's valueDecimals)
- `reportCID`: IPFS CID of full report

## Issue Severity Levels

| Severity | Description | Score Impact |
|----------|-------------|--------------|
| `critical` | Agent fails core requirements | Score = 0 for category |
| `error` | Significant issue affecting functionality | -20 to -40 points |
| `warning` | Missing recommended fields or minor issues | -5 to -15 points |
| `info` | Suggestions for improvement | No impact |

## Issue Codes

### Metadata Issues
- `MISSING_TYPE` - Missing type field (critical)
- `INVALID_TYPE` - Type doesn't match EIP-8004 (critical)
- `MISSING_NAME` - Missing name field (critical)
- `MISSING_DESCRIPTION` - Missing description (critical)
- `MISSING_IMAGE` - Missing image field (critical)
- `INVALID_IMAGE_URL` - Image URL not accessible (error)
- `MISSING_REGISTRATIONS` - Missing registrations array (critical)
- `REGISTRATION_MISMATCH` - Registration doesn't match request (critical)
- `MISSING_SERVICES` - No services declared (warning)
- `MISSING_ACTIVE` - Missing active field (warning)
- `MISSING_UPDATED_AT` - Missing updatedAt (info)
- `INCONSISTENT_CASING` - Field casing inconsistency (info)

### On-chain Issues
- `AGENT_NOT_FOUND` - Agent ID doesn't exist (critical)
- `URI_MISMATCH` - On-chain URI differs from fetched (error)
- `NO_WALLET` - Agent wallet not set (warning)

### Endpoint Issues
- `ENDPOINT_UNREACHABLE` - Cannot connect to endpoint (critical)
- `INVALID_RESPONSE` - Response not valid JSON (error)
- `SCHEMA_MISMATCH` - Response doesn't match expected schema (error)
- `SKILLS_MISMATCH` - Declared skills don't match actual (warning)
- `HIGH_LATENCY` - Response time > 2s (warning)
- `TLS_ERROR` - TLS certificate issue (error)

## Recurring Audits

For recurring audits:
1. Store audit schedule in local state/DB
2. Run audit at configured interval
3. Compare with previous audit
4. Track trend (improving/degrading)
5. Alert on significant score drops

## Security Considerations

1. **Rate Limiting**: Limit requests per agent to prevent abuse
2. **Timeout**: All HTTP requests have 10s timeout
3. **Input Validation**: Validate all input addresses and IDs
4. **No Secret Exposure**: Never log private keys or sensitive data
5. **TEE Attestation**: Running on EigenCloud provides TEE guarantees
