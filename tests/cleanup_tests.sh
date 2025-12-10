#!/bin/bash
# ==============================================================================
# ZFS Test Cleanup Script
# ==============================================================================
# Shared cleanup script for all ZFS stress tests.
# Destroys all test pools and removes temporary files.
#
# USAGE:
#   ./tests/cleanup_tests.sh              # Interactive cleanup
#   ./tests/cleanup_tests.sh --force      # Non-interactive cleanup
#   source ./tests/cleanup_tests.sh       # Source for use in other scripts
#
# POOLS CLEANED:
#   - parcour_pool_a, parcour_pool_b      (zfs_parcour.sh)
#   - stress_test_pool                     (zfs_stress_a_*.sh)
#   - stress_b_pool, stress_b_pool_alt     (zfs_stress_b_*.sh)
#   - stress_b_renamed                     (zfs_stress_b_*.sh rename tests)
#
# FILES CLEANED:
#   - /tmp/stress_*.zfs
#   - /tmp/parcour_*.zfs
#   - /tmp/replication_*.zfs
# ==============================================================================

# All known test pool names
TEST_POOLS=(
    "parcour_pool_a"
    "parcour_pool_b"
    "stress_test_pool"
    "stress_b_pool"
    "stress_b_pool_alt"
    "stress_b_renamed"
)

# Temp file patterns
TEMP_PATTERNS=(
    "/tmp/stress_*.zfs"
    "/tmp/parcour_*.zfs"
    "/tmp/replication_*.zfs"
)

# Colors (only if stdout is a terminal)
if [[ -t 1 ]]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    NC='\033[0m'
else
    RED='' GREEN='' YELLOW='' BLUE='' NC=''
fi

# ==============================================================================
# Functions
# ==============================================================================

cleanup_log() {
    echo -e "${BLUE}[CLEANUP]${NC} $1"
}

cleanup_ok() {
    echo -e "${GREEN}  ✓${NC} $1"
}

cleanup_warn() {
    echo -e "${YELLOW}  !${NC} $1"
}

cleanup_err() {
    echo -e "${RED}  ✗${NC} $1"
}

# Import any exported test pools so they can be destroyed
import_exported_pools() {
    local pool
    for pool in "${TEST_POOLS[@]}"; do
        if zpool import 2>/dev/null | grep -q "pool: $pool"; then
            cleanup_log "Importing exported pool: $pool"
            if zpool import "$pool" 2>/dev/null; then
                cleanup_ok "Imported $pool"
            else
                cleanup_warn "Could not import $pool (may need -f)"
                zpool import -f "$pool" 2>/dev/null || true
            fi
        fi
    done
}

# Destroy all test pools
destroy_test_pools() {
    local pool destroyed=0 skipped=0
    for pool in "${TEST_POOLS[@]}"; do
        if zpool list "$pool" &>/dev/null; then
            cleanup_log "Destroying pool: $pool"
            if zpool destroy -f "$pool" 2>/dev/null; then
                cleanup_ok "Destroyed $pool"
                destroyed=$((destroyed + 1))
            else
                cleanup_err "Failed to destroy $pool"
            fi
        else
            skipped=$((skipped + 1))
        fi
    done

    if [[ $destroyed -eq 0 && $skipped -eq ${#TEST_POOLS[@]} ]]; then
        cleanup_log "No test pools found"
    else
        cleanup_log "Destroyed $destroyed pool(s), $skipped not present"
    fi
}

# Remove temporary files
cleanup_temp_files() {
    local pattern count=0
    for pattern in "${TEMP_PATTERNS[@]}"; do
        # shellcheck disable=SC2086
        for file in $pattern; do
            if [[ -f "$file" ]]; then
                if rm -f "$file"; then
                    cleanup_ok "Removed $file"
                    count=$((count + 1))
                fi
            fi
        done
    done

    if [[ $count -eq 0 ]]; then
        cleanup_log "No temporary files found"
    else
        cleanup_log "Removed $count file(s)"
    fi
}

# Main cleanup function (can be called from other scripts)
run_test_cleanup() {
    local quiet="${1:-false}"

    [[ "$quiet" != "true" ]] && echo -e "${BLUE}━━━ ZFS TEST CLEANUP ━━━${NC}"

    # First import any exported pools
    import_exported_pools

    # Then destroy all test pools
    destroy_test_pools

    # Finally remove temp files
    cleanup_temp_files

    [[ "$quiet" != "true" ]] && echo -e "${GREEN}Cleanup complete${NC}"
}

# Show what would be cleaned (dry run)
show_cleanup_status() {
    echo -e "${BLUE}━━━ ZFS TEST CLEANUP STATUS ━━━${NC}"
    echo ""

    echo "Active test pools:"
    local found=false
    for pool in "${TEST_POOLS[@]}"; do
        if zpool list "$pool" &>/dev/null; then
            echo "  - $pool (active)"
            found=true
        fi
    done
    [[ "$found" == "false" ]] && echo "  (none)"

    echo ""
    echo "Exported test pools:"
    found=false
    for pool in "${TEST_POOLS[@]}"; do
        if zpool import 2>/dev/null | grep -q "pool: $pool"; then
            echo "  - $pool (exported)"
            found=true
        fi
    done
    [[ "$found" == "false" ]] && echo "  (none)"

    echo ""
    echo "Temporary files:"
    found=false
    for pattern in "${TEMP_PATTERNS[@]}"; do
        # shellcheck disable=SC2086
        for file in $pattern; do
            if [[ -f "$file" ]]; then
                echo "  - $file"
                found=true
            fi
        done
    done
    [[ "$found" == "false" ]] && echo "  (none)"
}

# ==============================================================================
# Main (only runs if script is executed directly, not sourced)
# ==============================================================================

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    case "${1:-}" in
        --force|-f)
            run_test_cleanup
            ;;
        --status|-s)
            show_cleanup_status
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --force, -f    Run cleanup without prompting"
            echo "  --status, -s   Show what would be cleaned"
            echo "  --help, -h     Show this help"
            echo ""
            echo "Without options, runs interactive cleanup."
            ;;
        *)
            show_cleanup_status
            echo ""
            read -rp "Run cleanup? [y/N] " answer
            if [[ "$answer" =~ ^[Yy] ]]; then
                run_test_cleanup
            else
                echo "Cancelled"
            fi
            ;;
    esac
fi
