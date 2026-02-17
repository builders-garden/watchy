# Watchy - EIP-8004 Agent Watchtower Service

## Project Overview

Watchy is a watchtower service for [EIP-8004](https://eips.ethereum.org/EIPS/eip-8004) AI agents. It provides auditing services that verify agent metadata compliance, on-chain registration validity, and endpoint functionality. Audit reports are uploaded to IPFS and reputation feedback is submitted to the 8004 reputation registry.

## Tech Stack

- **Language**: Rust
- **Web Framework**: Axum
- **Blockchain**: ethers-rs / alloy for Base (chainId 8453)
- **IPFS**: HTTP API client (Pinata/Infura/local node)
- **Async Runtime**: Tokio
- **Serialization**: Serde
- **Deployment**: Docker on EigenCloud (TEE environment)

## Core Features

1. **REST API** - Receive audit requests from agents
2. **Metadata Validation** - Validate off-chain JSON against EIP-8004 schema
3. **On-chain Verification** - Verify agent registration on Base
4. **Endpoint Testing** - Probe and validate A2A/MCP/OASF endpoints
5. **IPFS Upload** - Pin audit reports to IPFS
6. **Reputation Submission** - Submit scores to reputation registry

## Project Structure

```
watchy/
├── Dockerfile
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, server setup
│   ├── config.rs            # Configuration management
│   ├── api/
│   │   ├── mod.rs
│   │   ├── routes.rs        # API route definitions
│   │   └── handlers.rs      # Request handlers
│   ├── audit/
│   │   ├── mod.rs
│   │   ├── engine.rs        # Main audit orchestration
│   │   ├── metadata.rs      # Metadata validation
│   │   ├── onchain.rs       # On-chain verification
│   │   └── endpoints.rs     # Endpoint testing
│   ├── services/
│   │   ├── mod.rs
│   │   ├── a2a.rs           # A2A protocol validation
│   │   ├── mcp.rs           # MCP protocol validation
│   │   └── oasf.rs          # OASF protocol validation
│   ├── blockchain/
│   │   ├── mod.rs
│   │   ├── registry.rs      # 8004 registry interactions
│   │   └── reputation.rs    # Reputation registry interactions
│   ├── ipfs/
│   │   ├── mod.rs
│   │   └── client.rs        # IPFS upload client
│   └── types/
│       ├── mod.rs
│       ├── metadata.rs      # EIP-8004 metadata types
│       ├── audit.rs         # Audit report types
│       └── errors.rs        # Error types
└── tests/
    └── ...
```

## Development

### Setup

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build
cargo build

# Run tests
cargo test
```

### Running

```bash
# Development
cargo run

# With environment variables
RPC_URL=https://mainnet.base.org \
IPFS_API_URL=https://api.pinata.cloud \
cargo run
```

### Testing

```bash
cargo test
cargo test --lib          # Unit tests only
cargo test --test '*'     # Integration tests only
```

### Docker

```bash
docker build -t watchy .
docker run -p 8080:8080 watchy
```

## Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `RPC_URL` | Base RPC endpoint | Yes |
| `REGISTRY_ADDRESS` | EIP-8004 registry contract address | Yes |
| `REPUTATION_ADDRESS` | Reputation registry contract address | Yes |
| `IPFS_API_URL` | IPFS HTTP API endpoint | Yes |
| `IPFS_API_KEY` | IPFS API key (if using Pinata/Infura) | No |
| `PRIVATE_KEY` | Wallet private key for reputation submission | Yes |
| `PORT` | Server port (default: 8080) | No |

## API Endpoints

- `POST /audit` - Request a new audit
- `GET /audit/:id` - Get audit status/result
- `GET /health` - Health check

## Coding Conventions

- Use `thiserror` for error types
- Use `tracing` for logging
- Async everywhere with Tokio
- Validate all external input
- Use strong types for blockchain addresses and IDs
