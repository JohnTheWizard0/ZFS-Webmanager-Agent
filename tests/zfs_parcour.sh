#!/bin/bash
# ==============================================================================
# ZFS Feature Parcour - End-to-End Integration Test
# ==============================================================================
# Tests all implemented ZFS features in logical order against a running server.
#
# REQUIREMENTS:
#   - ZFS installed and loaded (modprobe zfs)
#   - Loop devices or spare disks for test pool
#   - Server running on localhost:3000 (or specify API_URL)
#   - Valid API key (or specify API_KEY)
#
# USAGE:
#   ./tests/zfs_parcour.sh                    # Use defaults
#   API_URL=http://host:3000 ./tests/zfs_parcour.sh
#   API_KEY=your-key ./tests/zfs_parcour.sh
#   CLEANUP=false ./tests/zfs_parcour.sh      # Keep test pool after run
#
# EXIT CODES:
#   0 - All tests passed
#   1 - Test failed
#   2 - Prerequisites not met
# ==============================================================================

set -euo pipefail

# Configuration
API_URL="${API_URL:-http://localhost:3000}"
API_KEY="${API_KEY:-}"
CLEANUP="${CLEANUP:-true}"

# Test resources
TEST_POOL="zfs_parcour_test_pool"
TEST_DATASET="testdata"
TEST_SNAPSHOT="snap1"
LOOP_FILE="/tmp/zfs_parcour_disk.img"
LOOP_SIZE="100M"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Counters
PASSED=0
FAILED=0
SKIPPED=0

# ==============================================================================
# Helper Functions
# ==============================================================================

log_header() {
    echo ""
    echo -e "${BLUE}━━━ $1 ━━━${NC}"
}

log_test() {
    echo -e "${CYAN}TEST:${NC} $1"
}

log_pass() {
    echo -e "${GREEN}  ✓ PASS${NC}"
    ((++PASSED))
}

log_fail() {
    echo -e "${RED}  ✗ FAIL: $1${NC}"
    ((++FAILED))
}

log_skip() {
    echo -e "${YELLOW}  ⊘ SKIP: $1${NC}"
    ((++SKIPPED))
}

log_info() {
    echo -e "  ${NC}→ $1${NC}"
}

# API call helper
api() {
    local method="$1"
    local endpoint="$2"
    local data="${3:-}"

    local curl_args=(-s -X "$method" "${API_URL}${endpoint}")
    curl_args+=(-H "Content-Type: application/json")

    if [[ -n "$API_KEY" ]]; then
        curl_args+=(-H "X-API-Key: $API_KEY")
    fi

    if [[ -n "$data" ]]; then
        curl_args+=(-d "$data")
    fi

    curl "${curl_args[@]}"
}

# Check if response indicates success
is_success() {
    echo "$1" | grep -q '"status":"success"'
}

# Extract field from JSON (basic)
json_field() {
    echo "$1" | grep -o "\"$2\":\"[^\"]*\"" | head -1 | cut -d'"' -f4
}

# ==============================================================================
# Prerequisites Check
# ==============================================================================

check_prerequisites() {
    log_header "PREREQUISITES"

    # Check ZFS
    log_test "ZFS kernel module loaded"
    if lsmod | grep -q "^zfs"; then
        log_pass
    else
        log_fail "ZFS module not loaded. Run: modprobe zfs"
        exit 2
    fi

    # Check server reachable
    log_test "API server reachable at $API_URL"
    if curl -s --connect-timeout 5 "${API_URL}/health" > /dev/null 2>&1; then
        log_pass
    else
        log_fail "Server not responding at $API_URL"
        exit 2
    fi

    # Check API key if required
    log_test "API authentication"
    local response
    response=$(api GET "/health")
    if is_success "$response"; then
        log_pass
    else
        if [[ -z "$API_KEY" ]]; then
            log_fail "Auth required but API_KEY not set"
            exit 2
        else
            log_fail "Invalid API key"
            exit 2
        fi
    fi
}

# ==============================================================================
# Setup
# ==============================================================================

setup_test_environment() {
    log_header "SETUP"

    # Create loop device for test pool
    log_test "Creating test disk image"
    if [[ -f "$LOOP_FILE" ]]; then
        log_info "Removing existing image"
        rm -f "$LOOP_FILE"
    fi
    truncate -s "$LOOP_SIZE" "$LOOP_FILE"
    log_pass

    log_test "Setting up loop device"
    LOOP_DEV=$(losetup -f --show "$LOOP_FILE")
    log_info "Loop device: $LOOP_DEV"
    log_pass
}

# ==============================================================================
# Cleanup
# ==============================================================================

cleanup() {
    if [[ "$CLEANUP" != "true" ]]; then
        echo ""
        echo -e "${YELLOW}CLEANUP SKIPPED (CLEANUP=false)${NC}"
        echo "Test pool '$TEST_POOL' and loop device remain for inspection."
        return
    fi

    log_header "CLEANUP"

    # Destroy test pool if it exists
    if zpool list "$TEST_POOL" &>/dev/null; then
        log_test "Destroying test pool"
        zpool destroy -f "$TEST_POOL" 2>/dev/null || true
        log_pass
    fi

    # Detach loop device
    if [[ -n "${LOOP_DEV:-}" ]] && losetup -a | grep -q "$LOOP_DEV"; then
        log_test "Detaching loop device"
        losetup -d "$LOOP_DEV" 2>/dev/null || true
        log_pass
    fi

    # Remove disk image
    if [[ -f "$LOOP_FILE" ]]; then
        log_test "Removing disk image"
        rm -f "$LOOP_FILE"
        log_pass
    fi
}

# Trap to ensure cleanup runs
trap cleanup EXIT

# ==============================================================================
# Test Cases
# ==============================================================================

test_health_endpoint() {
    log_header "1. HEALTH CHECK (MF-004)"

    log_test "GET /health returns status and version"
    local response
    response=$(api GET "/health")

    if is_success "$response"; then
        local version
        version=$(json_field "$response" "version")
        log_info "Version: $version"
        log_pass
    else
        log_fail "Unexpected response: $response"
    fi
}

test_pool_create() {
    log_header "2. CREATE POOL (MF-001)"

    log_test "POST /pools - create single-disk pool"
    local payload
    payload=$(cat <<EOF
{
    "name": "$TEST_POOL",
    "disks": ["$LOOP_DEV"]
}
EOF
)

    local response
    response=$(api POST "/pools" "$payload")

    if is_success "$response"; then
        log_pass

        # Verify with zpool list
        log_test "Pool visible in zpool list"
        if zpool list "$TEST_POOL" &>/dev/null; then
            log_pass
        else
            log_fail "Pool not found in zpool list"
        fi
    else
        log_fail "$(json_field "$response" "message")"
    fi
}

test_pool_status() {
    log_header "3. GET POOL STATUS (MF-001)"

    log_test "GET /pools/$TEST_POOL - retrieve status"
    local response
    response=$(api GET "/pools/$TEST_POOL")

    if is_success "$response"; then
        local health
        health=$(json_field "$response" "health")
        log_info "Health: $health"
        log_pass
    else
        log_fail "$(json_field "$response" "message")"
    fi
}

test_pool_list() {
    log_header "4. LIST POOLS (MF-001)"

    log_test "GET /pools - list all pools"
    local response
    response=$(api GET "/pools")

    if is_success "$response"; then
        if echo "$response" | grep -q "$TEST_POOL"; then
            log_info "Test pool found in list"
            log_pass
        else
            log_fail "Test pool not in list"
        fi
    else
        log_fail "$(json_field "$response" "message")"
    fi
}

test_dataset_create() {
    log_header "5. CREATE DATASET (MF-002)"

    log_test "POST /datasets - create filesystem"
    local payload
    payload=$(cat <<EOF
{
    "name": "$TEST_POOL/$TEST_DATASET",
    "kind": "filesystem"
}
EOF
)

    local response
    response=$(api POST "/datasets" "$payload")

    if is_success "$response"; then
        log_pass

        # Verify with zfs list
        log_test "Dataset visible in zfs list"
        if zfs list "$TEST_POOL/$TEST_DATASET" &>/dev/null; then
            log_pass
        else
            log_fail "Dataset not found in zfs list"
        fi
    else
        log_fail "$(json_field "$response" "message")"
    fi
}

test_dataset_list() {
    log_header "6. LIST DATASETS (MF-002)"

    log_test "GET /datasets/$TEST_POOL - list datasets"
    local response
    response=$(api GET "/datasets/$TEST_POOL")

    if is_success "$response"; then
        if echo "$response" | grep -q "$TEST_DATASET"; then
            log_info "Test dataset found in list"
            log_pass
        else
            log_fail "Test dataset not in list"
        fi
    else
        log_fail "$(json_field "$response" "message")"
    fi
}

test_snapshot_create() {
    log_header "7. CREATE SNAPSHOT (MF-003)"

    log_test "POST /snapshots/$TEST_POOL/$TEST_DATASET - create snapshot"
    local payload
    payload=$(cat <<EOF
{
    "snapshot_name": "$TEST_SNAPSHOT"
}
EOF
)

    local response
    response=$(api POST "/snapshots/$TEST_POOL/$TEST_DATASET" "$payload")

    if is_success "$response"; then
        log_pass

        # Verify with zfs list
        log_test "Snapshot visible in zfs list"
        if zfs list -t snapshot "$TEST_POOL/$TEST_DATASET@$TEST_SNAPSHOT" &>/dev/null; then
            log_pass
        else
            log_fail "Snapshot not found in zfs list"
        fi
    else
        log_fail "$(json_field "$response" "message")"
    fi
}

test_snapshot_list() {
    log_header "8. LIST SNAPSHOTS (MF-003)"

    log_test "GET /snapshots/$TEST_POOL/$TEST_DATASET - list snapshots"
    local response
    response=$(api GET "/snapshots/$TEST_POOL/$TEST_DATASET")

    if is_success "$response"; then
        if echo "$response" | grep -q "$TEST_SNAPSHOT"; then
            log_info "Test snapshot found in list"
            log_pass
        else
            log_fail "Test snapshot not in list"
        fi
    else
        log_fail "$(json_field "$response" "message")"
    fi
}

test_snapshot_delete() {
    log_header "9. DELETE SNAPSHOT (MF-003)"

    log_test "DELETE /snapshots/$TEST_POOL/$TEST_DATASET/$TEST_SNAPSHOT"
    local response
    response=$(api DELETE "/snapshots/$TEST_POOL/$TEST_DATASET/$TEST_SNAPSHOT")

    if is_success "$response"; then
        log_pass

        # Verify deletion
        log_test "Snapshot removed from zfs list"
        if ! zfs list -t snapshot "$TEST_POOL/$TEST_DATASET@$TEST_SNAPSHOT" &>/dev/null; then
            log_pass
        else
            log_fail "Snapshot still exists"
        fi
    else
        log_fail "$(json_field "$response" "message")"
    fi
}

test_dataset_delete() {
    log_header "10. DELETE DATASET (MF-002)"

    log_test "DELETE /datasets/$TEST_POOL/$TEST_DATASET"
    local response
    response=$(api DELETE "/datasets/$TEST_POOL/$TEST_DATASET")

    if is_success "$response"; then
        log_pass

        # Verify deletion
        log_test "Dataset removed from zfs list"
        if ! zfs list "$TEST_POOL/$TEST_DATASET" &>/dev/null; then
            log_pass
        else
            log_fail "Dataset still exists"
        fi
    else
        log_fail "$(json_field "$response" "message")"
    fi
}

test_scrub_start() {
    log_header "11. START SCRUB (MF-001 Phase 2)"

    log_test "POST /pools/$TEST_POOL/scrub - start scrub"
    local response
    response=$(api POST "/pools/$TEST_POOL/scrub")

    if is_success "$response"; then
        log_pass
    else
        # EBUSY is acceptable (scrub already running)
        if echo "$response" | grep -q "busy\|already"; then
            log_info "Scrub already running (acceptable)"
            log_pass
        else
            log_fail "$(json_field "$response" "message")"
        fi
    fi
}

test_scrub_status() {
    log_header "12. GET SCRUB STATUS (MF-001 Phase 2)"

    log_test "GET /pools/$TEST_POOL/scrub - get status"
    local response
    response=$(api GET "/pools/$TEST_POOL/scrub")

    if is_success "$response"; then
        local health
        health=$(json_field "$response" "pool_health")
        log_info "Pool health: $health"
        log_pass
    else
        log_fail "$(json_field "$response" "message")"
    fi
}

test_scrub_stop() {
    log_header "13. STOP SCRUB (MF-001 Phase 2)"

    log_test "POST /pools/$TEST_POOL/scrub/stop - stop scrub"
    local response
    response=$(api POST "/pools/$TEST_POOL/scrub/stop")

    if is_success "$response"; then
        log_pass
    else
        # Scrub already finished is expected on small/fast pools - treat as warning
        if echo "$response" | grep -q "NoActiveScrubs\|no.*scrub\|not.*running"; then
            log_skip "Scrub already finished (timing - small test pool)"
        else
            log_fail "$(json_field "$response" "message")"
        fi
    fi
}

test_pool_destroy() {
    log_header "14. DESTROY POOL (MF-001)"

    log_test "DELETE /pools/$TEST_POOL"
    local response
    response=$(api DELETE "/pools/$TEST_POOL")

    if is_success "$response"; then
        log_pass

        # Verify deletion
        log_test "Pool removed from zpool list"
        if ! zpool list "$TEST_POOL" &>/dev/null; then
            log_pass
        else
            log_fail "Pool still exists"
        fi
    else
        log_fail "$(json_field "$response" "message")"
    fi
}

# ==============================================================================
# Main
# ==============================================================================

main() {
    echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║           ZFS Feature Parcour - Integration Tests            ║${NC}"
    echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "API URL: $API_URL"
    echo "Test Pool: $TEST_POOL"
    echo ""

    check_prerequisites
    setup_test_environment

    # Run all tests in order
    test_health_endpoint
    test_pool_create
    test_pool_status
    test_pool_list
    test_dataset_create
    test_dataset_list
    test_snapshot_create
    test_snapshot_list
    test_snapshot_delete
    test_dataset_delete
    test_scrub_start
    test_scrub_status
    test_scrub_stop
    test_pool_destroy

    # Summary
    log_header "RESULTS"
    echo ""
    echo -e "  ${GREEN}Passed:${NC}  $PASSED"
    echo -e "  ${RED}Failed:${NC}  $FAILED"
    echo -e "  ${YELLOW}Skipped:${NC} $SKIPPED"
    echo ""

    if [[ $FAILED -eq 0 ]]; then
        echo -e "${GREEN}All tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}Some tests failed.${NC}"
        exit 1
    fi
}

main "$@"
