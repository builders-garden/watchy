# Watchy API Specification

Base URL: `https://watchy.eigencloud.app` (or configured host)

## Endpoints

### Health Check

```
GET /health
```

**Response** `200 OK`
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "chain": {
    "id": 8453,
    "name": "base",
    "connected": true,
    "blockNumber": 12345678
  }
}
```

---

### Request Audit

```
POST /audit
```

**Request Body**
```json
{
  "agent_id": 1434,
  "registry": "0x8004A169FB4a3325136EB29fA0ceB6D2e539a432",
  "audit_type": "single",
  "callback_url": "https://example.com/webhook"  // optional
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `agent_id` | `uint256` | Yes | The agent's token ID in the registry |
| `registry` | `address` | Yes | The EIP-8004 registry contract address |
| `audit_type` | `string` | No | `"single"` (default) or `"recurring"` |
| `callback_url` | `string` | No | Webhook URL to POST results when complete |

**Response** `202 Accepted`
```json
{
  "audit_id": "aud_abc123def456",
  "status": "pending",
  "created_at": 1708180000,
  "estimated_completion": 1708180060
}
```

**Error Responses**

`400 Bad Request` - Invalid input
```json
{
  "error": "invalid_request",
  "message": "Invalid registry address format",
  "details": {
    "field": "registry",
    "value": "invalid"
  }
}
```

`429 Too Many Requests` - Rate limited
```json
{
  "error": "rate_limited",
  "message": "Too many audit requests for this agent",
  "retry_after": 3600
}
```

---

### Get Audit Status

```
GET /audit/:audit_id
```

**Response** `200 OK` (pending)
```json
{
  "audit_id": "aud_abc123def456",
  "status": "in_progress",
  "agent_id": 1434,
  "registry": "0x8004A169FB4a3325136EB29fA0ceB6D2e539a432",
  "created_at": 1708180000,
  "progress": {
    "phase": "endpoint_testing",
    "completed_steps": 3,
    "total_steps": 5
  }
}
```

**Response** `200 OK` (completed)
```json
{
  "audit_id": "aud_abc123def456",
  "status": "completed",
  "agent_id": 1434,
  "registry": "0x8004A169FB4a3325136EB29fA0ceB6D2e539a432",
  "created_at": 1708180000,
  "completed_at": 1708180045,
  "result": {
    "scores": {
      "overall": 85,
      "metadata": 90,
      "onchain": 100,
      "endpoint_availability": 80,
      "endpoint_performance": 70
    },
    "issues_count": {
      "critical": 0,
      "error": 1,
      "warning": 2,
      "info": 1
    },
    "ipfs_cid": "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
    "report_url": "https://ipfs.io/ipfs/bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
  }
}
```

**Response** `200 OK` (failed)
```json
{
  "audit_id": "aud_abc123def456",
  "status": "failed",
  "agent_id": 1434,
  "registry": "0x8004A169FB4a3325136EB29fA0ceB6D2e539a432",
  "created_at": 1708180000,
  "failed_at": 1708180030,
  "error": {
    "code": "AGENT_NOT_FOUND",
    "message": "Agent ID 1434 does not exist in registry"
  }
}
```

**Error Responses**

`404 Not Found`
```json
{
  "error": "not_found",
  "message": "Audit not found"
}
```

---

### Get Full Report

```
GET /audit/:audit_id/report
```

Returns the full audit report (same structure as uploaded to IPFS).

**Response** `200 OK`
```json
{
  "version": "1.0.0",
  "auditor": { ... },
  "timestamp": 1708180000,
  "agent": { ... },
  "scores": { ... },
  "checks": { ... },
  "signature": "0x..."
}
```

---

### List Audits for Agent

```
GET /agents/:registry/:agent_id/audits
```

**Query Parameters**
| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `limit` | `int` | 10 | Max results (1-100) |
| `offset` | `int` | 0 | Pagination offset |

**Response** `200 OK`
```json
{
  "agent_id": 1434,
  "registry": "0x8004A169FB4a3325136EB29fA0ceB6D2e539a432",
  "audits": [
    {
      "audit_id": "aud_abc123def456",
      "status": "completed",
      "created_at": 1708180000,
      "overall_score": 85
    },
    {
      "audit_id": "aud_xyz789ghi012",
      "status": "completed",
      "created_at": 1707575000,
      "overall_score": 78
    }
  ],
  "total": 5,
  "limit": 10,
  "offset": 0
}
```

---

## Webhook Callback

If `callback_url` is provided in the audit request, Watchy will POST the result when the audit completes.

**Webhook Payload**
```json
{
  "event": "audit.completed",
  "audit_id": "aud_abc123def456",
  "timestamp": 1708180045,
  "result": {
    "status": "completed",
    "scores": {
      "overall": 85,
      "metadata": 90,
      "onchain": 100,
      "endpoint_availability": 80,
      "endpoint_performance": 70
    },
    "ipfs_cid": "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
  }
}
```

**Webhook Signature**

All webhooks include a signature header for verification:
```
X-Watchy-Signature: sha256=abc123...
```

Computed as: `HMAC-SHA256(webhook_secret, request_body)`

---

## Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `invalid_request` | 400 | Malformed request body |
| `invalid_address` | 400 | Invalid Ethereum address |
| `invalid_agent_id` | 400 | Agent ID must be positive integer |
| `not_found` | 404 | Resource not found |
| `rate_limited` | 429 | Too many requests |
| `internal_error` | 500 | Server error |

---

## Rate Limits

| Endpoint | Limit |
|----------|-------|
| `POST /audit` | 10 requests per agent per hour |
| `GET /audit/*` | 100 requests per minute |
| `GET /health` | No limit |

---

## Status Values

| Status | Description |
|--------|-------------|
| `pending` | Audit queued, not started |
| `in_progress` | Audit running |
| `completed` | Audit finished successfully |
| `failed` | Audit failed (agent not found, etc.) |
