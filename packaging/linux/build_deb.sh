#!/usr/bin/env bash
# ==============================================================================
# Build Debian/Ubuntu .deb package for Roze v1.0.0
# ==============================================================================

set -euo pipefail

VERSION="1.0.0"
ARCH="amd64"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUILD_DIR="${ROOT_DIR}/dist/deb/roze_${VERSION}_${ARCH}"

echo "📦 Preparing Debian package for Roze v${VERSION} (${ARCH})..."

rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}/DEBIAN"
mkdir -p "${BUILD_DIR}/usr/bin"
mkdir -p "${BUILD_DIR}/usr/lib/roze/stdlib"
mkdir -p "${BUILD_DIR}/usr/share/doc/roze"
mkdir -p "${BUILD_DIR}/usr/share/man/man1"

# Copy debian control file
cp "${ROOT_DIR}/packaging/linux/debian/control" "${BUILD_DIR}/DEBIAN/"

# Copy binaries
for bin in roze roze-build roze-pkg roze-lsp; do
    if [ -f "${ROOT_DIR}/target/release/${bin}" ]; then
        cp -f "${ROOT_DIR}/target/release/${bin}" "${BUILD_DIR}/usr/bin/"
    elif [ -f "${ROOT_DIR}/target/debug/${bin}" ]; then
        cp -f "${ROOT_DIR}/target/debug/${bin}" "${BUILD_DIR}/usr/bin/"
    fi
    chmod 755 "${BUILD_DIR}/usr/bin/${bin}"
done

# Copy standard library
cp -rf "${ROOT_DIR}/stdlib/"* "${BUILD_DIR}/usr/lib/roze/stdlib/"

# Copy documentation & license
cp "${ROOT_DIR}/README.md" "${BUILD_DIR}/usr/share/doc/roze/"
cp "${ROOT_DIR}/LICENSE" "${BUILD_DIR}/usr/share/doc/roze/copyright"

# Build .deb archive
mkdir -p "${ROOT_DIR}/dist"
dpkg-deb --build "${BUILD_DIR}" "${ROOT_DIR}/dist/roze_${VERSION}_${ARCH}.deb"

echo "✅ Created: ${ROOT_DIR}/dist/roze_${VERSION}_${ARCH}.deb"
