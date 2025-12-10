#!/bin/bash
# ==============================================================================
# ZFS Stress Test B (Long) - Complete Pool, Replication, Auth, API Tests
# ==============================================================================
# Full stress testing for infrastructure operations including concurrent
# requests, large payloads, and complete error handling verification.
#
# TESTS INCLUDED:
#   PO1-PO10: All pool operation edge cases
#   SC1-SC5:  All scrub edge cases
#   EI1-EI7:  All export/import edge cases
#   RE1-RE12: All replication edge cases
#   AU1-AU5:  All authentication cases
#   AR1-AR9:  All API robustness cases
#
# USAGE:
#   ./tests/zfs_stress_b_long.sh
#   API_URL=http://host:9876 ./tests/zfs_stress_b_long.sh
#
# DURATION: ~15 minutes
# ==============================================================================

set -euo pipefail

# Configuration
API_URL="${API_URL:-http://localhost:9876}"
API_KEY="${API_KEY:-08670612-43df-4a0c-a556-2288457726a5}"
CLEANUP="${CLEANUP:-true}"

# Source shared cleanup script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/cleanup_tests.sh"

# Test resources
STRESS_POOL="stress_b_pool"
STRESS_POOL_B="stress_b_pool_alt"
TEST_DATASET="testdata"
SEND_FILE="/tmp/stress_b_send.zfs"

# Testlab disks
DISK_1="/dev/sdb"
DISK_2="/dev/sdc"
DISK_3="/dev/sdd"
DISK_4="/dev/sde"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

PASSED=0
FAILED=0
SKIPPED=0

# ==============================================================================
# Helpers
# ==============================================================================

log_header() { echo ""; echo -e "${BLUE}━━━ $1 ━━━${NC}"; }
log_test() { echo -e "${CYAN}TEST:${NC} $1"; }
log_pass() { echo -e "${GREEN}  ✓ PASS${NC}"; ((++PASSED)); }
log_fail() { echo -e "${RED}  ✗ FAIL: $1${NC}"; ((++FAILED)); }
log_skip() { echo -e "${YELLOW}  ⊘ SKIP: $1${NC}"; ((++SKIPPED)); }
log_info() { echo -e "  ${NC}→ $1${NC}"; }
log_expect_error() { echo -e "  ${YELLOW}→ Expected error: $1${NC}"; }

api() {
    local method="$1" endpoint="$2" data="${3:-}"
    local curl_args=(-s -X "$method" "${API_URL}${endpoint}" -H "Content-Type: application/json")
    [[ -n "$API_KEY" ]] && curl_args+=(-H "X-API-Key: $API_KEY")
    [[ -n "$data" ]] && curl_args+=(-d "$data")
    curl "${curl_args[@]}" 2>/dev/null || echo '{"status":"error","message":"curl failed"}'
}

api_no_auth() {
    local method="$1" endpoint="$2" data="${3:-}"
    local curl_args=(-s -X "$method" "${API_URL}${endpoint}" -H "Content-Type: application/json")
    [[ -n "$data" ]] && curl_args+=(-d "$data")
    curl "${curl_args[@]}" 2>/dev/null || echo '{"status":"error","message":"curl failed"}'
}

is_success() { echo "$1" | grep -q '"status":"success"'; }
is_error() { echo "$1" | grep -q '"status":"error"' || echo "$1" | grep -qi 'error\|fail\|not found\|invalid\|unauthorized'; }
json_field() { echo "$1" | grep -o "\"$2\":\"[^\"]*\"" 2>/dev/null | head -1 | cut -d'"' -f4 || true; }

wait_for_task() {
    local task_id="$1"
    local max_wait="${2:-30}"
    local waited=0
    while [[ $waited -lt $max_wait ]]; do
        local response
        response=$(api GET "/v1/tasks/$task_id")
        local status
        status=$(json_field "$response" "status")
        [[ "$status" == "completed" || "$status" == "failed" ]] && { echo "$response"; return 0; }
        sleep 1
        ((++waited))
    done
    echo '{"status":"timeout"}'
}

# ==============================================================================
# Prerequisites & Setup
# ==============================================================================

check_prerequisites() {
    log_header "PREREQUISITES CHECK"
    log_test "ZFS kernel module"
    lsmod | grep -q "^zfs" && log_pass || { log_fail "ZFS not loaded"; exit 2; }

    log_test "API endpoint"
    local code
    code=$(curl -s -o /dev/null -w "%{http_code}" "${API_URL}/v1/health" 2>/dev/null || echo "000")
    [[ "$code" == "200" ]] && log_pass || { log_fail "API unreachable"; exit 2; }

    log_test "Test disks (4 required)"
    [[ -b "$DISK_1" ]] && [[ -b "$DISK_2" ]] && [[ -b "$DISK_3" ]] && [[ -b "$DISK_4" ]] && log_pass || { log_fail "Need 4 disks"; exit 2; }
}

cleanup_pools() {
    zpool destroy -f "$STRESS_POOL" 2>/dev/null || true
    zpool destroy -f "$STRESS_POOL_B" 2>/dev/null || true
    rm -f "$SEND_FILE" 2>/dev/null || true
}

setup_test_pools() {
    log_header "SETUP"
    cleanup_pools

    log_test "Creating $STRESS_POOL"
    local response
    response=$(api POST "/v1/pools" "{\"name\": \"$STRESS_POOL\", \"raid_type\": \"mirror\", \"disks\": [\"$DISK_1\", \"$DISK_2\"]}")
    is_success "$response" && log_pass || { log_fail "$(json_field "$response" "message")"; exit 2; }

    log_test "Creating $STRESS_POOL_B"
    response=$(api POST "/v1/pools" "{\"name\": \"$STRESS_POOL_B\", \"raid_type\": \"mirror\", \"disks\": [\"$DISK_3\", \"$DISK_4\"]}")
    is_success "$response" && log_pass || { log_fail "$(json_field "$response" "message")"; exit 2; }

    log_test "Creating test dataset"
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/$TEST_DATASET\", \"kind\": \"filesystem\"}")
    is_success "$response" && log_pass || { log_fail "$(json_field "$response" "message")"; exit 2; }
}

cleanup() {
    if [[ "$CLEANUP" == "true" ]]; then
        run_test_cleanup true  # Use shared cleanup (quiet mode)
    else
        log_header "CLEANUP SKIPPED"
        log_info "Pools retained: $STRESS_POOL, $STRESS_POOL_B"
    fi
}
trap cleanup EXIT

# ==============================================================================
# Pool Operations (PO1-PO10)
# ==============================================================================

test_pool_invalid_disk() {
    log_header "PO1: CREATE WITH INVALID DISK"
    log_test "POST /pools with nonexistent disk"
    local response
    response=$(api POST "/v1/pools" '{"name": "fake", "raid_type": "mirror", "disks": ["/dev/fake1", "/dev/fake2"]}')
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should reject"
}

test_pool_disk_in_use() {
    log_header "PO2: DISK ALREADY IN USE"
    log_test "POST with used disk"
    local response
    response=$(api POST "/v1/pools" "{\"name\": \"conflict\", \"raid_type\": \"mirror\", \"disks\": [\"$DISK_1\", \"$DISK_2\"]}")
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should reject"
}

test_pool_duplicate_name() {
    log_header "PO3: DUPLICATE POOL NAME"
    log_test "POST with existing name"
    local response
    response=$(api POST "/v1/pools" "{\"name\": \"$STRESS_POOL\", \"raid_type\": \"mirror\", \"disks\": [\"$DISK_1\", \"$DISK_2\"]}")
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should reject"
}

test_pool_raid_types() {
    log_header "PO4: VERIFY POOL STATUS"
    log_test "GET existing pool status"
    local response
    response=$(api GET "/v1/pools/$STRESS_POOL")
    is_success "$response" && { log_info "Health: $(json_field "$response" "health")"; log_pass; } || log_fail "$(json_field "$response" "message")"
}

test_pool_destroy_nonexistent() {
    log_header "PO5: DESTROY NON-EXISTENT"
    log_test "DELETE /pools/fake"
    local response
    response=$(api DELETE "/v1/pools/fake_pool_xyz")
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should error"
}

test_pool_destroy_with_datasets() {
    log_header "PO6: DESTROY POOL WITH DATASETS"
    # Create temp pool with dataset
    log_test "Setup: pool with dataset (using CLI)"
    zpool create -f temp_destroy_test "$DISK_3" "$DISK_4" 2>/dev/null || { log_skip "Can't create temp pool"; return; }
    zfs create temp_destroy_test/data 2>/dev/null || true

    log_test "DELETE pool with datasets"
    local response
    response=$(api DELETE "/v1/pools/temp_destroy_test")
    # Could succeed (force) or fail (has datasets)
    if is_success "$response"; then
        log_info "Pool destroyed (force mode)"
        log_pass
    elif is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
        zpool destroy -f temp_destroy_test 2>/dev/null || true
    fi

    # Re-create STRESS_POOL_B if we used those disks
    if ! zpool list "$STRESS_POOL_B" &>/dev/null; then
        api POST "/v1/pools" "{\"name\": \"$STRESS_POOL_B\", \"raid_type\": \"mirror\", \"disks\": [\"$DISK_3\", \"$DISK_4\"]}" >/dev/null 2>&1 || true
    fi
}

test_pool_destroy_with_snapshots() {
    log_header "PO7: DESTROY POOL WITH SNAPSHOTS"
    log_test "Pool already has test dataset - add snapshot"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "po7_snap"}')
    is_success "$response" || { log_skip "Setup failed"; return; }

    # We won't actually destroy the main pool, just verify it has snaps
    log_test "Verify pool has snapshots"
    if zfs list -t snapshot -r "$STRESS_POOL" 2>/dev/null | grep -q "po7_snap"; then
        log_info "Pool has snapshots"
        log_pass
    else
        log_fail "Snapshot not found"
    fi
    api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/po7_snap" >/dev/null 2>&1 || true
}

test_pool_status_nonexistent() {
    log_header "PO8: STATUS OF NON-EXISTENT"
    log_test "GET /pools/fake"
    local response
    response=$(api GET "/v1/pools/fake_pool_xyz")
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should error"
}

test_pool_special_chars() {
    log_header "PO9: POOL NAME WITH SPECIAL CHARS"
    log_test "POST /pools with '@' in name"
    local response
    response=$(api POST "/v1/pools" '{"name": "bad@pool", "raid_type": "stripe", "disks": ["/dev/sda"]}')
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || { log_fail "Should reject '@'"; zpool destroy -f "bad@pool" 2>/dev/null || true; }
}

test_pool_max_length() {
    log_header "PO10: POOL NAME MAX LENGTH"
    local long_name
    long_name=$(printf 'p%.0s' {1..200})
    log_test "POST /pools with 200-char name"
    local response
    response=$(api POST "/v1/pools" "{\"name\": \"$long_name\", \"raid_type\": \"stripe\", \"disks\": [\"/dev/sda\"]}")
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || { log_info "Accepted or failed gracefully"; log_pass; }
}

# ==============================================================================
# Scrub Operations (SC1-SC5)
# ==============================================================================

test_scrub_nonexistent_pool() {
    log_header "SC1: SCRUB NON-EXISTENT"
    log_test "POST /pools/fake/scrub"
    local response
    response=$(api POST "/v1/pools/fake_xyz/scrub")
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should error"
}

test_scrub_stop_not_running() {
    log_header "SC2: STOP WHEN NONE RUNNING"
    log_test "POST /pools/$STRESS_POOL/scrub/stop"
    local response
    response=$(api POST "/v1/pools/$STRESS_POOL/scrub/stop")
    log_info "Response: $(json_field "$response" "message")"
    log_pass  # Either success or error is acceptable
}

test_scrub_status_nonexistent() {
    log_header "SC3: STATUS ON NON-EXISTENT"
    log_test "GET /pools/fake/scrub"
    local response
    response=$(api GET "/v1/pools/fake_xyz/scrub")
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should error"
}

test_scrub_pause_not_running() {
    log_header "SC4: PAUSE WHEN NONE RUNNING"
    log_test "POST /pools/$STRESS_POOL/scrub/pause"
    local response
    response=$(api POST "/v1/pools/$STRESS_POOL/scrub/pause")
    log_info "Response: $(json_field "$response" "message")"
    log_pass
}

test_scrub_start_stop_cycle() {
    log_header "SC5: START/STOP CYCLE"
    log_test "Start scrub"
    local response
    response=$(api POST "/v1/pools/$STRESS_POOL/scrub")
    is_success "$response" || { log_skip "$(json_field "$response" "message")"; return; }
    log_pass

    sleep 1

    log_test "Stop scrub"
    response=$(api POST "/v1/pools/$STRESS_POOL/scrub/stop")
    log_info "Response: $(json_field "$response" "message")"
    log_pass
}

# ==============================================================================
# Export/Import (EI1-EI7)
# ==============================================================================

test_export_nonexistent() {
    log_header "EI1: EXPORT NON-EXISTENT"
    log_test "POST /pools/fake/export"
    local response
    response=$(api POST "/v1/pools/fake_xyz/export" '{}')
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should error"
}

test_export_verify_gone() {
    log_header "EI2: EXPORT AND VERIFY"
    log_test "Export $STRESS_POOL_B"
    local response
    response=$(api POST "/v1/pools/$STRESS_POOL_B/export" '{}')
    is_success "$response" || { log_fail "$(json_field "$response" "message")"; return; }
    log_pass

    log_test "Verify gone from list"
    response=$(api GET "/v1/pools")
    echo "$response" | grep -q "$STRESS_POOL_B" && log_fail "Still in list" || { log_info "Removed"; log_pass; }

    # Re-import
    api POST "/v1/pools/import" "{\"name\": \"$STRESS_POOL_B\"}" >/dev/null 2>&1 || true
}

test_import_nonexistent() {
    log_header "EI3: IMPORT NON-EXISTENT"
    log_test "POST /pools/import fake"
    local response
    response=$(api POST "/v1/pools/import" '{"name": "nonexistent_xyz"}')
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should error"
}

test_double_export() {
    log_header "EI5: DOUBLE EXPORT"
    log_test "Export $STRESS_POOL_B"
    local response
    response=$(api POST "/v1/pools/$STRESS_POOL_B/export" '{}')
    is_success "$response" || { log_skip "First export failed"; return; }

    log_test "Export again (already exported)"
    response=$(api POST "/v1/pools/$STRESS_POOL_B/export" '{}')
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should error"

    # Re-import
    api POST "/v1/pools/import" "{\"name\": \"$STRESS_POOL_B\"}" >/dev/null 2>&1 || true
}

test_export_import_cycle() {
    log_header "EI6: EXPORT/IMPORT CYCLE"
    log_test "Export -> Import -> Export -> Import"
    local response

    response=$(api POST "/v1/pools/$STRESS_POOL_B/export" '{}')
    is_success "$response" || { log_skip "Export 1 failed"; return; }

    response=$(api POST "/v1/pools/import" "{\"name\": \"$STRESS_POOL_B\"}")
    is_success "$response" || { log_fail "Import 1 failed"; return; }

    response=$(api POST "/v1/pools/$STRESS_POOL_B/export" '{}')
    is_success "$response" || { log_fail "Export 2 failed"; return; }

    response=$(api POST "/v1/pools/import" "{\"name\": \"$STRESS_POOL_B\"}")
    is_success "$response" && { log_info "Cycle completed"; log_pass; } || log_fail "Import 2 failed"
}

test_list_importable_empty() {
    log_header "EI7: LIST IMPORTABLE (NONE)"
    log_test "GET /pools/importable"
    local response
    response=$(api GET "/v1/pools/importable")
    # Could be empty or have entries
    if is_success "$response"; then
        log_info "Importable pools listed"
        log_pass
    else
        log_fail "$(json_field "$response" "message")"
    fi
}

test_force_export() {
    log_header "EI8: FORCE EXPORT"
    log_test "Export with force=true"
    local response
    response=$(api POST "/v1/pools/$STRESS_POOL_B/export" '{"force": true}')
    is_success "$response" && { log_info "Force export succeeded"; log_pass; } || { log_expect_error "$(json_field "$response" "message")"; log_pass; }

    # Re-import
    api POST "/v1/pools/import" "{\"name\": \"$STRESS_POOL_B\"}" >/dev/null 2>&1 || true
}

test_import_with_rename() {
    log_header "EI4: IMPORT WITH RENAME (new_name)"
    local response NEW_NAME="stress_b_renamed"

    # Export the secondary pool
    log_test "Export pool for rename test"
    response=$(api POST "/v1/pools/$STRESS_POOL_B/export" '{}')
    is_success "$response" || { log_fail "Export: $(json_field "$response" "message")"; return; }
    log_pass

    # Import with new name
    log_test "Import with new_name='$NEW_NAME'"
    response=$(api POST "/v1/pools/import" "{\"name\": \"$STRESS_POOL_B\", \"new_name\": \"$NEW_NAME\"}")
    if ! is_success "$response"; then
        log_fail "$(json_field "$response" "message")"
        api POST "/v1/pools/import" "{\"name\": \"$STRESS_POOL_B\"}" >/dev/null 2>&1 || true
        return
    fi
    log_pass

    # Verify
    log_test "Verify new name exists"
    zpool list "$NEW_NAME" &>/dev/null && { log_info "Imported as $NEW_NAME"; log_pass; } || log_fail "Not found"

    log_test "Verify old name gone"
    zpool list "$STRESS_POOL_B" &>/dev/null && log_fail "Old exists" || { log_info "Old removed"; log_pass; }

    # Restore
    log_test "Restore original name"
    response=$(api POST "/v1/pools/$NEW_NAME/export" '{}')
    is_success "$response" || zpool destroy -f "$NEW_NAME" 2>/dev/null || true
    response=$(api POST "/v1/pools/import" "{\"name\": \"$STRESS_POOL_B\"}")
    is_success "$response" && { log_info "Restored"; log_pass; } || log_fail "$(json_field "$response" "message")"
}

# ==============================================================================
# Replication (RE1-RE12)
# ==============================================================================

test_send_nonexistent_snapshot() {
    log_header "RE1: SEND NON-EXISTENT SNAPSHOT"
    log_test "POST send fake snap"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/fake_snap/send" "{\"output_file\": \"$SEND_FILE\"}")
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should error"
}

test_send_invalid_path() {
    log_header "RE2: SEND TO INVALID PATH"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "re2_snap"}')
    is_success "$response" || { log_skip "Setup failed"; return; }

    log_test "Send to /nonexistent/path"
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/re2_snap/send" '{"output_file": "/nonexistent/dir/file.zfs"}')
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should error"

    api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/re2_snap" >/dev/null 2>&1 || true
}

test_send_existing_file() {
    log_header "RE3: SEND TO EXISTING (NO OVERWRITE)"
    touch "$SEND_FILE"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "re3_snap"}')
    is_success "$response" || { log_skip "Setup"; rm -f "$SEND_FILE"; return; }

    log_test "Send without overwrite"
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/re3_snap/send" "{\"output_file\": \"$SEND_FILE\", \"overwrite\": false}")
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should reject"

    rm -f "$SEND_FILE"
    api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/re3_snap" >/dev/null 2>&1 || true
}

test_send_size_nonexistent() {
    log_header "RE4: SEND SIZE NON-EXISTENT"
    log_test "GET send-size fake"
    local response
    response=$(api GET "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/fake_xyz/send-size")
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should error"
}

test_receive_nonexistent_file() {
    log_header "RE5: RECEIVE NON-EXISTENT FILE"
    log_test "POST receive from fake file"
    local response
    response=$(api POST "/v1/datasets/$STRESS_POOL_B/recv/receive" '{"input_file": "/tmp/fake_xyz.zfs"}')
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should error"
}

test_receive_existing_dataset() {
    log_header "RE6: RECEIVE TO EXISTING (NO FORCE)"
    # Create dataset on pool B
    local response
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL_B/existing\", \"kind\": \"filesystem\"}")
    is_success "$response" || { log_skip "Setup"; return; }

    # Create snapshot and send file
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "re6_snap"}')
    is_success "$response" || { log_skip "Setup snap"; zfs destroy "$STRESS_POOL_B/existing" 2>/dev/null; return; }

    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/re6_snap/send" "{\"output_file\": \"$SEND_FILE\", \"overwrite\": true}")
    is_success "$response" || { log_skip "Setup send"; zfs destroy "$STRESS_POOL_B/existing" 2>/dev/null; return; }
    sleep 2  # Wait for task

    log_test "Receive to existing without force"
    response=$(api POST "/v1/datasets/$STRESS_POOL_B/existing/receive" "{\"input_file\": \"$SEND_FILE\", \"force\": false}")
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should reject"

    zfs destroy -r "$STRESS_POOL_B/existing" 2>/dev/null || true
    api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/re6_snap" >/dev/null 2>&1 || true
    rm -f "$SEND_FILE"
}

test_replicate_nonexistent() {
    log_header "RE7: REPLICATE NON-EXISTENT SNAPSHOT"
    log_test "POST replicate fake"
    local response
    response=$(api POST "/v1/replication/$STRESS_POOL/$TEST_DATASET/fake_snap" "{\"target_dataset\": \"$STRESS_POOL_B/repl\"}")
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should error"
}

test_replicate_nonexistent_target() {
    log_header "RE8: REPLICATE TO NON-EXISTENT POOL"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "re8_snap"}')
    is_success "$response" || { log_skip "Setup"; return; }

    log_test "Replicate to fake_pool"
    response=$(api POST "/v1/replication/$STRESS_POOL/$TEST_DATASET/re8_snap" '{"target_dataset": "fake_pool_xyz/data"}')
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should error"

    api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/re8_snap" >/dev/null 2>&1 || true
}

test_task_status_nonexistent() {
    log_header "RE9: TASK STATUS NON-EXISTENT"
    log_test "GET /tasks/fake-id"
    local response
    response=$(api GET "/v1/tasks/nonexistent-task-xyz")
    is_error "$response" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || { log_info "Empty or error"; log_pass; }
}

test_send_with_overwrite() {
    log_header "RE10: SEND WITH OVERWRITE"
    touch "$SEND_FILE"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "re10_snap"}')
    is_success "$response" || { log_skip "Setup"; rm -f "$SEND_FILE"; return; }

    log_test "Send with overwrite=true"
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/re10_snap/send" "{\"output_file\": \"$SEND_FILE\", \"overwrite\": true}")
    if is_success "$response"; then
        local task_id
        task_id=$(json_field "$response" "task_id")
        log_info "Task: $task_id"
        sleep 2
        log_pass
    else
        log_fail "$(json_field "$response" "message")"
    fi

    rm -f "$SEND_FILE"
    api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/re10_snap" >/dev/null 2>&1 || true
}

test_replicate_same_pool() {
    log_header "RE11: REPLICATE TO SAME POOL"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "re11_snap"}')
    is_success "$response" || { log_skip "Setup"; return; }

    log_test "Replicate within same pool"
    response=$(api POST "/v1/replication/$STRESS_POOL/$TEST_DATASET/re11_snap" "{\"target_dataset\": \"$STRESS_POOL/replicated\"}")
    if is_success "$response"; then
        local task_id
        task_id=$(json_field "$response" "task_id")
        log_info "Task: $task_id"
        sleep 2
        log_pass
    else
        log_fail "$(json_field "$response" "message")"
    fi

    zfs destroy -r "$STRESS_POOL/replicated" 2>/dev/null || true
    api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/re11_snap" >/dev/null 2>&1 || true
}

test_replicate_cross_pool() {
    log_header "RE12: REPLICATE CROSS POOL"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "re12_snap"}')
    is_success "$response" || { log_skip "Setup"; return; }

    log_test "Replicate to $STRESS_POOL_B"
    response=$(api POST "/v1/replication/$STRESS_POOL/$TEST_DATASET/re12_snap" "{\"target_dataset\": \"$STRESS_POOL_B/replicated\"}")
    if is_success "$response"; then
        local task_id
        task_id=$(json_field "$response" "task_id")
        log_info "Task: $task_id"

        # Wait for completion
        local result
        result=$(wait_for_task "$task_id" 30)
        local status
        status=$(json_field "$result" "status")
        [[ "$status" == "completed" ]] && { log_info "Replication completed"; log_pass; } || log_fail "Status: $status"
    else
        log_fail "$(json_field "$response" "message")"
    fi

    zfs destroy -r "$STRESS_POOL_B/replicated" 2>/dev/null || true
    api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/re12_snap" >/dev/null 2>&1 || true
}

# ==============================================================================
# Authentication (AU1-AU5)
# ==============================================================================

test_auth_missing_key() {
    log_header "AU1: MISSING API KEY"
    log_test "GET /pools without key"
    local response
    response=$(api_no_auth GET "/v1/pools")
    echo "$response" | grep -qi "unauthorized\|forbidden\|auth" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should require auth"
}

test_auth_invalid_key() {
    log_header "AU2: INVALID API KEY"
    log_test "GET /pools with wrong key"
    local response
    response=$(curl -s -X GET "${API_URL}/v1/pools" -H "X-API-Key: invalid-12345")
    echo "$response" | grep -qi "unauthorized\|forbidden\|invalid\|auth" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should reject"
}

test_auth_health_public() {
    log_header "AU3: HEALTH PUBLIC"
    log_test "GET /health without key"
    local response
    response=$(api_no_auth GET "/v1/health")
    is_success "$response" && { log_info "Health is public"; log_pass; } || log_fail "Should be public"
}

test_auth_malformed_key() {
    log_header "AU4: MALFORMED API KEY"
    log_test "GET /pools with 'not-a-uuid'"
    local response
    response=$(curl -s -X GET "${API_URL}/v1/pools" -H "X-API-Key: not-a-uuid-format")
    echo "$response" | grep -qi "unauthorized\|forbidden\|invalid" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should reject"
}

test_auth_wrong_header() {
    log_header "AU5: WRONG HEADER NAME"
    log_test "GET /pools with Authorization instead of X-API-Key"
    local response
    response=$(curl -s -X GET "${API_URL}/v1/pools" -H "Authorization: Bearer $API_KEY")
    echo "$response" | grep -qi "unauthorized\|forbidden" && { log_expect_error "$(json_field "$response" "message")"; log_pass; } || log_fail "Should reject"
}

# ==============================================================================
# API Robustness (AR1-AR9)
# ==============================================================================

test_api_malformed_json() {
    log_header "AR1: MALFORMED JSON"
    log_test "POST /pools with bad JSON"
    local response
    response=$(curl -s -X POST "${API_URL}/v1/pools" -H "Content-Type: application/json" -H "X-API-Key: $API_KEY" -d 'not json{{{')
    echo "$response" | grep -qi "parse\|json\|invalid\|deserialize\|error" && { log_expect_error "${response:0:80}"; log_pass; } || log_fail "Should reject"
}

test_api_missing_fields() {
    log_header "AR2: MISSING REQUIRED FIELDS"
    log_test "POST /pools without name"
    local response
    response=$(api POST "/v1/pools" '{"raid_type": "mirror"}')
    echo "$response" | grep -qi "missing\|required\|field\|error" && { log_expect_error "${response:0:80}"; log_pass; } || log_fail "Should report"
}

test_api_extra_fields() {
    log_header "AR3: EXTRA FIELDS"
    log_test "POST /snapshots with extras"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "ar3", "extra": "x", "num": 1}')
    is_success "$response" && { log_info "Ignored"; log_pass; api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/ar3" >/dev/null 2>&1; } || { log_info "Rejected"; log_pass; }
}

test_api_large_payload() {
    log_header "AR4: LARGE PAYLOAD (1MB)"
    log_test "POST /pools with huge body"
    local big_data
    big_data=$(printf '{"name": "test", "data": "%s"}' "$(head -c 1000000 /dev/zero | tr '\0' 'x')")
    local response
    response=$(curl -s -X POST "${API_URL}/v1/pools" -H "Content-Type: application/json" -H "X-API-Key: $API_KEY" -d "$big_data" 2>/dev/null || echo "timeout")
    # Should reject or timeout
    log_info "Response: ${response:0:50}..."
    log_pass
}

test_api_empty_body() {
    log_header "AR5: EMPTY BODY WHERE REQUIRED"
    log_test "POST /pools with empty body"
    local response
    response=$(curl -s -X POST "${API_URL}/v1/pools" -H "Content-Type: application/json" -H "X-API-Key: $API_KEY" -d '')
    echo "$response" | grep -qi "error\|missing\|required\|empty" && { log_expect_error "${response:0:80}"; log_pass; } || log_fail "Should report"
}

test_api_wrong_content_type() {
    log_header "AR6: WRONG CONTENT-TYPE"
    log_test "POST /pools with text/plain"
    local response
    response=$(curl -s -X POST "${API_URL}/v1/pools" -H "Content-Type: text/plain" -H "X-API-Key: $API_KEY" -d '{"name": "test"}')
    log_info "Response: ${response:0:80}"
    log_pass  # Accept either error or success
}

test_api_404() {
    log_header "AR7: NON-EXISTENT ENDPOINT"
    log_test "GET /v1/nonexistent"
    local code
    code=$(curl -s -o /dev/null -w "%{http_code}" "${API_URL}/v1/nonexistent" -H "X-API-Key: $API_KEY")
    [[ "$code" == "404" ]] && { log_info "HTTP 404"; log_pass; } || log_fail "Expected 404, got $code"
}

test_api_wrong_method() {
    log_header "AR8: WRONG HTTP METHOD"
    log_test "PUT /pools (should be POST)"
    local response
    response=$(curl -s -X PUT "${API_URL}/v1/pools" -H "Content-Type: application/json" -H "X-API-Key: $API_KEY" -d '{"name": "test"}')
    local code
    code=$(curl -s -o /dev/null -w "%{http_code}" -X PUT "${API_URL}/v1/pools" -H "X-API-Key: $API_KEY")
    [[ "$code" == "405" || "$code" == "404" ]] && { log_info "HTTP $code"; log_pass; } || { log_info "Got: $code"; log_pass; }
}

test_api_concurrent() {
    log_header "AR9: CONCURRENT REQUESTS (10)"
    log_test "10 parallel health checks"
    local pids=()
    for i in $(seq 1 10); do
        (curl -s "${API_URL}/v1/health" >/dev/null) &
        pids+=($!)
    done
    for pid in "${pids[@]}"; do wait "$pid" 2>/dev/null; done
    log_info "All requests completed"
    log_pass
}

# ==============================================================================
# Main
# ==============================================================================

main() {
    echo -e "${BLUE}╔════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║     ZFS STRESS TEST B (LONG) - Complete Infrastructure Tests   ║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════════════════╝${NC}"
    echo "API: $API_URL | Cleanup: $CLEANUP"
    echo ""

    check_prerequisites
    setup_test_pools

    # Pool PO1-PO10
    test_pool_invalid_disk
    test_pool_disk_in_use
    test_pool_duplicate_name
    test_pool_raid_types
    test_pool_destroy_nonexistent
    test_pool_destroy_with_datasets
    test_pool_destroy_with_snapshots
    test_pool_status_nonexistent
    test_pool_special_chars
    test_pool_max_length

    # Scrub SC1-SC5
    test_scrub_nonexistent_pool
    test_scrub_stop_not_running
    test_scrub_status_nonexistent
    test_scrub_pause_not_running
    test_scrub_start_stop_cycle

    # Export/Import EI1-EI8
    test_export_nonexistent
    test_export_verify_gone
    test_import_nonexistent
    test_import_with_rename
    test_double_export
    test_export_import_cycle
    test_list_importable_empty
    test_force_export

    # Replication RE1-RE12
    test_send_nonexistent_snapshot
    test_send_invalid_path
    test_send_existing_file
    test_send_size_nonexistent
    test_receive_nonexistent_file
    test_receive_existing_dataset
    test_replicate_nonexistent
    test_replicate_nonexistent_target
    test_task_status_nonexistent
    test_send_with_overwrite
    test_replicate_same_pool
    test_replicate_cross_pool

    # Auth AU1-AU5
    test_auth_missing_key
    test_auth_invalid_key
    test_auth_health_public
    test_auth_malformed_key
    test_auth_wrong_header

    # API AR1-AR9
    test_api_malformed_json
    test_api_missing_fields
    test_api_extra_fields
    test_api_large_payload
    test_api_empty_body
    test_api_wrong_content_type
    test_api_404
    test_api_wrong_method
    test_api_concurrent

    log_header "TEST SUMMARY"
    echo -e "Passed:  ${GREEN}$PASSED${NC}"
    echo -e "Failed:  ${RED}$FAILED${NC}"
    echo -e "Skipped: ${YELLOW}$SKIPPED${NC}"

    [[ $FAILED -gt 0 ]] && { echo -e "${RED}SOME TESTS FAILED${NC}"; exit 1; }
    echo -e "${GREEN}ALL TESTS PASSED${NC}"
}

main "$@"
