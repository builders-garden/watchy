#!/bin/bash
#
# Test script for running a full audit flow
#
# Usage:
#   ./scripts/test-audit.sh <agent_id> [chain_id]
#
# Examples:
#   ./scripts/test-audit.sh 1                    # Audit agent 1 on default chain
#   ./scripts/test-audit.sh 42 84532             # Audit agent 42 on Base Sepolia
#   ./scripts/test-audit.sh 100 11155111         # Audit agent 100 on Ethereum Sepolia
#
# Chain IDs:
#   8453     - Base Mainnet
#   1        - Ethereum Mainnet
#   84532    - Base Sepolia (testnet)
#   11155111 - Ethereum Sepolia (testnet)

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
HOST="${WATCHY_HOST:-http://localhost:8080}"
DEFAULT_CHAIN_ID=84532  # Base Sepolia

# Parse arguments
AGENT_ID="${1:-}"
CHAIN_ID="${2:-$DEFAULT_CHAIN_ID}"

if [ -z "$AGENT_ID" ]; then
    echo -e "${RED}Error: agent_id is required${NC}"
    echo ""
    echo "Usage: $0 <agent_id> [chain_id]"
    echo ""
    echo "Examples:"
    echo "  $0 1              # Audit agent 1 on Base Sepolia"
    echo "  $0 42 84532       # Audit agent 42 on Base Sepolia"
    echo "  $0 100 8453       # Audit agent 100 on Base Mainnet"
    exit 1
fi

echo -e "${BLUE}=== Watchy Audit Test ===${NC}"
echo ""

# Step 1: Check health
echo -e "${YELLOW}[1/4] Checking Watchy health...${NC}"
HEALTH=$(curl -s "$HOST/health")
echo "$HEALTH" | jq .
echo ""

WALLET_MODE=$(echo "$HEALTH" | jq -r '.wallet_mode')
SIGNER=$(echo "$HEALTH" | jq -r '.signer_address // "none"')

if [ "$WALLET_MODE" = "none" ]; then
    echo -e "${YELLOW}Warning: No wallet configured. Arweave uploads and on-chain feedback will be skipped.${NC}"
    echo ""
fi

# Step 2: Request audit
echo -e "${YELLOW}[2/4] Requesting audit for agent $AGENT_ID on chain $CHAIN_ID...${NC}"
RESPONSE=$(curl -s -X POST "$HOST/audit" \
    -H "Content-Type: application/json" \
    -d "{\"agent_id\": $AGENT_ID, \"chain_id\": $CHAIN_ID}")

echo "$RESPONSE" | jq .
echo ""

AUDIT_ID=$(echo "$RESPONSE" | jq -r '.audit_id')

if [ "$AUDIT_ID" = "null" ] || [ -z "$AUDIT_ID" ]; then
    echo -e "${RED}Error: Failed to create audit${NC}"
    echo "$RESPONSE"
    exit 1
fi

echo -e "${GREEN}Audit created: $AUDIT_ID${NC}"
echo ""

# Step 3: Poll for completion
echo -e "${YELLOW}[3/4] Polling for completion...${NC}"
MAX_ATTEMPTS=60  # 2 minutes max
ATTEMPT=0

while [ $ATTEMPT -lt $MAX_ATTEMPTS ]; do
    STATUS_RESPONSE=$(curl -s "$HOST/audit/$AUDIT_ID")
    STATUS=$(echo "$STATUS_RESPONSE" | jq -r '.status')

    case "$STATUS" in
        "completed")
            echo -e "${GREEN}Audit completed!${NC}"
            echo ""
            echo "$STATUS_RESPONSE" | jq .
            break
            ;;
        "failed")
            echo -e "${RED}Audit failed!${NC}"
            echo "$STATUS_RESPONSE" | jq .
            exit 1
            ;;
        "pending"|"in_progress")
            echo -n "."
            sleep 2
            ATTEMPT=$((ATTEMPT + 1))
            ;;
        *)
            echo -e "${RED}Unknown status: $STATUS${NC}"
            echo "$STATUS_RESPONSE" | jq .
            exit 1
            ;;
    esac
done

if [ $ATTEMPT -ge $MAX_ATTEMPTS ]; then
    echo -e "${RED}Timeout waiting for audit completion${NC}"
    exit 1
fi

echo ""

# Step 4: Get full report
echo -e "${YELLOW}[4/4] Fetching full report...${NC}"
REPORT=$(curl -s "$HOST/audit/$AUDIT_ID/report")
echo "$REPORT" | jq .

echo ""
echo -e "${BLUE}=== Summary ===${NC}"
echo -e "Audit ID:    ${GREEN}$AUDIT_ID${NC}"
echo -e "Agent ID:    $AGENT_ID"
echo -e "Chain ID:    $CHAIN_ID"
echo -e "Overall:     $(echo "$REPORT" | jq -r '.scores.overall')%"
echo -e "Metadata:    $(echo "$REPORT" | jq -r '.scores.metadata')%"
echo -e "On-chain:    $(echo "$REPORT" | jq -r '.scores.onchain')%"
echo -e "Endpoints:   $(echo "$REPORT" | jq -r '.scores.endpoint_availability')%"
echo -e "Security:    $(echo "$REPORT" | jq -r '.scores.security')%"
echo ""

# Check for local report file
REPORT_FILE="reports/agent-${AGENT_ID}-audit-*.md"
if ls $REPORT_FILE 1> /dev/null 2>&1; then
    echo -e "Local report: ${GREEN}$(ls -t $REPORT_FILE | head -1)${NC}"
fi

echo ""
echo -e "${GREEN}Done!${NC}"
