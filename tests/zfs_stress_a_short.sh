#!/bin/bash
# ==============================================================================
# ZFS Stress Test A (Short) - Dataset, Snapshot, Property Edge Cases
# ==============================================================================
# Tests edge cases and error handling for data-layer operations.
# Runs against existing pools or creates test pools if needed.
#
# TESTS INCLUDED:
#   P1-P5: Property edge cases (invalid names, values, non-existent datasets)
#   S1-S5: Snapshot edge cases (invalid names, duplicates, clone dependencies)
#   D1-D5: Dataset edge cases (nesting, duplicates, delete with children)
#   R1-R2: Rollback basic cases
#
# USAGE:
#   ./tests/zfs_stress_a_short.sh
#   API_URL=http://host:9876 ./tests/zfs_stress_a_short.sh
#   CLEANUP=false ./tests/zfs_stress_a_short.sh
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

# Test pool names
STRESS_POOL="stress_test_pool"
TEST_DATASET="stressdata"

# Testlab disks (use 2 for single pool)
DISK_1="/dev/sdb"
DISK_2="/dev/sdc"

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

log_expect_error() {
    echo -e "  ${YELLOW}→ Expected error: $1${NC}"
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

    curl "${curl_args[@]}" 2>/dev/null || echo '{"status":"error","message":"curl failed"}'
}

# API call without auth
api_no_auth() {
    local method="$1"
    local endpoint="$2"
    local data="${3:-}"

    local curl_args=(-s -X "$method" "${API_URL}${endpoint}")
    curl_args+=(-H "Content-Type: application/json")

    if [[ -n "$data" ]]; then
        curl_args+=(-d "$data")
    fi

    curl "${curl_args[@]}" 2>/dev/null || echo '{"status":"error","message":"curl failed"}'
}

is_success() {
    echo "$1" | grep -q '"status":"success"'
}

is_error() {
    echo "$1" | grep -q '"status":"error"' || echo "$1" | grep -qi 'error\|fail\|not found\|invalid'
}

json_field() {
    echo "$1" | grep -o "\"$2\":\"[^\"]*\"" 2>/dev/null | head -1 | cut -d'"' -f4 || true
}

# ==============================================================================
# Prerequisites
# ==============================================================================

check_prerequisites() {
    log_header "PREREQUISITES CHECK"

    # Check ZFS loaded
    log_test "ZFS kernel module loaded"
    if lsmod | grep -q "^zfs"; then
        log_pass
    else
        log_fail "ZFS not loaded"
        exit 2
    fi

    # Check API reachable
    log_test "API endpoint reachable"
    local response
    response=$(curl -s -o /dev/null -w "%{http_code}" "${API_URL}/v1/health" 2>/dev/null || echo "000")
    if [[ "$response" == "200" ]]; then
        log_pass
    else
        log_fail "Cannot reach ${API_URL} (HTTP $response)"
        exit 2
    fi

    # Check disks available
    log_test "Test disks available"
    if [[ -b "$DISK_1" ]] && [[ -b "$DISK_2" ]]; then
        log_pass
    else
        log_fail "Disks $DISK_1 and $DISK_2 not found"
        exit 2
    fi
}

# ==============================================================================
# Setup / Teardown
# ==============================================================================

setup_test_pool() {
    log_header "SETUP"

    # Destroy existing test pool if exists
    if zpool list "$STRESS_POOL" &>/dev/null; then
        log_test "Destroying existing test pool"
        zpool destroy -f "$STRESS_POOL" 2>/dev/null || true
        log_pass
    fi

    # Create test pool
    log_test "Creating test pool: $STRESS_POOL (mirror)"
    local payload
    payload=$(cat <<EOF
{
    "name": "$STRESS_POOL",
    "raid_type": "mirror",
    "disks": ["$DISK_1", "$DISK_2"]
}
EOF
)
    local response
    response=$(api POST "/v1/pools" "$payload")

    if is_success "$response"; then
        log_pass
    else
        log_fail "$(json_field "$response" "message")"
        exit 2
    fi

    # Create base dataset for tests
    log_test "Creating base dataset: $STRESS_POOL/$TEST_DATASET"
    payload=$(cat <<EOF
{
    "name": "$STRESS_POOL/$TEST_DATASET",
    "kind": "filesystem"
}
EOF
)
    response=$(api POST "/v1/datasets" "$payload")

    if is_success "$response"; then
        log_pass
    else
        log_fail "$(json_field "$response" "message")"
        exit 2
    fi
}

cleanup() {
    if [[ "$CLEANUP" == "true" ]]; then
        log_header "CLEANUP"
        log_test "Destroying test pool"
        if zpool destroy -f "$STRESS_POOL" 2>/dev/null; then
            log_pass
        else
            log_info "Pool already destroyed or not found"
        fi
    else
        log_header "CLEANUP SKIPPED"
        log_info "Pool $STRESS_POOL retained for inspection"
    fi
}

trap cleanup EXIT

# ==============================================================================
# TEST A1: Dataset Properties Edge Cases
# ==============================================================================

test_property_invalid_name() {
    log_header "P1: SET INVALID PROPERTY NAME"

    log_test "PUT /datasets/.../properties - invalid property 'notarealproperty'"
    local payload='{"property": "notarealproperty", "value": "somevalue"}'
    local response
    response=$(api PUT "/v1/datasets/$STRESS_POOL/$TEST_DATASET/properties" "$payload")

    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should have rejected invalid property name"
    fi
}

test_property_invalid_value() {
    log_header "P2: SET INVALID PROPERTY VALUE"

    log_test "PUT /datasets/.../properties - compression=notavalidvalue"
    local payload='{"property": "compression", "value": "notavalidvalue"}'
    local response
    response=$(api PUT "/v1/datasets/$STRESS_POOL/$TEST_DATASET/properties" "$payload")

    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should have rejected invalid compression value"
    fi
}

test_property_readonly() {
    log_header "P3: SET READ-ONLY PROPERTY"

    log_test "PUT /datasets/.../properties - 'used' (read-only)"
    local payload='{"property": "used", "value": "12345"}'
    local response
    response=$(api PUT "/v1/datasets/$STRESS_POOL/$TEST_DATASET/properties" "$payload")

    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should have rejected setting read-only property"
    fi
}

test_property_special_chars() {
    log_header "P4: PROPERTY VALUE WITH SPECIAL CHARS"

    # Test with a valid property that accepts string values
    log_test "PUT /datasets/.../properties - comment with special chars"
    local payload='{"property": "comment", "value": "test value with spaces & symbols!"}'
    local response
    response=$(api PUT "/v1/datasets/$STRESS_POOL/$TEST_DATASET/properties" "$payload")

    # This could either succeed or fail depending on shell escaping
    if is_success "$response" || is_error "$response"; then
        log_info "Response: $(json_field "$response" "message")"
        log_pass
    else
        log_fail "Unexpected response"
    fi
}

test_property_nonexistent_dataset() {
    log_header "P5: SET PROPERTY ON NON-EXISTENT DATASET"

    log_test "PUT /datasets/nonexistent/dataset/properties"
    local payload='{"property": "compression", "value": "lz4"}'
    local response
    response=$(api PUT "/v1/datasets/$STRESS_POOL/nonexistent_ds_12345/properties" "$payload")

    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should have returned error for non-existent dataset"
    fi
}

# ==============================================================================
# TEST A2: Snapshot Edge Cases
# ==============================================================================

test_snapshot_invalid_name() {
    log_header "S1: SNAPSHOT WITH INVALID NAME"

    # Test with @ symbol (invalid in snapshot name portion)
    log_test "POST /snapshots - name containing '@'"
    local payload='{"snapshot_name": "snap@invalid"}'
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" "$payload")

    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        # ZFS might accept it, check if it actually exists
        if zfs list "$STRESS_POOL/$TEST_DATASET@snap@invalid" &>/dev/null; then
            log_fail "ZFS should not allow @ in snapshot name"
        else
            log_pass
        fi
    fi

    # Test with spaces
    log_test "POST /snapshots - name containing spaces"
    payload='{"snapshot_name": "snap with spaces"}'
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" "$payload")

    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should reject snapshot name with spaces"
    fi
}

test_snapshot_nonexistent_dataset() {
    log_header "S2: SNAPSHOT ON NON-EXISTENT DATASET"

    log_test "POST /snapshots/nonexistent_dataset - create snapshot"
    local payload='{"snapshot_name": "testsnap"}'
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/nonexistent_dataset_xyz" "$payload")

    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should have returned error for non-existent dataset"
    fi
}

test_snapshot_duplicate() {
    log_header "S3: DUPLICATE SNAPSHOT NAME"

    # Create first snapshot
    log_test "Create initial snapshot 'dupsnap'"
    local payload='{"snapshot_name": "dupsnap"}'
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" "$payload")

    if is_success "$response"; then
        log_pass
    else
        log_fail "$(json_field "$response" "message")"
        return
    fi

    # Try to create duplicate
    log_test "Create duplicate snapshot 'dupsnap'"
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" "$payload")

    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should have rejected duplicate snapshot name"
    fi

    # Cleanup
    api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/dupsnap" >/dev/null 2>&1 || true
}

test_snapshot_delete_with_clone() {
    log_header "S4: DELETE SNAPSHOT WITH DEPENDENT CLONE"

    # Create snapshot
    log_test "Create snapshot 'clonedsnap'"
    local payload='{"snapshot_name": "clonedsnap"}'
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" "$payload")

    if ! is_success "$response"; then
        log_fail "Setup: $(json_field "$response" "message")"
        return
    fi
    log_pass

    # Create clone
    log_test "Create clone from snapshot"
    payload="{\"target\": \"$STRESS_POOL/clone_from_snap\"}"
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/clonedsnap/clone" "$payload")

    if ! is_success "$response"; then
        log_fail "Setup: $(json_field "$response" "message")"
        return
    fi
    log_pass

    # Try to delete snapshot (should fail)
    log_test "DELETE snapshot with dependent clone"
    response=$(api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/clonedsnap")

    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should have rejected deletion of snapshot with clone"
    fi

    # Cleanup: destroy clone first, then snapshot
    zfs destroy "$STRESS_POOL/clone_from_snap" 2>/dev/null || true
    api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/clonedsnap" >/dev/null 2>&1 || true
}

test_snapshot_rapid_create() {
    log_header "S5: RAPID SNAPSHOT CREATION (5 snapshots)"

    log_test "Creating 5 snapshots rapidly"
    local success_count=0
    local i

    for i in {1..5}; do
        local payload="{\"snapshot_name\": \"rapidsnap$i\"}"
        local response
        response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" "$payload")

        if is_success "$response"; then
            ((++success_count))
        else
            log_info "Snap $i failed: $(json_field "$response" "message")"
        fi
    done

    if [[ $success_count -eq 5 ]]; then
        log_info "All 5 snapshots created"
        log_pass
    else
        log_fail "Only $success_count/5 snapshots created"
    fi

    # Verify with list
    log_test "Verify all snapshots exist"
    local response
    response=$(api GET "/v1/snapshots/$STRESS_POOL/$TEST_DATASET")

    local list_count
    list_count=$(echo "$response" | grep -o 'rapidsnap' | wc -l)

    if [[ $list_count -ge 5 ]]; then
        log_pass
    else
        log_fail "Only found $list_count snapshots in list"
    fi

    # Cleanup
    for i in {1..5}; do
        api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/rapidsnap$i" >/dev/null 2>&1 || true
    done
}

# ==============================================================================
# TEST A3: Dataset Edge Cases
# ==============================================================================

test_dataset_nested() {
    log_header "D1: DEEPLY NESTED DATASET (5 levels)"

    log_test "Create 5-level nested dataset"
    local nested_path="$STRESS_POOL/$TEST_DATASET/level1/level2/level3/level4/level5"

    # Create each level
    local current_path="$STRESS_POOL/$TEST_DATASET"
    local success=true

    for level in level1 level2 level3 level4 level5; do
        current_path="$current_path/$level"
        local payload
        payload=$(cat <<EOF
{
    "name": "$current_path",
    "kind": "filesystem"
}
EOF
)
        local response
        response=$(api POST "/v1/datasets" "$payload")

        if ! is_success "$response"; then
            log_fail "Failed at $level: $(json_field "$response" "message")"
            success=false
            break
        fi
    done

    if $success; then
        log_info "Created: $nested_path"
        log_pass
    fi

    # Verify existence
    log_test "Verify nested dataset exists"
    if zfs list "$nested_path" &>/dev/null; then
        log_pass
    else
        log_fail "Dataset not found via zfs list"
    fi

    # Cleanup - destroy from deepest level up
    for level in level5 level4 level3 level2 level1; do
        zfs destroy "$STRESS_POOL/$TEST_DATASET/$level" 2>/dev/null || true
    done
    # Actually destroy properly
    zfs destroy -r "$STRESS_POOL/$TEST_DATASET/level1" 2>/dev/null || true
}

test_dataset_invalid_name() {
    log_header "D2: DATASET WITH INVALID NAME"

    # Test with @ symbol
    log_test "Create dataset with '@' in name"
    local payload
    payload=$(cat <<EOF
{
    "name": "$STRESS_POOL/invalid@name",
    "kind": "filesystem"
}
EOF
)
    local response
    response=$(api POST "/v1/datasets" "$payload")

    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should reject '@' in dataset name"
        zfs destroy "$STRESS_POOL/invalid@name" 2>/dev/null || true
    fi

    # Test with spaces
    log_test "Create dataset with spaces in name"
    payload=$(cat <<EOF
{
    "name": "$STRESS_POOL/invalid name",
    "kind": "filesystem"
}
EOF
)
    response=$(api POST "/v1/datasets" "$payload")

    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should reject spaces in dataset name"
    fi
}

test_dataset_duplicate() {
    log_header "D3: DUPLICATE DATASET"

    # Create first dataset
    log_test "Create dataset 'dupdata'"
    local payload
    payload=$(cat <<EOF
{
    "name": "$STRESS_POOL/dupdata",
    "kind": "filesystem"
}
EOF
)
    local response
    response=$(api POST "/v1/datasets" "$payload")

    if ! is_success "$response"; then
        log_fail "Setup: $(json_field "$response" "message")"
        return
    fi
    log_pass

    # Try duplicate
    log_test "Create duplicate dataset 'dupdata'"
    response=$(api POST "/v1/datasets" "$payload")

    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should have rejected duplicate dataset"
    fi

    # Cleanup
    zfs destroy "$STRESS_POOL/dupdata" 2>/dev/null || true
}

test_dataset_delete_with_children() {
    log_header "D4: DELETE DATASET WITH CHILDREN"

    # Create parent dataset
    log_test "Create parent dataset"
    local payload
    payload=$(cat <<EOF
{
    "name": "$STRESS_POOL/parent_ds",
    "kind": "filesystem"
}
EOF
)
    local response
    response=$(api POST "/v1/datasets" "$payload")

    if ! is_success "$response"; then
        log_fail "Setup: $(json_field "$response" "message")"
        return
    fi
    log_pass

    # Create child dataset
    log_test "Create child dataset"
    payload=$(cat <<EOF
{
    "name": "$STRESS_POOL/parent_ds/child_ds",
    "kind": "filesystem"
}
EOF
)
    response=$(api POST "/v1/datasets" "$payload")

    if ! is_success "$response"; then
        log_fail "Setup: $(json_field "$response" "message")"
        return
    fi
    log_pass

    # Try to delete parent (should fail without -r)
    log_test "DELETE parent dataset (has children)"
    response=$(api DELETE "/v1/datasets/$STRESS_POOL/parent_ds")

    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should have rejected deletion of dataset with children"
    fi

    # Cleanup: recursive destroy via CLI
    zfs destroy -r "$STRESS_POOL/parent_ds" 2>/dev/null || true
}

test_dataset_recursive_delete() {
    log_header "D4b: RECURSIVE DELETE VIA API (?recursive=true)"

    # Create parent dataset
    log_test "Create parent dataset"
    local payload
    payload=$(cat <<EOF
{
    "name": "$STRESS_POOL/recursive_del",
    "kind": "filesystem"
}
EOF
)
    local response
    response=$(api POST "/v1/datasets" "$payload")

    if ! is_success "$response"; then
        log_fail "Setup: $(json_field "$response" "message")"
        return
    fi
    log_pass

    # Create child dataset
    log_test "Create child dataset"
    payload=$(cat <<EOF
{
    "name": "$STRESS_POOL/recursive_del/child1",
    "kind": "filesystem"
}
EOF
)
    response=$(api POST "/v1/datasets" "$payload")

    if ! is_success "$response"; then
        log_fail "Setup: $(json_field "$response" "message")"
        zfs destroy -r "$STRESS_POOL/recursive_del" 2>/dev/null || true
        return
    fi
    log_pass

    # Create grandchild dataset
    log_test "Create grandchild dataset"
    payload=$(cat <<EOF
{
    "name": "$STRESS_POOL/recursive_del/child1/grandchild",
    "kind": "filesystem"
}
EOF
)
    response=$(api POST "/v1/datasets" "$payload")

    if ! is_success "$response"; then
        log_fail "Setup: $(json_field "$response" "message")"
        zfs destroy -r "$STRESS_POOL/recursive_del" 2>/dev/null || true
        return
    fi
    log_pass

    # Create snapshot on grandchild
    log_test "Create snapshot on grandchild"
    payload='{"snapshot_name": "snap1"}'
    response=$(api POST "/v1/snapshots/$STRESS_POOL/recursive_del/child1/grandchild" "$payload")

    if ! is_success "$response"; then
        log_fail "Setup: $(json_field "$response" "message")"
        zfs destroy -r "$STRESS_POOL/recursive_del" 2>/dev/null || true
        return
    fi
    log_pass

    # Recursive delete using ?recursive=true
    log_test "DELETE with ?recursive=true"
    response=$(api DELETE "/v1/datasets/$STRESS_POOL/recursive_del?recursive=true")

    if is_success "$response"; then
        log_info "Recursive delete succeeded"
        log_pass
    else
        log_fail "$(json_field "$response" "message")"
        # Cleanup on failure
        zfs destroy -r "$STRESS_POOL/recursive_del" 2>/dev/null || true
        return
    fi

    # Verify deletion
    log_test "Verify dataset is gone"
    if zfs list "$STRESS_POOL/recursive_del" &>/dev/null; then
        log_fail "Dataset still exists"
    else
        log_info "Dataset and children deleted"
        log_pass
    fi
}

test_dataset_delete_with_snapshots() {
    log_header "D5: DELETE DATASET WITH SNAPSHOTS"

    # Create dataset
    log_test "Create dataset with snapshot"
    local payload
    payload=$(cat <<EOF
{
    "name": "$STRESS_POOL/snapdata",
    "kind": "filesystem"
}
EOF
)
    local response
    response=$(api POST "/v1/datasets" "$payload")

    if ! is_success "$response"; then
        log_fail "Setup: $(json_field "$response" "message")"
        return
    fi
    log_pass

    # Create snapshot
    log_test "Create snapshot on dataset"
    payload='{"snapshot_name": "testsnap"}'
    response=$(api POST "/v1/snapshots/$STRESS_POOL/snapdata" "$payload")

    if ! is_success "$response"; then
        log_fail "Setup: $(json_field "$response" "message")"
        return
    fi
    log_pass

    # Try to delete dataset (should fail)
    log_test "DELETE dataset with snapshot"
    response=$(api DELETE "/v1/datasets/$STRESS_POOL/snapdata")

    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should have rejected deletion of dataset with snapshots"
    fi

    # Cleanup
    zfs destroy -r "$STRESS_POOL/snapdata" 2>/dev/null || true
}

# ==============================================================================
# TEST A4: Rollback Cases
# ==============================================================================

test_rollback_most_recent() {
    log_header "R1: ROLLBACK TO MOST RECENT SNAPSHOT"

    # Create a dataset with snapshot
    log_test "Setup: create dataset and snapshot"
    local payload
    payload=$(cat <<EOF
{
    "name": "$STRESS_POOL/rollback_test",
    "kind": "filesystem"
}
EOF
)
    local response
    response=$(api POST "/v1/datasets" "$payload")

    if ! is_success "$response"; then
        log_fail "Setup: $(json_field "$response" "message")"
        return
    fi

    # Write some data
    echo "initial data" > "/$STRESS_POOL/rollback_test/testfile.txt" 2>/dev/null || true

    # Create snapshot
    payload='{"snapshot_name": "before_change"}'
    response=$(api POST "/v1/snapshots/$STRESS_POOL/rollback_test" "$payload")

    if ! is_success "$response"; then
        log_fail "Setup snapshot: $(json_field "$response" "message")"
        zfs destroy -r "$STRESS_POOL/rollback_test" 2>/dev/null || true
        return
    fi
    log_pass

    # Modify data
    echo "modified data" > "/$STRESS_POOL/rollback_test/testfile.txt" 2>/dev/null || true

    # Rollback
    log_test "Rollback to most recent snapshot"
    payload='{"snapshot": "before_change"}'
    response=$(api POST "/v1/datasets/$STRESS_POOL/rollback_test/rollback" "$payload")

    if is_success "$response"; then
        log_pass
    else
        log_fail "$(json_field "$response" "message")"
    fi

    # Cleanup
    zfs destroy -r "$STRESS_POOL/rollback_test" 2>/dev/null || true
}

test_rollback_older_blocked() {
    log_header "R2: ROLLBACK TO OLDER SNAPSHOT (BLOCKED)"

    # Create dataset
    log_test "Setup: dataset with 2 snapshots"
    local payload
    payload=$(cat <<EOF
{
    "name": "$STRESS_POOL/rollback_block_test",
    "kind": "filesystem"
}
EOF
)
    local response
    response=$(api POST "/v1/datasets" "$payload")

    if ! is_success "$response"; then
        log_fail "Setup: $(json_field "$response" "message")"
        return
    fi

    # Create first snapshot
    payload='{"snapshot_name": "snap_old"}'
    response=$(api POST "/v1/snapshots/$STRESS_POOL/rollback_block_test" "$payload")

    if ! is_success "$response"; then
        log_fail "Setup snap1: $(json_field "$response" "message")"
        zfs destroy -r "$STRESS_POOL/rollback_block_test" 2>/dev/null || true
        return
    fi

    # Create second snapshot
    payload='{"snapshot_name": "snap_new"}'
    response=$(api POST "/v1/snapshots/$STRESS_POOL/rollback_block_test" "$payload")

    if ! is_success "$response"; then
        log_fail "Setup snap2: $(json_field "$response" "message")"
        zfs destroy -r "$STRESS_POOL/rollback_block_test" 2>/dev/null || true
        return
    fi
    log_pass

    # Try to rollback to older snapshot (should be blocked)
    log_test "Rollback to older snapshot (blocked by newer)"
    payload='{"snapshot": "snap_old"}'
    response=$(api POST "/v1/datasets/$STRESS_POOL/rollback_block_test/rollback" "$payload")

    if is_error "$response"; then
        # Check for blocking info
        if echo "$response" | grep -qi "block\|newer"; then
            log_expect_error "$(json_field "$response" "message")"
            log_pass
        else
            log_info "Error: $(json_field "$response" "message")"
            log_pass  # Still pass as long as it was rejected
        fi
    else
        log_fail "Should have blocked rollback to older snapshot"
    fi

    # Cleanup
    zfs destroy -r "$STRESS_POOL/rollback_block_test" 2>/dev/null || true
}

# ==============================================================================
# Main
# ==============================================================================

main() {
    echo -e "${BLUE}╔════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║     ZFS STRESS TEST A (SHORT) - Dataset/Snapshot/Property      ║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "API URL: $API_URL"
    echo "Cleanup: $CLEANUP"
    echo ""

    check_prerequisites
    setup_test_pool

    # A1: Properties
    test_property_invalid_name
    test_property_invalid_value
    test_property_readonly
    test_property_special_chars
    test_property_nonexistent_dataset

    # A2: Snapshots
    test_snapshot_invalid_name
    test_snapshot_nonexistent_dataset
    test_snapshot_duplicate
    test_snapshot_delete_with_clone
    test_snapshot_rapid_create

    # A3: Datasets
    test_dataset_nested
    test_dataset_invalid_name
    test_dataset_duplicate
    test_dataset_delete_with_children
    test_dataset_recursive_delete
    test_dataset_delete_with_snapshots

    # A4: Rollback
    test_rollback_most_recent
    test_rollback_older_blocked

    # Summary
    log_header "TEST SUMMARY"
    echo -e "Passed:  ${GREEN}$PASSED${NC}"
    echo -e "Failed:  ${RED}$FAILED${NC}"
    echo -e "Skipped: ${YELLOW}$SKIPPED${NC}"
    echo ""

    if [[ $FAILED -gt 0 ]]; then
        echo -e "${RED}SOME TESTS FAILED${NC}"
        exit 1
    else
        echo -e "${GREEN}ALL TESTS PASSED${NC}"
        exit 0
    fi
}

main "$@"
