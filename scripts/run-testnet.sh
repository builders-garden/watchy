#!/bin/bash
#
# Run Watchy with Base Sepolia testnet configuration
#
# Usage:
#   ./scripts/run-testnet.sh
#
# Environment variables (set before running or in .env):
#   PRIVATE_KEY  - For signing reports and on-chain feedback (optional)
#   MNEMONIC     - Alternative to PRIVATE_KEY (for EigenCloud)
#   REDIS_URL    - For job persistence (optional, defaults to in-memory)

set -e

# Load .env if exists
if [ -f .env ]; then
    echo "Loading .env..."
    export $(grep -v '^#' .env | xargs)
fi

# Set testnet defaults
export DEFAULT_CHAIN_ID="${DEFAULT_CHAIN_ID:-84532}"  # Base Sepolia
export PORT="${PORT:-8080}"
export RUST_LOG="${RUST_LOG:-info,watchy=debug}"

echo "=== Watchy Testnet Mode ==="
echo ""
echo "Configuration:"
echo "  Chain:    Base Sepolia ($DEFAULT_CHAIN_ID)"
echo "  Port:     $PORT"
echo "  Redis:    ${REDIS_URL:-in-memory}"
echo "  Wallet:   ${KEY_MODE:-auto-detect}"
echo ""

# Check if private key is set
if [ -z "$PRIVATE_KEY" ] && [ -z "$MNEMONIC" ]; then
    echo "Warning: No PRIVATE_KEY or MNEMONIC set."
    echo "         Arweave uploads and on-chain feedback will be disabled."
    echo ""
fi

echo "Starting Watchy..."
echo ""

cargo run
