#!/usr/bin/env bash
# ==============================================================================
# 🌹 Official Roze Programming Language Installer (v2.0.0 'Prometheus')
# Usage: curl -fsSL https://roze-lang.org/install.sh | bash
# ==============================================================================

set -euo pipefail

ROZE_VERSION="2.0.0"
INSTALL_DIR="${HOME}/.roze"
BIN_DIR="${INSTALL_DIR}/bin"
STDLIB_DIR="${INSTALL_DIR}/stdlib"

echo "=============================================================================="
echo "🌹 Installing Roze Programming Language v${ROZE_VERSION} 'Prometheus'"
echo "=============================================================================="

# Detect OS and architecture
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "${ARCH}" in
    x86_64|amd64)
        ROZE_ARCH="x86_64"
        ;;
    aarch64|arm64)
        ROZE_ARCH="aarch64"
        ;;
    *)
        echo "❌ Unsupported architecture: ${ARCH}"
        exit 1
        ;;
esac

echo "Detected Platform: ${OS} (${ROZE_ARCH})"

# Create installation directories
mkdir -p "${BIN_DIR}"
mkdir -p "${STDLIB_DIR}"

# Locate repo or pre-built binaries
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

BINARIES=(roze bsr roze-pkg roze-build roze-lsp roze-bindgen roze-svd rozedoc)

if [ -f "${SCRIPT_DIR}/target/release/roze" ]; then
    echo "📦 Installing from local release build..."
    for bin in "${BINARIES[@]}"; do
        [ -f "${SCRIPT_DIR}/target/release/${bin}" ] && cp -f "${SCRIPT_DIR}/target/release/${bin}" "${BIN_DIR}/"
    done
elif [ -f "${SCRIPT_DIR}/target/debug/roze" ]; then
    echo "📦 Installing from local debug build..."
    for bin in "${BINARIES[@]}"; do
        [ -f "${SCRIPT_DIR}/target/debug/${bin}" ] && cp -f "${SCRIPT_DIR}/target/debug/${bin}" "${BIN_DIR}/"
    done
else
    echo "⚡ Compiling Roze toolchain (v${ROZE_VERSION})..."
    cargo build --release --workspace
    for bin in "${BINARIES[@]}"; do
        [ -f "${SCRIPT_DIR}/target/release/${bin}" ] && cp -f "${SCRIPT_DIR}/target/release/${bin}" "${BIN_DIR}/"
    done
fi

# Copy standard library
if [ -d "${SCRIPT_DIR}/stdlib/src" ]; then
    cp -rf "${SCRIPT_DIR}/stdlib/src/"* "${STDLIB_DIR}/"
elif [ -d "${SCRIPT_DIR}/stdlib" ]; then
    cp -rf "${SCRIPT_DIR}/stdlib/"* "${STDLIB_DIR}/"
fi

# Set executable permissions
chmod +x "${BIN_DIR}/"*

# Configure PATH in shell profiles
add_to_path() {
    local profile="$1"
    if [ -f "${profile}" ]; then
        if ! grep -q 'export PATH="${HOME}/.roze/bin:$PATH"' "${profile}"; then
            echo "" >> "${profile}"
            echo '# Roze Programming Language' >> "${profile}"
            echo 'export PATH="${HOME}/.roze/bin:$PATH"' >> "${profile}"
            echo 'export ROZE_STDLIB_PATH="${HOME}/.roze/stdlib"' >> "${profile}"
            echo "✅ Configured PATH in ${profile}"
        fi
    fi
}

add_to_path "${HOME}/.bashrc"
add_to_path "${HOME}/.zshrc"
add_to_path "${HOME}/.profile"

echo ""
echo "=============================================================================="
echo "🎉 Roze v${ROZE_VERSION} 'Prometheus' installed successfully to ${INSTALL_DIR}!"
echo "=============================================================================="
echo ""
echo "To activate Roze in your current shell session, run:"
echo "    source ~/.bashrc  (or source ~/.zshrc)"
echo ""
echo "Verify installation:"
echo "    roze --version"
echo "    bsr --version"
echo "    roze-lsp --version"
echo ""
