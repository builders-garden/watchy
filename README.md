# Watchy

EIP-8004 Agent Watchtower Service - Automated audits for on-chain AI agents.

## Overview

Watchy is a Rust service that audits AI agents registered under [EIP-8004](https://eips.ethereum.org/EIPS/eip-8004). It validates on-chain registration, metadata compliance, endpoint availability, and submits reputation feedback.

```
┌─────────────────────────────────────────────────────────────────┐
│                         Watchy Flow                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  POST /audit { agent_id: 17 }                                   │
│         │                                                        │
│         ▼                                                        │
│  ┌─────────────────┐                                            │
│  │  Audit Engine   │                                            │
│  │  ├─ On-chain    │ ◄── Query registry, verify registration    │
│  │  ├─ Metadata    │ ◄── Fetch & validate metadata JSON         │
│  │  ├─ Endpoints   │ ◄── Test availability & performance        │
│  │  └─ Security    │ ◄── Check for common issues                │
│  └────────┬────────┘                                            │
│           │                                                      │
│           ▼                                                      │
│  ┌─────────────────┐                                            │
│  │  Report Output  │                                            │
│  │  ├─ Arweave     │ ◄── Permanent storage (MD + JSON)          │
│  │  ├─ On-chain    │ ◄── Reputation registry feedback           │
│  │  └─ Redis       │ ◄── Job status & results cache             │
│  └─────────────────┘                                            │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Features

- **On-chain Validation** - Verifies agent registration in EIP-8004 registry
- **Metadata Compliance** - Validates metadata JSON against schema
- **Endpoint Testing** - Checks availability, response time, SSL/TLS
- **Arweave Storage** - Permanent report storage via Turbo/Irys
- **Reputation Feedback** - Submits scores to on-chain reputation registry
- **Multi-chain Support** - Base, Ethereum, and testnets
- **TEE Ready** - Supports EigenCloud mnemonic injection

## Supported Chains

| Chain | ID | Registry | Reputation |
|-------|-----|----------|------------|
| Base | 8453 | `0x8004...` | `0x...` |
| Base Sepolia | 84532 | `0x8004...` | `0x...` |
| Ethereum | 1 | - | - |
| Sepolia | 11155111 | - | - |

## Quick Start

### Local Development

```bash
# Clone and setup
git clone https://github.com/your-org/watchy.git
cd watchy
cp .env.example .env

# Edit .env with your config
# At minimum, set PRIVATE_KEY for signing

# Run
cargo run

# Test
curl http://localhost:8080/health
```

### Docker

```bash
# Build
docker build -t watchy .

# Run
docker run -p 8080:8080 \
  -e PRIVATE_KEY=0x... \
  -e DEFAULT_CHAIN_ID=8453 \
  -e REDIS_URL=redis://host:6379 \
  watchy
```

## API

### Health Check

```http
GET /health
```

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "supported_chains": [8453, 84532, 1, 11155111],
  "default_chain": 8453,
  "storage": "redis",
  "wallet_mode": "private_key",
  "signer_address": "0x..."
}
```

### Request Audit

```http
POST /audit
Content-Type: application/json
X-API-Key: <your-api-key>

{
  "agent_id": 17,
  "chain_id": 8453
}
```

**Response (202 Accepted):**
```json
{
  "audit_id": "aud_7e07f2720d634c1c82f77279c3737820",
  "chain_id": 8453,
  "chain_name": "Base",
  "status": "pending",
  "created_at": 1737123456,
  "estimated_completion": 1737123486
}
```

### Get Audit Status

```http
GET /audit/:audit_id
X-API-Key: <your-api-key>
```

**Response:**
```json
{
  "audit_id": "aud_...",
  "agent_id": 17,
  "status": "completed",
  "created_at": 1737123456,
  "completed_at": 1737123486,
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
      "info": 5
    }
  }
}
```

Status values: `pending` | `in_progress` | `completed` | `failed`

### Get Full Report

```http
GET /audit/:audit_id/report
X-API-Key: <your-api-key>
```

Returns complete audit report JSON (only when `completed`).

## Configuration

### Environment Variables

```bash
# Server
PORT=8080                      # HTTP port (default: 8080)

# Chain
DEFAULT_CHAIN_ID=8453          # Default chain (default: 8453 Base)

# Storage
REDIS_URL=redis://localhost    # Optional, falls back to in-memory

# Authentication
API_KEY=your-secret            # Optional, enables X-API-Key auth

# Wallet (choose one)
PRIVATE_KEY=0x...              # Direct private key
# OR
MNEMONIC=word1 word2 ...       # BIP-39 mnemonic (EigenCloud)
DERIVATION_INDEX=0             # HD derivation index (default: 0)

# Logging
RUST_LOG=info,watchy=debug
```

### Wallet Modes

Watchy auto-detects the wallet mode:

| Mode | Env Var | Use Case |
|------|---------|----------|
| `private_key` | `PRIVATE_KEY` | Traditional deployment |
| `mnemonic` | `MNEMONIC` | EigenCloud TEE |
| `none` | Neither | Read-only (no signing) |

## Architecture

```
src/
├── main.rs              # Entry point, server setup
├── config.rs            # Environment configuration
├── wallet.rs            # Key management (PRIVATE_KEY / MNEMONIC)
├── store.rs             # Redis + in-memory job storage
├── chains.rs            # Multi-chain configuration
├── api/
│   ├── handlers.rs      # HTTP request handlers
│   ├── routes.rs        # Route definitions
│   └── middleware.rs    # API key authentication
├── audit/
│   ├── engine.rs        # Audit orchestration
│   ├── onchain.rs       # Registry validation
│   ├── metadata.rs      # Metadata fetching & validation
│   ├── endpoints.rs     # Endpoint availability testing
│   ├── security.rs      # Security checks
│   ├── content.rs       # Content analysis
│   └── report.rs        # Report generation
├── blockchain/
│   ├── registry.rs      # EIP-8004 registry client
│   └── reputation.rs    # Reputation registry client
├── arweave/
│   └── irys.rs          # Turbo/Irys uploads (ANS-104)
└── types/
    ├── audit.rs         # Audit types & report structure
    ├── metadata.rs      # Metadata schema types
    └── errors.rs        # Error definitions
```

## Deployment

### Docker Compose

```yaml
version: "3.8"
services:
  watchy:
    build: .
    ports:
      - "8080:8080"
    environment:
      - PRIVATE_KEY=${PRIVATE_KEY}
      - DEFAULT_CHAIN_ID=8453
      - REDIS_URL=redis://redis:6379
      - API_KEY=${API_KEY}
      - RUST_LOG=info,watchy=debug
    depends_on:
      - redis

  redis:
    image: redis:7-alpine
    volumes:
      - redis-data:/data

volumes:
  redis-data:
```

### EigenCloud TEE

Watchy is TEE-ready with mnemonic support:

```bash
# EigenCloud auto-injects MNEMONIC from KMS
ecloud compute app deploy your-registry/watchy:latest \
  --env DEFAULT_CHAIN_ID=8453 \
  --env REDIS_URL=redis://... \
  --env API_KEY=your-secret \
  --env APP_PORT=8080
```

The service automatically:
1. Detects `MNEMONIC` environment variable
2. Derives private key using BIP-39/BIP-44
3. Initializes wallet for signing

### EigenCloud TLS (HTTPS)

For production with a custom domain:

```bash
# 1. Configure TLS (creates Caddyfile)
ecloud compute app configure tls

# 2. Add to .env
DOMAIN=watchy.yourdomain.com
APP_PORT=8080
ACME_STAGING=true        # Test with staging certs first
ENABLE_CADDY_LOGS=true   # Debug logs

# 3. Set DNS A record pointing to instance IP
ecloud compute app info  # Get IP address

# 4. Deploy
ecloud compute app upgrade

# 5. Switch to production certs (after testing)
ACME_STAGING=false
ACME_FORCE_ISSUE=true    # One-time flag
ecloud compute app upgrade
```

Caddy handles Let's Encrypt certificates automatically and proxies 80/443 to `APP_PORT`.

### Production Checklist

- [ ] Set `API_KEY` for authentication
- [ ] Configure `REDIS_URL` for persistence
- [ ] Set `PRIVATE_KEY` or deploy to EigenCloud for `MNEMONIC`
- [ ] Use reverse proxy (nginx/caddy) for SSL termination
- [ ] Configure monitoring on `/health` endpoint

## Audit Scores

Reports include scores from 0-100:

| Score | Description |
|-------|-------------|
| `overall` | Weighted average of all scores |
| `metadata` | Metadata schema compliance |
| `onchain` | On-chain registration validity |
| `endpoint_availability` | Endpoint uptime & reachability |
| `endpoint_performance` | Response time & throughput |

## Report Storage

Completed audits are stored in:

1. **Redis** - Job status and results (7-day TTL)
2. **Arweave** - Permanent storage via Turbo
   - Markdown report (`text/markdown`)
   - Signed JSON report (`application/json`)
3. **On-chain** - Reputation feedback submitted to registry

## Integration with Servex

Watchy is designed to work with [Servex](../servex) for payment-protected access:

```
User ──x402 payment──► Servex ──X-API-Key──► Watchy
```

See [servex/docs/watchy-integration.md](../servex/docs/watchy-integration.md) for integration guide.

## Development

```bash
# Run with auto-reload
cargo watch -x run

# Run tests
cargo test

# Check formatting
cargo fmt --check

# Lint
cargo clippy
```

## License

MIT
