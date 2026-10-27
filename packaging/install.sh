#!/usr/bin/env bash
# ==============================================================================
# 🌹 Official Roze Programming Language Installer (v1.0.0)
# Usage: curl -fsSL https://roze-lang.org/install.sh | bash
# ==============================================================================

set -euo pipefail

ROZE_VERSION="1.0.0"
INSTALL_DIR="${HOME}/.roze"
BIN_DIR="${INSTALL_DIR}/bin"
STDLIB_DIR="${INSTALL_DIR}/stdlib"

echo "=============================================================================="
echo "🌹 Installing Roze Programming Language v${ROZE_VERSION}"
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

if [ -f "${SCRIPT_DIR}/target/release/roze" ]; then
    echo "📦 Installing from local release build..."
    cp -f "${SCRIPT_DIR}/target/release/roze" "${BIN_DIR}/"
    [ -f "${SCRIPT_DIR}/target/release/roze-pkg" ] && cp -f "${SCRIPT_DIR}/target/release/roze-pkg" "${BIN_DIR}/"
    [ -f "${SCRIPT_DIR}/target/release/roze-build" ] && cp -f "${SCRIPT_DIR}/target/release/roze-build" "${BIN_DIR}/"
    [ -f "${SCRIPT_DIR}/target/release/roze-lsp" ] && cp -f "${SCRIPT_DIR}/target/release/roze-lsp" "${BIN_DIR}/"
elif [ -f "${SCRIPT_DIR}/target/debug/roze" ]; then
    echo "📦 Installing from local debug build..."
    cp -f "${SCRIPT_DIR}/target/debug/roze" "${BIN_DIR}/"
    [ -f "${SCRIPT_DIR}/target/debug/roze-pkg" ] && cp -f "${SCRIPT_DIR}/target/debug/roze-pkg" "${BIN_DIR}/"
    [ -f "${SCRIPT_DIR}/target/debug/roze-build" ] && cp -f "${SCRIPT_DIR}/target/debug/roze-build" "${BIN_DIR}/"
    [ -f "${SCRIPT_DIR}/target/debug/roze-lsp" ] && cp -f "${SCRIPT_DIR}/target/debug/roze-lsp" "${BIN_DIR}/"
else
    echo "⚡ Compiling Roze toolchain (v${ROZE_VERSION})..."
    cargo build --release --workspace
    cp -f "${SCRIPT_DIR}/target/release/roze"* "${BIN_DIR}/"
fi

# Copy standard library
if [ -d "${SCRIPT_DIR}/stdlib" ]; then
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
echo "🎉 Roze v${ROZE_VERSION} installed successfully to ${INSTALL_DIR}!"
echo "=============================================================================="
echo ""
echo "To activate Roze in your current shell session, run:"
echo "    source ~/.bashrc  (or source ~/.zshrc)"
echo ""
echo "Verify installation:"
echo "    roze --version"
echo "    roze-pkg --version"
echo ""
