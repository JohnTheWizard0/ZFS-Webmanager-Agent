#!/bin/bash
# ==============================================================================
# ZFS Web Manager API - Test Runner
# ==============================================================================
# Runs all tests in dependency order:
#   1. Compilation gate (must pass for anything else to work)
#   2. Unit tests (in-file #[cfg(test)] modules)
#   3. Integration tests (tests/ directory)
#
# Usage:
#   ./run_tests.sh          # Run all tests
#   ./run_tests.sh --unit   # Unit tests only
#   ./run_tests.sh --int    # Integration tests only
#   ./run_tests.sh --quick  # Skip ignored tests (faster)
#
# Exit codes:
#   0 - All tests passed
#   1 - Compilation failed
#   2 - Unit tests failed
#   3 - Integration tests failed
# ==============================================================================

set -e  # Exit on first error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Parse arguments
RUN_UNIT=true
RUN_INT=true
INCLUDE_IGNORED=true

for arg in "$@"; do
    case $arg in
        --unit)
            RUN_INT=false
            ;;
        --int)
            RUN_UNIT=false
            ;;
        --quick)
            INCLUDE_IGNORED=false
            ;;
    esac
done

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║         ZFS Web Manager API - Test Suite                     ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# ==============================================================================
# STAGE 1: Compilation Gate
# ==============================================================================
echo -e "${YELLOW}━━━ STAGE 1: Compilation Check ━━━${NC}"
echo "Running: cargo check"
echo ""

if cargo check 2>&1; then
    echo -e "${GREEN}✓ Compilation successful${NC}"
else
    echo -e "${RED}✗ Compilation FAILED${NC}"
    echo ""
    echo "Known blockers:"
    echo "  - src/zfs_management.rs:135 - CreateDataset not imported"
    echo "  - src/zfs_management.rs:144 - undefined request_builder"
    echo ""
    echo "Fix these errors before tests can run."
    exit 1
fi
echo ""

# ==============================================================================
# STAGE 2: Unit Tests (in-file)
# ==============================================================================
if [ "$RUN_UNIT" = true ]; then
    echo -e "${YELLOW}━━━ STAGE 2: Unit Tests ━━━${NC}"
    echo "Testing internal logic in source files"
    echo ""
    echo "Modules tested:"
    echo "  MI-001 (Auth)        → src/auth.rs"
    echo "  MI-002 (Models)      → src/models.rs"
    echo "  MI-002 (Utils)       → src/utils.rs"
    echo "  MC-001 (ZFS Engine)  → src/zfs_management.rs"
    echo ""

    # Build test command
    UNIT_CMD="cargo test --lib"
    if [ "$INCLUDE_IGNORED" = true ]; then
        UNIT_CMD="$UNIT_CMD -- --include-ignored"
    fi

    echo "Running: $UNIT_CMD"
    echo ""

    if $UNIT_CMD 2>&1; then
        echo -e "${GREEN}✓ Unit tests passed${NC}"
    else
        echo -e "${RED}✗ Unit tests FAILED${NC}"
        exit 2
    fi
    echo ""
fi

# ==============================================================================
# STAGE 3: Integration Tests (tests/ directory)
# ==============================================================================
if [ "$RUN_INT" = true ]; then
    echo -e "${YELLOW}━━━ STAGE 3: Integration Tests ━━━${NC}"
    echo "Testing HTTP API endpoints"
    echo ""
    echo "Test files:"
    echo "  MF-004 (Health)      → tests/api_health.rs"
    echo "  MI-001 (Auth)        → tests/api_auth.rs"
    echo "  MF-001 (Pools)       → tests/api_pools.rs"
    echo "  MF-002 (Datasets)    → tests/api_datasets.rs"
    echo "  MF-003 (Snapshots)   → tests/api_snapshots.rs"
    echo ""

    # Build test command
    INT_CMD="cargo test --tests"
    if [ "$INCLUDE_IGNORED" = true ]; then
        INT_CMD="$INT_CMD -- --include-ignored"
    fi

    echo "Running: $INT_CMD"
    echo ""

    if $INT_CMD 2>&1; then
        echo -e "${GREEN}✓ Integration tests passed${NC}"
    else
        echo -e "${RED}✗ Integration tests FAILED${NC}"
        exit 3
    fi
    echo ""
fi

# ==============================================================================
# Summary
# ==============================================================================
echo -e "${BLUE}━━━ SUMMARY ━━━${NC}"
echo -e "${GREEN}✓ All tests passed!${NC}"
echo ""
echo "Test coverage by module:"
echo "  ┌─────────────────────────────────────────────────────────┐"
echo "  │ Module                  │ Unit Tests │ Integration     │"
echo "  ├─────────────────────────┼────────────┼─────────────────┤"
echo "  │ MI-001 Auth             │ ✓          │ ✓               │"
echo "  │ MI-002 API Framework    │ ✓          │ (via endpoints) │"
echo "  │ MC-001 ZFS Engine       │ ✓          │ (via endpoints) │"
echo "  │ MF-001 Pool Management  │ —          │ ✓               │"
echo "  │ MF-002 Dataset Ops      │ —          │ ✓               │"
echo "  │ MF-003 Snapshot Handling│ —          │ ✓               │"
echo "  │ MF-004 Health Monitoring│ —          │ ✓               │"
echo "  └─────────────────────────┴────────────┴─────────────────┘"
