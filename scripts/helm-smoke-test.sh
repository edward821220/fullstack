#!/usr/bin/env bash
# Helm chart smoke test — verifies the chart renders without errors
# and produces expected Kubernetes resources.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CHART_DIR="$SCRIPT_DIR/../k8s/helm/fullstack-template"

echo "=== Helm smoke test ==="

pass() { echo "PASS"; }
fail() { echo "FAIL"; exit 1; }

# Test 1: Default values render
echo -n "[1/4] Default values render... "
helm template test-release "$CHART_DIR" > /dev/null 2>&1 && pass || fail

# Test 2: Manual OIDC discovery mode
echo -n "[2/4] Manual discovery mode... "
helm template test-release "$CHART_DIR" \
    --set env.authDiscoveryMode=manual \
    --set env.authManualJwksUri=https://idp.bank.com/keys \
    --set env.authManualIssuer=https://idp.bank.com \
    > /dev/null 2>&1 && pass || fail

# Test 3: Auth disabled produces false value
echo -n "[3/4] Auth disabled (boolean)... "
VAL=$(helm template test-release "$CHART_DIR" --set env.authEnabled=false 2>&1 | grep "APP_AUTH__ENABLED" -A1 | tail -1)
echo "$VAL" | grep -q "false" && pass || fail

# Test 4: Custom audience
echo -n "[4/4] Custom audience... "
helm template test-release "$CHART_DIR" \
    --set-string 'env.authAudience={aud1,aud2}' \
    > /dev/null 2>&1 && pass || fail

echo
echo "=== Helm smoke test PASSED ==="
