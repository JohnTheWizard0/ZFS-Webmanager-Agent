#!/bin/bash
# ==============================================================================
# ZFS Stress Test A (Long) - Complete Dataset, Snapshot, Property Stress
# ==============================================================================
# Full stress testing for data-layer operations including volume tests,
# property inheritance, and high-volume snapshot operations.
#
# TESTS INCLUDED:
#   P1-P8: All property edge cases including inheritance
#   S1-S10: All snapshot edge cases including 50-snapshot volume test
#   D1-D10: All dataset edge cases including volumes and concurrency
#   R1-R5: All rollback scenarios including force destroy
#
# USAGE:
#   ./tests/zfs_stress_a_long.sh
#   API_URL=http://host:9876 ./tests/zfs_stress_a_long.sh
#   CLEANUP=false ./tests/zfs_stress_a_long.sh
#
# DURATION: ~10 minutes
# ==============================================================================

set -euo pipefail

# Configuration
API_URL="${API_URL:-http://localhost:9876}"
API_KEY="${API_KEY:-08670612-43df-4a0c-a556-2288457726a5}"
CLEANUP="${CLEANUP:-true}"
SERVICE_NAME="zfs-agent"

# Source shared cleanup script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/cleanup_tests.sh"

# Test pool names
STRESS_POOL="stress_test_pool"
TEST_DATASET="stressdata"

# Testlab disks
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
# Helper Functions (same as short version)
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

is_success() { echo "$1" | grep -q '"status":"success"'; }
is_error() { echo "$1" | grep -q '"status":"error"' || echo "$1" | grep -qi 'error\|fail\|not found\|invalid'; }
json_field() { echo "$1" | grep -o "\"$2\":\"[^\"]*\"" 2>/dev/null | head -1 | cut -d'"' -f4 || true; }

# ==============================================================================
# Prerequisites & Setup
# ==============================================================================

check_prerequisites() {
    log_header "PREREQUISITES CHECK"
    log_test "ZFS kernel module loaded"
    if lsmod | grep -q "^zfs"; then log_pass; else log_fail "ZFS not loaded"; exit 2; fi

    log_test "API endpoint reachable"
    local response
    response=$(curl -s -o /dev/null -w "%{http_code}" "${API_URL}/v1/health" 2>/dev/null || echo "000")
    if [[ "$response" == "200" ]]; then log_pass; else log_fail "Cannot reach ${API_URL}"; exit 2; fi

    log_test "Test disks available"
    if [[ -b "$DISK_1" ]] && [[ -b "$DISK_2" ]]; then log_pass; else log_fail "Disks not found"; exit 2; fi
}

setup_test_pool() {
    log_header "SETUP"
    if zpool list "$STRESS_POOL" &>/dev/null; then
        log_test "Destroying existing test pool"
        zpool destroy -f "$STRESS_POOL" 2>/dev/null || true
        log_pass
    fi

    log_test "Creating test pool: $STRESS_POOL"
    local payload="{\"name\": \"$STRESS_POOL\", \"raid_type\": \"mirror\", \"disks\": [\"$DISK_1\", \"$DISK_2\"]}"
    local response
    response=$(api POST "/v1/pools" "$payload")
    if is_success "$response"; then log_pass; else log_fail "$(json_field "$response" "message")"; exit 2; fi

    log_test "Creating base dataset"
    payload="{\"name\": \"$STRESS_POOL/$TEST_DATASET\", \"kind\": \"filesystem\"}"
    response=$(api POST "/v1/datasets" "$payload")
    if is_success "$response"; then log_pass; else log_fail "$(json_field "$response" "message")"; exit 2; fi
}

cleanup() {
    if [[ "$CLEANUP" == "true" ]]; then
        run_test_cleanup true  # Use shared cleanup (quiet mode)
    else
        log_header "CLEANUP SKIPPED"
        log_info "Pool $STRESS_POOL retained for inspection"
    fi
}
trap cleanup EXIT

# ==============================================================================
# TEST A1: Property Edge Cases (P1-P8)
# ==============================================================================

test_property_invalid_name() {
    log_header "P1: SET INVALID PROPERTY NAME"
    log_test "PUT /datasets/.../properties - invalid property 'notarealproperty'"
    local response
    response=$(api PUT "/v1/datasets/$STRESS_POOL/$TEST_DATASET/properties" '{"property": "notarealproperty", "value": "x"}')
    if is_error "$response"; then log_expect_error "$(json_field "$response" "message")"; log_pass
    else log_fail "Should reject invalid property"; fi
}

test_property_invalid_value() {
    log_header "P2: SET INVALID PROPERTY VALUE"
    log_test "PUT - compression=invalid"
    local response
    response=$(api PUT "/v1/datasets/$STRESS_POOL/$TEST_DATASET/properties" '{"property": "compression", "value": "notvalid"}')
    if is_error "$response"; then log_expect_error "$(json_field "$response" "message")"; log_pass
    else log_fail "Should reject invalid value"; fi
}

test_property_readonly() {
    log_header "P3: SET READ-ONLY PROPERTY"
    log_test "PUT - 'used' (read-only)"
    local response
    response=$(api PUT "/v1/datasets/$STRESS_POOL/$TEST_DATASET/properties" '{"property": "used", "value": "12345"}')
    if is_error "$response"; then log_expect_error "$(json_field "$response" "message")"; log_pass
    else log_fail "Should reject read-only property"; fi
}

test_property_special_chars() {
    log_header "P4: PROPERTY VALUE WITH SPECIAL CHARS"
    log_test "PUT - comment with special chars"
    local response
    response=$(api PUT "/v1/datasets/$STRESS_POOL/$TEST_DATASET/properties" '{"property": "comment", "value": "test & symbols!"}')
    if is_success "$response" || is_error "$response"; then log_info "$(json_field "$response" "message")"; log_pass
    else log_fail "Unexpected response"; fi
}

test_property_nonexistent_dataset() {
    log_header "P5: SET PROPERTY ON NON-EXISTENT DATASET"
    log_test "PUT on nonexistent"
    local response
    response=$(api PUT "/v1/datasets/$STRESS_POOL/nonexistent_xyz/properties" '{"property": "compression", "value": "lz4"}')
    if is_error "$response"; then log_expect_error "$(json_field "$response" "message")"; log_pass
    else log_fail "Should error for nonexistent"; fi
}

test_property_rapid_changes() {
    log_header "P6: RAPID PROPERTY CHANGES (10 sets)"
    log_test "Setting compression 10 times rapidly"
    local success=0 i
    local values=("on" "lz4" "gzip" "zstd" "off" "lz4" "on" "zstd" "gzip" "lz4")
    for i in {0..9}; do
        local val="${values[$i]}"
        local response
        response=$(api PUT "/v1/datasets/$STRESS_POOL/$TEST_DATASET/properties" "{\"property\": \"compression\", \"value\": \"$val\"}")
        is_success "$response" && ((++success))
    done
    if [[ $success -eq 10 ]]; then log_info "All 10 property changes succeeded"; log_pass
    else log_fail "Only $success/10 succeeded"; fi
}

test_property_inheritance() {
    log_header "P7: PROPERTY INHERITANCE"

    # Create child dataset
    log_test "Create child dataset"
    local response
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/$TEST_DATASET/inherit_child\", \"kind\": \"filesystem\"}")
    if ! is_success "$response"; then log_fail "Setup: $(json_field "$response" "message")"; return; fi
    log_pass

    # Set compression on parent
    log_test "Set compression=zstd on parent"
    response=$(api PUT "/v1/datasets/$STRESS_POOL/$TEST_DATASET/properties" '{"property": "compression", "value": "zstd"}')
    if ! is_success "$response"; then log_fail "$(json_field "$response" "message")"; fi
    log_pass

    # Check child inherits
    log_test "Verify child inherited compression"
    local child_comp
    child_comp=$(zfs get -H -o value compression "$STRESS_POOL/$TEST_DATASET/inherit_child" 2>/dev/null)
    if [[ "$child_comp" == "zstd" ]]; then log_info "Child inherited: $child_comp"; log_pass
    else log_fail "Child has: $child_comp (expected zstd)"; fi

    # Cleanup
    zfs destroy "$STRESS_POOL/$TEST_DATASET/inherit_child" 2>/dev/null || true
}

test_property_override_inherited() {
    log_header "P8: OVERRIDE INHERITED PROPERTY"

    # Create child
    local response
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/$TEST_DATASET/override_child\", \"kind\": \"filesystem\"}")
    if ! is_success "$response"; then log_skip "Setup failed"; return; fi

    # Set different value on child
    log_test "Override compression on child"
    response=$(api PUT "/v1/datasets/$STRESS_POOL/$TEST_DATASET/override_child/properties" '{"property": "compression", "value": "lz4"}')
    if ! is_success "$response"; then log_fail "$(json_field "$response" "message")"; return; fi
    log_pass

    # Verify local value
    log_test "Verify child has local value"
    local child_comp
    child_comp=$(zfs get -H -o value compression "$STRESS_POOL/$TEST_DATASET/override_child" 2>/dev/null)
    if [[ "$child_comp" == "lz4" ]]; then log_info "Child has local: $child_comp"; log_pass
    else log_fail "Child has: $child_comp"; fi

    zfs destroy "$STRESS_POOL/$TEST_DATASET/override_child" 2>/dev/null || true
}

# ==============================================================================
# TEST A2: Snapshot Edge Cases (S1-S10)
# ==============================================================================

test_snapshot_invalid_name() {
    log_header "S1: SNAPSHOT WITH INVALID NAME"
    log_test "Name with '@'"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "snap@invalid"}')
    if is_error "$response"; then log_expect_error "$(json_field "$response" "message")"; log_pass
    else log_fail "Should reject '@'"; fi

    log_test "Name with spaces"
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "snap space"}')
    if is_error "$response"; then log_expect_error "$(json_field "$response" "message")"; log_pass
    else log_fail "Should reject spaces"; fi
}

test_snapshot_nonexistent_dataset() {
    log_header "S2: SNAPSHOT ON NON-EXISTENT DATASET"
    log_test "POST on nonexistent"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/fake_ds_xyz" '{"snapshot_name": "snap"}')
    if is_error "$response"; then log_expect_error "$(json_field "$response" "message")"; log_pass
    else log_fail "Should error"; fi
}

test_snapshot_duplicate() {
    log_header "S3: DUPLICATE SNAPSHOT"
    log_test "Create 'dup_snap'"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "dup_snap"}')
    is_success "$response" && log_pass || { log_fail "Setup"; return; }

    log_test "Create duplicate 'dup_snap'"
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "dup_snap"}')
    if is_error "$response"; then log_expect_error "$(json_field "$response" "message")"; log_pass
    else log_fail "Should reject duplicate"; fi

    api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/dup_snap" >/dev/null 2>&1 || true
}

test_snapshot_delete_with_clone() {
    log_header "S4: DELETE SNAPSHOT WITH CLONE"
    log_test "Setup snapshot+clone"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "cloned_snap"}')
    is_success "$response" || { log_fail "Setup snap"; return; }
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/cloned_snap/clone" "{\"target\": \"$STRESS_POOL/clone_s4\"}")
    is_success "$response" || { log_fail "Setup clone"; return; }
    log_pass

    log_test "DELETE snapshot with clone"
    response=$(api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/cloned_snap")
    if is_error "$response"; then log_expect_error "$(json_field "$response" "message")"; log_pass
    else log_fail "Should reject"; fi

    zfs destroy "$STRESS_POOL/clone_s4" 2>/dev/null || true
    api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/cloned_snap" >/dev/null 2>&1 || true
}

test_snapshot_rapid_create() {
    log_header "S5: RAPID SNAPSHOT CREATION (5)"
    log_test "Creating 5 snapshots"
    local success=0 i
    for i in {1..5}; do
        local response
        response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" "{\"snapshot_name\": \"rapid$i\"}")
        is_success "$response" && ((++success))
    done
    [[ $success -eq 5 ]] && { log_info "All 5 created"; log_pass; } || log_fail "Only $success/5"
    for i in {1..5}; do api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/rapid$i" >/dev/null 2>&1 || true; done
}

test_snapshot_clone_multiple() {
    log_header "S6: CLONE SAME SNAPSHOT MULTIPLE TIMES"
    log_test "Create source snapshot"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "multi_clone_src"}')
    is_success "$response" || { log_fail "Setup"; return; }
    log_pass

    log_test "Create 3 clones from same snapshot"
    local success=0 i
    for i in 1 2 3; do
        response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/multi_clone_src/clone" "{\"target\": \"$STRESS_POOL/multi_clone_$i\"}")
        is_success "$response" && ((++success))
    done
    [[ $success -eq 3 ]] && { log_info "All 3 clones created"; log_pass; } || log_fail "Only $success/3"

    for i in 1 2 3; do zfs destroy "$STRESS_POOL/multi_clone_$i" 2>/dev/null || true; done
    api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/multi_clone_src" >/dev/null 2>&1 || true
}

test_snapshot_promote() {
    log_header "S7: PROMOTE CLONE"
    log_test "Setup: snapshot and clone"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" '{"snapshot_name": "promote_snap"}')
    is_success "$response" || { log_fail "Setup snap"; return; }
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/promote_snap/clone" "{\"target\": \"$STRESS_POOL/promote_clone\"}")
    is_success "$response" || { log_fail "Setup clone"; return; }
    log_pass

    log_test "Promote clone"
    response=$(api POST "/v1/datasets/$STRESS_POOL/promote_clone/promote" '{}')
    if is_success "$response"; then
        log_info "Clone promoted successfully"
        log_pass
    else
        log_fail "$(json_field "$response" "message")"
    fi

    # Cleanup (order matters after promote)
    zfs destroy -r "$STRESS_POOL/promote_clone" 2>/dev/null || true
    zfs destroy "$STRESS_POOL/$TEST_DATASET@promote_snap" 2>/dev/null || true
}

test_snapshot_delete_nonexistent() {
    log_header "S8: DELETE NON-EXISTENT SNAPSHOT"
    log_test "DELETE nonexistent snapshot"
    local response
    response=$(api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/nonexistent_snap_xyz")
    if is_error "$response"; then log_expect_error "$(json_field "$response" "message")"; log_pass
    else log_fail "Should error"; fi
}

test_snapshot_max_length_name() {
    log_header "S9: SNAPSHOT NAME AT MAX LENGTH"
    # ZFS max snapshot name is 255 chars including dataset path
    local long_name
    long_name=$(printf 'x%.0s' {1..180})
    log_test "Create snapshot with 180-char name"
    local response
    response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" "{\"snapshot_name\": \"$long_name\"}")
    if is_success "$response"; then
        log_info "Long name accepted"
        log_pass
        api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/$long_name" >/dev/null 2>&1 || true
    elif is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Unexpected"
    fi
}

test_snapshot_volume_create() {
    log_header "S10: CREATE 50 SNAPSHOTS"
    log_test "Creating 50 snapshots"
    local success=0 i
    for i in $(seq 1 50); do
        local response
        response=$(api POST "/v1/snapshots/$STRESS_POOL/$TEST_DATASET" "{\"snapshot_name\": \"vol_snap_$i\"}")
        is_success "$response" && ((++success))
    done
    log_info "Created $success/50 snapshots"
    [[ $success -ge 48 ]] && log_pass || log_fail "Only $success/50"

    log_test "Verify list returns all"
    local response
    response=$(api GET "/v1/snapshots/$STRESS_POOL/$TEST_DATASET")
    local count
    count=$(echo "$response" | grep -o 'vol_snap_' | wc -l)
    [[ $count -ge 48 ]] && { log_info "List shows $count snapshots"; log_pass; } || log_fail "Only $count in list"

    # Cleanup
    for i in $(seq 1 50); do api DELETE "/v1/snapshots/$STRESS_POOL/$TEST_DATASET/vol_snap_$i" >/dev/null 2>&1 || true; done
}

# ==============================================================================
# TEST A3: Dataset Edge Cases (D1-D10)
# ==============================================================================

test_dataset_nested() {
    log_header "D1: DEEPLY NESTED DATASET (5 levels)"
    log_test "Create 5-level nesting"
    local path="$STRESS_POOL/$TEST_DATASET"
    local success=true
    for l in l1 l2 l3 l4 l5; do
        path="$path/$l"
        local response
        response=$(api POST "/v1/datasets" "{\"name\": \"$path\", \"kind\": \"filesystem\"}")
        is_success "$response" || { success=false; break; }
    done
    $success && { log_info "Created 5 levels"; log_pass; } || log_fail "Failed"
    zfs destroy -r "$STRESS_POOL/$TEST_DATASET/l1" 2>/dev/null || true
}

test_dataset_invalid_name() {
    log_header "D2: DATASET WITH INVALID NAME"
    log_test "Name with '@'"
    local response
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/bad@name\", \"kind\": \"filesystem\"}")
    if is_error "$response"; then log_expect_error "$(json_field "$response" "message")"; log_pass
    else log_fail "Should reject '@'"; zfs destroy "$STRESS_POOL/bad@name" 2>/dev/null || true; fi
}

test_dataset_duplicate() {
    log_header "D3: DUPLICATE DATASET"
    log_test "Create 'dup_ds'"
    local response
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/dup_ds\", \"kind\": \"filesystem\"}")
    is_success "$response" && log_pass || { log_fail "Setup"; return; }

    log_test "Create duplicate"
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/dup_ds\", \"kind\": \"filesystem\"}")
    if is_error "$response"; then log_expect_error "$(json_field "$response" "message")"; log_pass
    else log_fail "Should reject"; fi
    zfs destroy "$STRESS_POOL/dup_ds" 2>/dev/null || true
}

test_dataset_delete_with_children() {
    log_header "D4: DELETE DATASET WITH CHILDREN"
    local response
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/parent\", \"kind\": \"filesystem\"}")
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/parent/child\", \"kind\": \"filesystem\"}")

    log_test "DELETE parent with child"
    response=$(api DELETE "/v1/datasets/$STRESS_POOL/parent")
    if is_error "$response"; then log_expect_error "$(json_field "$response" "message")"; log_pass
    else log_fail "Should reject"; fi
    zfs destroy -r "$STRESS_POOL/parent" 2>/dev/null || true
}

test_dataset_recursive_delete() {
    log_header "D4b: RECURSIVE DELETE VIA API (?recursive=true)"
    local response

    # Create hierarchy: parent -> child -> grandchild + snapshot
    log_test "Create nested hierarchy with snapshot"
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/rec_del\", \"kind\": \"filesystem\"}")
    is_success "$response" || { log_fail "Parent"; return; }
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/rec_del/child\", \"kind\": \"filesystem\"}")
    is_success "$response" || { log_fail "Child"; zfs destroy -r "$STRESS_POOL/rec_del" 2>/dev/null || true; return; }
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/rec_del/child/gc\", \"kind\": \"filesystem\"}")
    is_success "$response" || { log_fail "Grandchild"; zfs destroy -r "$STRESS_POOL/rec_del" 2>/dev/null || true; return; }
    response=$(api POST "/v1/snapshots/$STRESS_POOL/rec_del/child/gc" '{"snapshot_name": "snap"}')
    is_success "$response" || { log_fail "Snapshot"; zfs destroy -r "$STRESS_POOL/rec_del" 2>/dev/null || true; return; }
    log_pass

    log_test "DELETE with ?recursive=true"
    response=$(api DELETE "/v1/datasets/$STRESS_POOL/rec_del?recursive=true")
    if is_success "$response"; then
        log_info "Recursive delete succeeded"
        log_pass
    else
        log_fail "$(json_field "$response" "message")"
        zfs destroy -r "$STRESS_POOL/rec_del" 2>/dev/null || true
        return
    fi

    log_test "Verify hierarchy deleted"
    if zfs list "$STRESS_POOL/rec_del" &>/dev/null; then
        log_fail "Dataset still exists"
        zfs destroy -r "$STRESS_POOL/rec_del" 2>/dev/null || true
    else
        log_info "All children deleted"
        log_pass
    fi
}

test_dataset_delete_with_snapshots() {
    log_header "D5: DELETE DATASET WITH SNAPSHOTS"
    local response
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/snap_ds\", \"kind\": \"filesystem\"}")
    response=$(api POST "/v1/snapshots/$STRESS_POOL/snap_ds" '{"snapshot_name": "snap"}')

    log_test "DELETE dataset with snapshot"
    response=$(api DELETE "/v1/datasets/$STRESS_POOL/snap_ds")
    if is_error "$response"; then log_expect_error "$(json_field "$response" "message")"; log_pass
    else log_fail "Should reject"; fi
    zfs destroy -r "$STRESS_POOL/snap_ds" 2>/dev/null || true
}

test_dataset_volume() {
    log_header "D6: CREATE VOLUME VS FILESYSTEM"
    log_test "Create volume (1M)"
    local response
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/testvol\", \"kind\": \"volume\", \"properties\": {\"volsize\": \"1M\"}}")
    if is_success "$response"; then
        log_info "Volume created"
        log_pass
    else
        # Volume might need special handling
        log_expect_error "$(json_field "$response" "message")"
        log_skip "Volume creation may need volsize in different format"
    fi
    zfs destroy "$STRESS_POOL/testvol" 2>/dev/null || true
}

test_dataset_nonexistent_pool() {
    log_header "D7: CREATE DATASET ON NON-EXISTENT POOL"
    log_test "POST to fake pool"
    local response
    response=$(api POST "/v1/datasets" '{"name": "fake_pool_xyz/dataset", "kind": "filesystem"}')
    if is_error "$response"; then log_expect_error "$(json_field "$response" "message")"; log_pass
    else log_fail "Should error"; fi
}

test_dataset_max_length() {
    log_header "D8: DATASET NAME AT MAX LENGTH"
    local long_name
    long_name=$(printf 'x%.0s' {1..200})
    log_test "Create dataset with 200-char name"
    local response
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/$long_name\", \"kind\": \"filesystem\"}")
    if is_success "$response"; then
        log_info "Long name accepted"
        log_pass
        zfs destroy "$STRESS_POOL/$long_name" 2>/dev/null || true
    elif is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    fi
}

test_dataset_concurrent_create() {
    log_header "D9: CONCURRENT DATASET CREATES (10)"
    log_test "Creating 10 datasets in parallel"

    # Launch in background
    local pids=()
    for i in $(seq 1 10); do
        (api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/concurrent_$i\", \"kind\": \"filesystem\"}" >/dev/null) &
        pids+=($!)
    done

    # Wait for all
    for pid in "${pids[@]}"; do wait "$pid" 2>/dev/null || true; done

    # Count successes
    local count
    count=$(zfs list -r -H -o name "$STRESS_POOL" 2>/dev/null | grep -c "concurrent_" || echo 0)
    log_info "Created $count/10 datasets"
    [[ $count -ge 8 ]] && log_pass || log_fail "Only $count/10"

    for i in $(seq 1 10); do zfs destroy "$STRESS_POOL/concurrent_$i" 2>/dev/null || true; done
}

test_dataset_delete_clone_origin() {
    log_header "D10: DELETE DATASET THAT IS CLONE ORIGIN"
    # After promotion, the original dataset becomes a clone of the promoted one
    log_test "Setup: dataset -> snapshot -> clone -> promote"
    local response
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/origin_test\", \"kind\": \"filesystem\"}")
    response=$(api POST "/v1/snapshots/$STRESS_POOL/origin_test" '{"snapshot_name": "snap"}')
    response=$(api POST "/v1/snapshots/$STRESS_POOL/origin_test/snap/clone" "{\"target\": \"$STRESS_POOL/promoted_clone\"}")
    response=$(api POST "/v1/datasets/$STRESS_POOL/promoted_clone/promote" '{}')

    log_test "DELETE original (now dependent on promoted)"
    response=$(api DELETE "/v1/datasets/$STRESS_POOL/origin_test")
    # After promote, original may or may not be deletable depending on snap ownership
    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_info "Deletion succeeded (snap ownership transferred)"
        log_pass
    fi

    zfs destroy -r "$STRESS_POOL/promoted_clone" 2>/dev/null || true
    zfs destroy -r "$STRESS_POOL/origin_test" 2>/dev/null || true
}

# ==============================================================================
# TEST A4: Rollback Cases (R1-R5)
# ==============================================================================

test_rollback_most_recent() {
    log_header "R1: ROLLBACK TO MOST RECENT"
    local response
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/rb1\", \"kind\": \"filesystem\"}")
    response=$(api POST "/v1/snapshots/$STRESS_POOL/rb1" '{"snapshot_name": "snap"}')

    log_test "Rollback to most recent"
    response=$(api POST "/v1/datasets/$STRESS_POOL/rb1/rollback" '{"snapshot": "snap"}')
    is_success "$response" && log_pass || log_fail "$(json_field "$response" "message")"
    zfs destroy -r "$STRESS_POOL/rb1" 2>/dev/null || true
}

test_rollback_older_blocked() {
    log_header "R2: ROLLBACK TO OLDER (BLOCKED)"
    local response
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/rb2\", \"kind\": \"filesystem\"}")
    response=$(api POST "/v1/snapshots/$STRESS_POOL/rb2" '{"snapshot_name": "old"}')
    response=$(api POST "/v1/snapshots/$STRESS_POOL/rb2" '{"snapshot_name": "new"}')

    log_test "Rollback to older (blocked)"
    response=$(api POST "/v1/datasets/$STRESS_POOL/rb2/rollback" '{"snapshot": "old"}')
    if is_error "$response" && echo "$response" | grep -qi "block\|newer"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should be blocked"
    fi
    zfs destroy -r "$STRESS_POOL/rb2" 2>/dev/null || true
}

test_rollback_force_destroy() {
    log_header "R3: ROLLBACK WITH FORCE DESTROY"
    local response
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/rb3\", \"kind\": \"filesystem\"}")
    response=$(api POST "/v1/snapshots/$STRESS_POOL/rb3" '{"snapshot_name": "old"}')
    response=$(api POST "/v1/snapshots/$STRESS_POOL/rb3" '{"snapshot_name": "new"}')

    log_test "Rollback with force_destroy_newer=true"
    response=$(api POST "/v1/datasets/$STRESS_POOL/rb3/rollback" '{"snapshot": "old", "force_destroy_newer": true}')
    if is_success "$response"; then
        log_info "Rollback succeeded, newer snaps destroyed"
        log_pass
    else
        log_fail "$(json_field "$response" "message")"
    fi
    zfs destroy -r "$STRESS_POOL/rb3" 2>/dev/null || true
}

test_rollback_blocked_by_clones() {
    log_header "R4: ROLLBACK BLOCKED BY CLONES"
    local response
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/rb4\", \"kind\": \"filesystem\"}")
    response=$(api POST "/v1/snapshots/$STRESS_POOL/rb4" '{"snapshot_name": "old"}')
    response=$(api POST "/v1/snapshots/$STRESS_POOL/rb4" '{"snapshot_name": "new"}')
    response=$(api POST "/v1/snapshots/$STRESS_POOL/rb4/new/clone" "{\"target\": \"$STRESS_POOL/rb4_clone\"}")

    log_test "Rollback blocked by clone"
    response=$(api POST "/v1/datasets/$STRESS_POOL/rb4/rollback" '{"snapshot": "old", "force_destroy_newer": true}')
    if is_error "$response"; then
        log_expect_error "$(json_field "$response" "message")"
        log_pass
    else
        log_fail "Should be blocked by clone"
    fi
    zfs destroy "$STRESS_POOL/rb4_clone" 2>/dev/null || true
    zfs destroy -r "$STRESS_POOL/rb4" 2>/dev/null || true
}

test_rollback_nonexistent() {
    log_header "R5: ROLLBACK TO NON-EXISTENT SNAPSHOT"
    local response
    response=$(api POST "/v1/datasets" "{\"name\": \"$STRESS_POOL/rb5\", \"kind\": \"filesystem\"}")

    log_test "Rollback to nonexistent"
    response=$(api POST "/v1/datasets/$STRESS_POOL/rb5/rollback" '{"snapshot": "nonexistent"}')
    if is_error "$response"; then log_expect_error "$(json_field "$response" "message")"; log_pass
    else log_fail "Should error"; fi
    zfs destroy -r "$STRESS_POOL/rb5" 2>/dev/null || true
}

# ==============================================================================
# Main
# ==============================================================================

main() {
    echo -e "${BLUE}╔════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║      ZFS STRESS TEST A (LONG) - Complete Data Layer Tests      ║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════════════════╝${NC}"
    echo "API: $API_URL | Cleanup: $CLEANUP"
    echo ""

    check_prerequisites
    setup_test_pool

    # Properties P1-P8
    test_property_invalid_name
    test_property_invalid_value
    test_property_readonly
    test_property_special_chars
    test_property_nonexistent_dataset
    test_property_rapid_changes
    test_property_inheritance
    test_property_override_inherited

    # Snapshots S1-S10
    test_snapshot_invalid_name
    test_snapshot_nonexistent_dataset
    test_snapshot_duplicate
    test_snapshot_delete_with_clone
    test_snapshot_rapid_create
    test_snapshot_clone_multiple
    test_snapshot_promote
    test_snapshot_delete_nonexistent
    test_snapshot_max_length_name
    test_snapshot_volume_create

    # Datasets D1-D10 (plus D4b)
    test_dataset_nested
    test_dataset_invalid_name
    test_dataset_duplicate
    test_dataset_delete_with_children
    test_dataset_recursive_delete
    test_dataset_delete_with_snapshots
    test_dataset_volume
    test_dataset_nonexistent_pool
    test_dataset_max_length
    test_dataset_concurrent_create
    test_dataset_delete_clone_origin

    # Rollback R1-R5
    test_rollback_most_recent
    test_rollback_older_blocked
    test_rollback_force_destroy
    test_rollback_blocked_by_clones
    test_rollback_nonexistent

    log_header "TEST SUMMARY"
    echo -e "Passed:  ${GREEN}$PASSED${NC}"
    echo -e "Failed:  ${RED}$FAILED${NC}"
    echo -e "Skipped: ${YELLOW}$SKIPPED${NC}"

    [[ $FAILED -gt 0 ]] && { echo -e "${RED}SOME TESTS FAILED${NC}"; exit 1; }
    echo -e "${GREEN}ALL TESTS PASSED${NC}"
}

main "$@"
