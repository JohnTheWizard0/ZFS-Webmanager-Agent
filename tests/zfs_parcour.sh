#!/bin/bash
# ==============================================================================
# ZFS Feature Parcour - End-to-End Integration Test (Testlab Edition)
# ==============================================================================
# Tests all implemented ZFS features using real disks in a testlab environment.
# Creates a RAIDZ pool with 3x10G disks, writes random data for scrub testing.
#
# REQUIREMENTS:
#   - ZFS installed and loaded (modprobe zfs)
#   - Three 10G test disks: /dev/sdb, /dev/sdc, /dev/sdd
#   - Disks must NOT be mounted or in use
#   - Root privileges
#
# USAGE:
#   ./tests/zfs_parcour.sh                    # Use defaults
#   API_URL=http://host:9876 ./tests/zfs_parcour.sh
#   API_KEY=your-key ./tests/zfs_parcour.sh
#   CLEANUP=false ./tests/zfs_parcour.sh      # Keep test pool after run
#   DATA_SIZE=500M ./tests/zfs_parcour.sh     # Write 500MB random data
#
# EXIT CODES:
#   0 - All tests passed
#   1 - Test failed
#   2 - Prerequisites not met
# ==============================================================================

set -euo pipefail

# Configuration
API_URL="${API_URL:-http://localhost:9876}"
API_KEY="${API_KEY:-08670612-43df-4a0c-a556-2288457726a5}"
CLEANUP="${CLEANUP:-true}"
SERVICE_NAME="zfs-agent"
DATA_SIZE="${DATA_SIZE:-100M}"

# Test resources - using real 10G disks
TEST_POOL="zfs_parcour_test_pool"
TEST_DATASET="testdata"
TEST_SNAPSHOT="snap1"

# Testlab disks (3x 10G)
TEST_DISK1="/dev/sdb"
TEST_DISK2="/dev/sdc"
TEST_DISK3="/dev/sdd"

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

# Extract field from JSON (basic) - returns empty if not found
json_field() {
    echo "$1" | grep -o "\"$2\":\"[^\"]*\"" 2>/dev/null | head -1 | cut -d'"' -f4 || true
}

# Extract numeric field from JSON - returns empty if not found
json_number() {
    echo "$1" | grep -o "\"$2\":[0-9.]*" 2>/dev/null | head -1 | cut -d':' -f2 || true
}

# ==============================================================================
# Service Management
# ==============================================================================

start_service() {
    log_header "SERVICE STARTUP"
    log_test "Starting $SERVICE_NAME service"
    if systemctl is-active --quiet "$SERVICE_NAME"; then
        log_info "Service already running, restarting..."
        systemctl restart "$SERVICE_NAME"
    else
        systemctl start "$SERVICE_NAME"
    fi
    sleep 2
    if systemctl is-active --quiet "$SERVICE_NAME"; then
        log_pass
    else
        log_fail "Failed to start $SERVICE_NAME"
        exit 2
    fi
}

stop_service() {
    log_header "SERVICE SHUTDOWN"
    log_test "Stopping $SERVICE_NAME service"
    systemctl stop "$SERVICE_NAME" 2>/dev/null || true
    log_pass
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

    # Check test disks exist
    log_test "Test disks available ($TEST_DISK1, $TEST_DISK2, $TEST_DISK3)"
    local missing=""
    for disk in "$TEST_DISK1" "$TEST_DISK2" "$TEST_DISK3"; do
        if [[ ! -b "$disk" ]]; then
            missing="$missing $disk"
        fi
    done
    if [[ -n "$missing" ]]; then
        log_fail "Missing disks:$missing"
        exit 2
    fi
    log_pass

    # Check disks are not mounted
    log_test "Test disks not in use"
    local in_use=""
    for disk in "$TEST_DISK1" "$TEST_DISK2" "$TEST_DISK3"; do
        if mount | grep -q "^$disk"; then
            in_use="$in_use $disk"
        fi
        # Also check partitions
        if mount | grep -q "^${disk}[0-9]"; then
            in_use="$in_use ${disk}*"
        fi
    done
    if [[ -n "$in_use" ]]; then
        log_fail "Disks in use:$in_use"
        exit 2
    fi
    log_pass

    # Check server reachable
    log_test "API server reachable at $API_URL"
    if curl -s --connect-timeout 5 "${API_URL}/v1/health" > /dev/null 2>&1; then
        log_pass
    else
        log_fail "Server not responding at $API_URL"
        exit 2
    fi

    # Check API key if required
    log_test "API authentication"
    local response
    response=$(api GET "/v1/health")
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
# Setup - No loop devices, using real disks
# ==============================================================================

setup_test_environment() {
    log_header "SETUP"

    # Destroy existing test pool if present
    log_test "Checking for existing test pool"
    if zpool list "$TEST_POOL" &>/dev/null; then
        log_info "Destroying existing test pool..."
        zpool destroy -f "$TEST_POOL" 2>/dev/null || true
    fi
    log_pass

    # Wipe partition tables on test disks
    log_test "Wiping partition tables and ZFS labels"
    for disk in "$TEST_DISK1" "$TEST_DISK2" "$TEST_DISK3"; do
        # Clear ZFS labels
        zpool labelclear -f "$disk" &>/dev/null || true
        # Wipe all signatures
        wipefs -af "$disk" &>/dev/null || true
        # Zap GPT
        sgdisk --zap-all "$disk" &>/dev/null || true
        # Zero first 100MB to clear any remaining labels
        dd if=/dev/zero of="$disk" bs=1M count=100 conv=notrunc &>/dev/null || true
        # Force kernel to re-read partition table
        blockdev --rereadpt "$disk" &>/dev/null || true
    done
    partprobe &>/dev/null || true
    sleep 1
    log_pass

    log_info "Test environment ready"
    log_info "Disks: $TEST_DISK1, $TEST_DISK2, $TEST_DISK3 (3x 10GB)"
}

# ==============================================================================
# Cleanup
# ==============================================================================

cleanup() {
    if [[ "$CLEANUP" != "true" ]]; then
        echo ""
        echo -e "${YELLOW}CLEANUP SKIPPED (CLEANUP=false)${NC}"
        echo "Test pool '$TEST_POOL' remains for inspection."
        echo "To destroy: zpool destroy -f $TEST_POOL"
        return
    fi

    log_header "CLEANUP"

    # Destroy test pool if it exists
    if zpool list "$TEST_POOL" &>/dev/null; then
        log_test "Destroying test pool"
        zpool destroy -f "$TEST_POOL" 2>/dev/null || true
        log_pass
    fi

    # Wipe disk labels
    log_test "Clearing ZFS labels from disks"
    for disk in "$TEST_DISK1" "$TEST_DISK2" "$TEST_DISK3"; do
        zpool labelclear -f "$disk" 2>/dev/null || true
    done
    log_pass
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
    response=$(api GET "/v1/health")

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
    log_header "2. CREATE RAIDZ POOL (MF-001)"

    log_test "POST /pools - create raidz pool with 3 disks"
    local payload
    payload=$(cat <<EOF
{
    "name": "$TEST_POOL",
    "raid_type": "raidz",
    "disks": ["$TEST_DISK1", "$TEST_DISK2", "$TEST_DISK3"]
}
EOF
)

    local response
    response=$(api POST "/v1/pools" "$payload")

    if is_success "$response"; then
        log_pass

        # Verify with zpool list
        log_test "Pool visible in zpool list"
        if zpool list "$TEST_POOL" &>/dev/null; then
            local size
            size=$(zpool list -H -o size "$TEST_POOL")
            log_info "Pool size: $size (raidz with 3 disks)"
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
    response=$(api GET "/v1/pools/$TEST_POOL")

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
    response=$(api GET "/v1/pools")

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
    response=$(api POST "/v1/datasets" "$payload")

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

test_write_random_data() {
    log_header "6. WRITE RANDOM DATA"

    log_test "Writing $DATA_SIZE of random data to dataset"

    # Get actual mountpoint from ZFS
    local mountpoint
    mountpoint=$(zfs get -H -o value mountpoint "$TEST_POOL/$TEST_DATASET" 2>/dev/null)

    # If not mounted or legacy, try to mount it
    if [[ "$mountpoint" == "none" ]] || [[ "$mountpoint" == "legacy" ]] || [[ ! -d "$mountpoint" ]]; then
        mountpoint="/$TEST_POOL/$TEST_DATASET"
        # Ensure dataset is mounted
        zfs set mountpoint="$mountpoint" "$TEST_POOL/$TEST_DATASET" 2>/dev/null || true
        zfs mount "$TEST_POOL/$TEST_DATASET" 2>/dev/null || true
    fi

    log_info "Mountpoint: $mountpoint"

    # Ensure directory exists
    if [[ ! -d "$mountpoint" ]]; then
        log_fail "Mountpoint $mountpoint does not exist"
        return
    fi

    # Write random data using dd
    dd if=/dev/urandom of="${mountpoint}/random_data.bin" bs=1M count="${DATA_SIZE%M}" 2>/dev/null
    sync

    # Verify file was created
    if [[ -f "${mountpoint}/random_data.bin" ]]; then
        local file_size
        file_size=$(du -h "${mountpoint}/random_data.bin" | cut -f1)
        log_info "Created ${file_size} random data file"
        log_pass
    else
        log_fail "Failed to create random data file"
        return
    fi

    # Also create some smaller files to have variety
    log_test "Creating additional test files"
    for i in {1..10}; do
        dd if=/dev/urandom of="${mountpoint}/file_${i}.bin" bs=1M count=5 2>/dev/null
    done
    sync

    local total_files
    total_files=$(ls -1 "${mountpoint}" | wc -l)
    log_info "Total files created: $total_files"
    log_pass
}

test_dataset_list() {
    log_header "7. LIST DATASETS (MF-002)"

    log_test "GET /datasets/$TEST_POOL - list datasets"
    local response
    response=$(api GET "/v1/datasets/$TEST_POOL")

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
    log_header "8. CREATE SNAPSHOT (MF-003)"

    log_test "POST /snapshots/$TEST_POOL/$TEST_DATASET - create snapshot"
    local payload
    payload=$(cat <<EOF
{
    "snapshot_name": "$TEST_SNAPSHOT"
}
EOF
)

    local response
    response=$(api POST "/v1/snapshots/$TEST_POOL/$TEST_DATASET" "$payload")

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
    log_header "9. LIST SNAPSHOTS (MF-003)"

    log_test "GET /snapshots/$TEST_POOL/$TEST_DATASET - list snapshots"
    local response
    response=$(api GET "/v1/snapshots/$TEST_POOL/$TEST_DATASET")

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

test_scrub_start() {
    log_header "10. START SCRUB (MF-001 Phase 2)"

    log_test "POST /pools/$TEST_POOL/scrub - start scrub"
    local response
    response=$(api POST "/v1/pools/$TEST_POOL/scrub")

    if is_success "$response"; then
        log_pass
        log_info "Scrub started on raidz pool with data"
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
    log_header "11. GET SCRUB STATUS (MF-001 Phase 2)"

    # Wait a moment for scrub to start processing
    sleep 2

    log_test "GET /pools/$TEST_POOL/scrub - get status"
    local response
    response=$(api GET "/v1/pools/$TEST_POOL/scrub")

    if is_success "$response"; then
        local health state
        health=$(json_field "$response" "pool_health")
        state=$(json_field "$response" "scan_state")
        log_info "Pool health: $health"
        log_info "Scrub state: $state"

        # Try to get progress info
        local scanned_pct
        scanned_pct=$(json_number "$response" "percent_done")
        if [[ -n "$scanned_pct" ]]; then
            log_info "Scanned: ${scanned_pct}%"
        fi
        log_pass
    else
        local err_msg
        err_msg=$(json_field "$response" "message")
        log_fail "${err_msg:-Unknown error}"
    fi
}

test_scrub_stop() {
    log_header "12. STOP SCRUB (MF-001 Phase 2)"

    log_test "POST /pools/$TEST_POOL/scrub/stop - stop scrub"
    local response
    response=$(api POST "/v1/pools/$TEST_POOL/scrub/stop")

    if is_success "$response"; then
        log_pass
    else
        # Scrub may have finished - that's OK with larger data
        if echo "$response" | grep -q "NoActiveScrubs\|no.*scrub\|not.*running\|finished"; then
            log_skip "Scrub already finished"
        else
            log_fail "$(json_field "$response" "message")"
        fi
    fi
}

test_snapshot_delete() {
    log_header "13. DELETE SNAPSHOT (MF-003)"

    log_test "DELETE /snapshots/$TEST_POOL/$TEST_DATASET/$TEST_SNAPSHOT"
    local response
    response=$(api DELETE "/v1/snapshots/$TEST_POOL/$TEST_DATASET/$TEST_SNAPSHOT")

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
    log_header "14. DELETE DATASET (MF-002)"

    log_test "DELETE /datasets/$TEST_POOL/$TEST_DATASET"
    local response
    response=$(api DELETE "/v1/datasets/$TEST_POOL/$TEST_DATASET")

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

test_pool_destroy() {
    log_header "15. DESTROY POOL (MF-001)"

    log_test "DELETE /pools/$TEST_POOL"
    local response
    response=$(api DELETE "/v1/pools/$TEST_POOL")

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
    echo -e "${BLUE}║      ZFS Feature Parcour - Integration Tests (Testlab)       ║${NC}"
    echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "API URL: $API_URL"
    echo "Test Pool: $TEST_POOL (raidz)"
    echo "Test Disks: $TEST_DISK1, $TEST_DISK2, $TEST_DISK3"
    echo "Random Data Size: $DATA_SIZE"
    echo ""

    # Start service
    start_service

    # Trap to ensure cleanup on exit
    trap 'cleanup; stop_service' EXIT

    check_prerequisites
    setup_test_environment

    # Run all tests in order
    test_health_endpoint
    test_pool_create
    test_pool_status
    test_pool_list
    test_dataset_create
    test_write_random_data    # New: write data for scrubbing
    test_dataset_list
    test_snapshot_create
    test_snapshot_list
    test_scrub_start
    test_scrub_status
    test_scrub_stop
    test_snapshot_delete
    test_dataset_delete
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
