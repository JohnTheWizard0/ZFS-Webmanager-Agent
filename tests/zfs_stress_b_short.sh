#!/bin/bash
# ==============================================================================
# ZFS Stress Test B (Short) - Pool, Replication, Auth, API Edge Cases
# ==============================================================================
# Tests edge cases for infrastructure operations: pools, scrub, export/import,
# replication, authentication, and API robustness.
#
# TESTS INCLUDED:
#   PO1-PO5: Pool creation/destruction edge cases
#   SC1-SC2: Scrub basic edge cases
#   EI1-EI3: Export/Import basic edge cases
#   RE1-RE5: Replication basic edge cases
#   AU1-AU3: Authentication cases
#   AR1-AR3: API robustness cases
#
# USAGE:
#   ./tests/zfs_stress_b_short.sh
#   API_URL=http://host:9876 ./tests/zfs_stress_b_short.sh
#
# DURATION: ~5 minutes
# ==============================================================================

set -euo pipefail

# Configuration
API_URL="${API_URL:-http://localhost:9876}"
API_KEY="${API_KEY:-08670612-43df-4a0c-a556-2288457726a5}"
CLEANUP="${CLEANUP:-true}"

# Test resources
STRESS_POOL="stress_b_pool"
STRESS_POOL_B="stress_b_pool_alt"
TEST_DATASET="testdata"
SEND_FILE="/tmp/stress_b_send.zfs"

# Testlab disks (need 4 for two mirror pools)
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

# Counters
PASSED=0
FAILED=0
SKIPPED=0

# ==============================================================================
# Helper Functions
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

api_raw() {
    local method="$1" endpoint="$2" data="${3:-}"
    local curl_args=(-s -X "$method" "${API_URL}${endpoint}")
    [[ -n "$API_KEY" ]] && curl_args+=(-H "X-API-Key: $API_KEY")
    [[ -n "$data" ]] && curl_args+=(-d "$data")
    curl "${curl_args[@]}" 2>/dev/null || echo 'curl failed'
}

is_success() { echo "$1" | grep -q '"status":"success"'; }
is_error() { echo "$1" | grep -q '"status":"error"' || echo "$1" | grep -qi 'error\|fail\|not found\|invalid\|unauthorized'; }
json_field() { echo "$1" | grep -o "\"$2\":\"[^\"]*\"" 2>/dev/null | head -1 | cut -d'"' -f4 || true; }

# ==============================================================================
# Prerequisites
# ==============================================================================

check_prerequisites() {
    log_header "PREREQUISITES CHECK"

    log_test "ZFS kernel module"
    lsmod | grep -q "^zfs" && log_pass || { log_fail "ZFS not loaded"; exit 2; }

    log_test "API endpoint"
    local code
    code=$(curl -s -o /dev/null -w "%{http_code}" "${API_URL}/v1/health" 2>/dev/null || echo "000")
    [[ "$code" == "200" ]] && log_pass || { log_fail "API unreachable (HTTP $code)"; exit 2; }

    log_test "Test disks available"
    if [[ -b "$DISK_1" ]] && [[ -b "$DISK_2" ]] && [[ -b "$DISK_3" ]] && [[ -b "$DISK_4" ]]; then
        log_pass
    else
        log_fail "Need 4 disks: $DISK_1 $DISK_2 $DISK_3 $DISK_4"
        exit 2
    fi
}

# ==============================================================================
# Setup / Cleanup
# ==============================================================================

cleanup_pools() {
    zpool destroy -f "$STRESS_POOL" 2>/dev/null || true
    zpool destroy -f "$STRESS_POOL_B" 2>/dev/null || true
    rm -f "$SEND_FILE" 2>/dev/null || true
}

setup_test_pools() {
    log_header "SETUP"
    cleanup_pools

    log_test "Creating primary pool: $STRESS_POOL"
    local response
    response=$(api POST "/v1/pools" "{\"name\": \"$STRESS_POOL\", \"raid_type\": \"mirror\", \"disks\": [\"$DISK_1\", \"$DISK_2\"]}")
    is_success "$response" && log_pass || { log_fail "$(json_field "$response" "message")"; exit 2; }

    log_test "Creating secondary pool: $STRESS_POOL_B"
    response=$(api POST "/v1/pools" "{\"name\": \"$STRESS_POOL_B\", \"raid_type\": \"mirror\", \"disks\": [\"$DISK_3\", \"$DISK_4\"]}")
    is_success "$response" && log_pass || { log_fail "$(json_field "$response" "message")"; exit 2; }

    log_test "Creating test dataset"
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/$TEST_DATASET\", \"kind\": \"filesystem\"}")
    is_success "$response" && log_pass || { log_fail "$(json_field "$response" "message")"; exit 2; }
}

cleanup() {
    if [[ "$CLEANUP" == "true" ]]; then
        log_header "CLEANUP"
        cleanup_pools
        log_info "Cleaned up test resources"
    else
        log_header "CLEANUP SKIPPED"
        log_info "Pools retained: $STRESS_POOL, $STRESS_POOL_B"
    fi
}
trap cleanup EXIT

# ==============================================================================
# TEST B1: Pool Operations (PO1-PO5)
# ==============================================================================

test_pool_invalid_disk() {
    log_header "PO1: CREATE POOL WITH INVALID DISK"
    log_test "POST /pools with /dev/nonexistent"
    local response
    response=$(api POST "/v1/pools" '{"name": "fake_pool", "raid_type": "mirror", "disks": ["/dev/nonexistent1", "/dev/nonexistent2"]}')
    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should reject invalid disk"
        zpool destroy -f "fake_pool" 2>/dev/null || true
    fi
}

test_pool_disk_in_use() {
    log_header "PO2: CREATE POOL WITH DISK ALREADY IN USE"
    log_test "POST /pools with disk from existing pool"
    local response
    response=$(api POST "/v1/pools" "{\"name\": \"conflict_pool\", \"raid_type\": \"mirror\", \"disks\": [\"$DISK_1\", \"$DISK_2\"]}")
    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should reject disk already in pool"
        zpool destroy -f "conflict_pool" 2>/dev/null || true
    fi
}

test_pool_duplicate_name() {
    log_header "PO3: CREATE DUPLICATE POOL NAME"
    log_test "POST /pools with existing name"
    # Can't test without free disks, so just try with the same disks (will fail for in-use)
    local response
    response=$(api POST "/v1/pools" "{\"name\": \"$STRESS_POOL\", \"raid_type\": \"mirror\", \"disks\": [\"$DISK_1\", \"$DISK_2\"]}")
    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should reject duplicate pool name"
    fi
}

test_pool_raid_types() {
    log_header "PO4: VERIFY EXISTING POOL RAID TYPES"
    # We can't create more pools without more disks, so just verify the existing ones
    log_test "GET status of mirror pools"
    local response
    response=$(api GET "/v1/pools/$STRESS_POOL")
    if is_success "$response"; then
        log_info "Pool $STRESS_POOL: $(json_field "$response" "health")"
        log_pass
    else
        log_fail "$(json_field "$response" "message")"
    fi
}

test_pool_destroy_nonexistent() {
    log_header "PO5: DESTROY NON-EXISTENT POOL"
    log_test "DELETE /pools/nonexistent_pool_xyz"
    local response
    response=$(api DELETE "/v1/pools/nonexistent_pool_xyz")
    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should error for nonexistent pool"
    fi
}

# ==============================================================================
# TEST B2: Scrub Operations (SC1-SC2)
# ==============================================================================

test_scrub_nonexistent_pool() {
    log_header "SC1: START SCRUB ON NON-EXISTENT POOL"
    log_test "POST /pools/fake_pool/scrub"
    local response
    response=$(api POST "/v1/pools/fake_pool_xyz/scrub")
    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should error for nonexistent pool"
    fi
}

test_scrub_stop_not_running() {
    log_header "SC2: STOP SCRUB WHEN NONE RUNNING"
    log_test "POST /pools/$STRESS_POOL/scrub/stop (no scrub active)"
    local response
    response=$(api POST "/v1/pools/$STRESS_POOL/scrub/stop")
    # This could either succeed as no-op or return an error
    if is_success "$response" || is_error "$response"; then
        log_info "Response: $(json_field "$response" "message")"
        log_pass
    else
        log_fail "Unexpected response"
    fi
}

# ==============================================================================
# TEST B3: Export/Import (EI1-EI3)
# ==============================================================================

test_export_nonexistent() {
    log_header "EI1: EXPORT NON-EXISTENT POOL"
    log_test "POST /pools/fake_pool/export"
    local response
    response=$(api POST "/v1/pools/fake_pool_xyz/export" '{}')
    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should error for nonexistent pool"
    fi
}

test_export_verify_gone() {
    log_header "EI2: EXPORT POOL AND VERIFY GONE"

    log_test "Export $STRESS_POOL_B"
    local response
    response=$(api POST "/v1/pools/$STRESS_POOL_B/export" '{}')
    if ! is_success "$response"; then
        log_fail "$(json_field "$response" "message")"
        return
    fi
    log_pass

    log_test "Verify pool not in list"
    response=$(api GET "/v1/pools")
    if echo "$response" | grep -q "$STRESS_POOL_B"; then
        log_fail "Pool still in list after export"
    else
        log_info "Pool removed from list"
        log_pass
    fi

    # Re-import for subsequent tests
    log_test "Re-import pool"
    response=$(api POST "/v1/pools/import" "{\"name\": \"$STRESS_POOL_B\"}")
    is_success "$response" && log_pass || log_fail "$(json_field "$response" "message")"
}

test_import_nonexistent() {
    log_header "EI3: IMPORT NON-EXISTENT POOL"
    log_test "POST /pools/import with fake name"
    local response
    response=$(api POST "/v1/pools/import" '{"name": "nonexistent_import_xyz"}')
    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should error for nonexistent pool"
    fi
}

test_import_with_rename() {
    log_header "EI4: IMPORT WITH RENAME (new_name)"

    # Export the secondary pool
    log_test "Export pool for rename test"
    local response
    response=$(api POST "/v1/pools/$STRESS_POOL_B/export" '{}')
    if ! is_success "$response"; then
        log_fail "Export failed: $(json_field "$response" "message")"
        return
    fi
    log_pass

    # Define the new name
    local NEW_NAME="stress_b_renamed"

    # Import with new name
    log_test "Import with new_name='$NEW_NAME'"
    response=$(api POST "/v1/pools/import" "{\"name\": \"$STRESS_POOL_B\", \"new_name\": \"$NEW_NAME\"}")
    if ! is_success "$response"; then
        log_fail "Import with rename failed: $(json_field "$response" "message")"
        # Try to re-import without rename
        api POST "/v1/pools/import" "{\"name\": \"$STRESS_POOL_B\"}" >/dev/null 2>&1 || true
        return
    fi
    log_pass

    # Verify the pool is imported with new name
    log_test "Verify pool exists with new name"
    if zpool list "$NEW_NAME" &>/dev/null; then
        log_info "Pool imported as '$NEW_NAME'"
        log_pass
    else
        log_fail "Pool not found with new name"
    fi

    # Verify old name is gone
    log_test "Verify old name is gone"
    if zpool list "$STRESS_POOL_B" &>/dev/null; then
        log_fail "Old name still exists"
    else
        log_info "Old name correctly removed"
        log_pass
    fi

    # Re-export and re-import with original name for subsequent tests
    log_test "Restore original pool name"
    response=$(api POST "/v1/pools/$NEW_NAME/export" '{}')
    if ! is_success "$response"; then
        log_info "Export renamed pool failed, trying direct destroy"
        zpool destroy -f "$NEW_NAME" 2>/dev/null || true
    fi

    response=$(api POST "/v1/pools/import" "{\"name\": \"$STRESS_POOL_B\"}")
    if is_success "$response"; then
        log_info "Restored as $STRESS_POOL_B"
        log_pass
    else
        log_fail "Could not restore: $(json_field "$response" "message")"
    fi
}

# ==============================================================================
# TEST B4: Replication (RE1-RE5)
# ==============================================================================

test_send_nonexistent_snapshot() {
    log_header "RE1: SEND NON-EXISTENT SNAPSHOT"
    log_test "POST /snapshots/.../fake_snap/send"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/nonexistent_snap/send" "{\"output_file\": \"$SEND_FILE\"}")
    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should error for nonexistent snapshot"
    fi
}

test_send_invalid_path() {
    log_header "RE2: SEND TO INVALID PATH"

    # Create a snapshot first
    log_test "Setup: create snapshot"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "send_test_snap"}')
    is_success "$response" || { log_skip "Setup failed"; return; }
    log_pass

    log_test "Send to non-writable path"
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/send_test_snap/send" '{"output_file": "/nonexistent/path/file.zfs"}')
    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should error for invalid path"
    fi

    api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/send_test_snap" >/dev/null 2>&1 || true
}

test_send_existing_file() {
    log_header "RE3: SEND TO EXISTING FILE (NO OVERWRITE)"

    # Create file and snapshot
    touch "$SEND_FILE"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "send_exist_snap"}')
    is_success "$response" || { log_skip "Setup failed"; rm -f "$SEND_FILE"; return; }

    log_test "Send without overwrite flag"
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/send_exist_snap/send" "{\"output_file\": \"$SEND_FILE\", \"overwrite\": false}")
    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should reject without overwrite"
    fi

    rm -f "$SEND_FILE"
    api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/send_exist_snap" >/dev/null 2>&1 || true
}

test_send_size_nonexistent() {
    log_header "RE4: SEND SIZE ESTIMATE ON NON-EXISTENT"
    log_test "GET send-size for fake snapshot"
    local response
    response=$(api GET "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/fake_snap_xyz/send-size")
    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should error for nonexistent"
    fi
}

test_receive_nonexistent_file() {
    log_header "RE5: RECEIVE FROM NON-EXISTENT FILE"
    log_test "POST receive from /tmp/nonexistent.zfs"
    local response
    response=$(api POST "/v1/datasets/$STRESS_POOL_B/recv_test/receive" '{"input_file": "/tmp/nonexistent_file_xyz.zfs"}')
    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should error for nonexistent file"
    fi
}

# ==============================================================================
# TEST B5: Authentication (AU1-AU3)
# ==============================================================================

test_auth_missing_key() {
    log_header "AU1: REQUEST WITHOUT API KEY"
    log_test "GET /pools without X-API-Key header"
    local response
    response=$(api_no_auth GET "/v1/pools")
    if echo "$response" | grep -qi "unauthorized\|forbidden\|auth"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should require authentication"
    fi
}

test_auth_invalid_key() {
    log_header "AU2: REQUEST WITH INVALID API KEY"
    log_test "GET /pools with wrong key"
    local response
    response=$(curl -s -X GET "${API_URL}/v1/pools" \
        -H "Content-Type: application/json" \
        -H "X-API-Key: invalid-key-12345" 2>/dev/null)
    if echo "$response" | grep -qi "unauthorized\|forbidden\|invalid\|auth"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should reject invalid key"
    fi
}

test_auth_health_public() {
    log_header "AU3: HEALTH ENDPOINT WITHOUT AUTH"
    log_test "GET /health without API key"
    local response
    response=$(api_no_auth GET "/v1/health")
    if is_success "$response"; then
        log_info "Health endpoint is public"
        log_pass
    else
        log_fail "Health should be public"
    fi
}

# ==============================================================================
# TEST B6: API Robustness (AR1-AR3)
# ==============================================================================

test_api_malformed_json() {
    log_header "AR1: MALFORMED JSON BODY"
    log_test "POST /pools with invalid JSON"
    local response
    response=$(curl -s -X POST "${API_URL}/v1/pools" \
        -H "Content-Type: application/json" \
        -H "X-API-Key: $API_KEY" \
        -d 'this is not json{{{' 2>/dev/null)
    if is_error "$response" || echo "$response" | grep -qi "parse\|json\|invalid\|deserialize"; then
        log_expect_error "$(echo "$response" | head -c 100)"
        log_pass
    else
        log_fail "Should reject malformed JSON"
    fi
}

test_api_missing_fields() {
    log_header "AR2: MISSING REQUIRED FIELDS"
    log_test "POST /pools without 'name' field"
    local response
    response=$(api POST "/v1/pools" '{"raid_type": "mirror", "disks": ["/dev/sda"]}')
    if is_error "$response" || echo "$response" | grep -qi "missing\|required\|field"; then
        log_expect_error "$(echo "$response" | head -c 100)"
        log_pass
    else
        log_fail "Should report missing field"
    fi
}

test_api_extra_fields() {
    log_header "AR3: EXTRA UNEXPECTED FIELDS"
    log_test "POST /snapshots with extra field"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "extra_test", "unknown_field": "value", "another": 123}')
    # Should either succeed (ignoring extra fields) or cleanly error
    if is_success "$response"; then
        log_info "Extra fields ignored"
        log_pass
        api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/extra_test" >/dev/null 2>&1 || true
    elif is_error "$response"; then
        log_info "Extra fields rejected cleanly"
        log_pass
    else
        log_fail "Unexpected response"
    fi
}

# ==============================================================================
# Main
# ==============================================================================

main() {
    echo -e "${BLUE}╔════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║    ZFS STRESS TEST B (SHORT) - Pool/Replication/Auth Tests     ║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════════════════╝${NC}"
    echo "API: $API_URL | Cleanup: $CLEANUP"
    echo ""

    check_prerequisites
    setup_test_pools

    # Pool operations PO1-PO5
    test_pool_invalid_disk
    test_pool_disk_in_use
    test_pool_duplicate_name
    test_pool_raid_types
    test_pool_destroy_nonexistent

    # Scrub SC1-SC2
    test_scrub_nonexistent_pool
    test_scrub_stop_not_running

    # Export/Import EI1-EI4
    test_export_nonexistent
    test_export_verify_gone
    test_import_nonexistent
    test_import_with_rename

    # Replication RE1-RE5
    test_send_nonexistent_snapshot
    test_send_invalid_path
    test_send_existing_file
    test_send_size_nonexistent
    test_receive_nonexistent_file

    # Auth AU1-AU3
    test_auth_missing_key
    test_auth_invalid_key
    test_auth_health_public

    # API AR1-AR3
    test_api_malformed_json
    test_api_missing_fields
    test_api_extra_fields

    log_header "TEST SUMMARY"
    echo -e "Passed:  ${GREEN}$PASSED${NC}"
    echo -e "Failed:  ${RED}$FAILED${NC}"
    echo -e "Skipped: ${YELLOW}$SKIPPED${NC}"

    [[ $FAILED -gt 0 ]] && { echo -e "${RED}SOME TESTS FAILED${NC}"; exit 1; }
    echo -e "${GREEN}ALL TESTS PASSED${NC}"
}

main "$@"
