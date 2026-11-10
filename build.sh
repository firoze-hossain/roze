#!/usr/bin/env bash
# ==============================================================================
# 🌹 Roze Autonomous Build Script (M6-T05)
# ==============================================================================
# Pure Roze bootstrap and build driver - zero Rust/Cargo required
# ==============================================================================
set -euo pipefail

CMD="${1:-all}"

case "$CMD" in
    all|build)
        echo "🌹 Building Roze Autonomous Compiler..."
        make all
        ;;
    bootstrap)
        echo "🌹 Running Pure Roze Autonomous Bootstrap Pipeline..."
        make bootstrap
        ;;
    fixpoint)
        echo "🌹 Running 3-Stage Bootstrap Fixpoint Verification..."
        make fixpoint
        ;;
    test)
        echo "🌹 Running Pure Roze Test Suites..."
        make test
        ;;
    seed)
        echo "🌹 Regenerating bootstrap seed..."
        make seed
        ;;
    clean)
        echo "🌹 Cleaning build artifacts..."
        make clean
        ;;
    help|--help|-h)
        make help
        ;;
    *)
        echo "Unknown command: $CMD"
        echo "Run './build.sh help' for usage."
        exit 1
        ;;
esac
