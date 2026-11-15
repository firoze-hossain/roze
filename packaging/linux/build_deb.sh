#!/usr/bin/env bash
# ==============================================================================
# Build Debian/Ubuntu .deb package for Roze v2.0.0 'Prometheus'
# ==============================================================================

set -euo pipefail

VERSION="2.0.0"
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
for bin in roze bsr roze-build roze-pkg roze-lsp roze-bindgen roze-svd rozedoc; do
    if [ -f "${ROOT_DIR}/target/release/${bin}" ]; then
        cp -f "${ROOT_DIR}/target/release/${bin}" "${BUILD_DIR}/usr/bin/"
    elif [ -f "${ROOT_DIR}/target/debug/${bin}" ]; then
        cp -f "${ROOT_DIR}/target/debug/${bin}" "${BUILD_DIR}/usr/bin/"
    fi
    [ -f "${BUILD_DIR}/usr/bin/${bin}" ] && chmod 755 "${BUILD_DIR}/usr/bin/${bin}"
done

# Copy standard library
if [ -d "${ROOT_DIR}/stdlib/src" ]; then
    cp -rf "${ROOT_DIR}/stdlib/src/"* "${BUILD_DIR}/usr/lib/roze/stdlib/"
elif [ -d "${ROOT_DIR}/stdlib" ]; then
    cp -rf "${ROOT_DIR}/stdlib/"* "${BUILD_DIR}/usr/lib/roze/stdlib/"
fi

# Copy documentation & license
[ -f "${ROOT_DIR}/README.md" ] && cp "${ROOT_DIR}/README.md" "${BUILD_DIR}/usr/share/doc/roze/"
[ -f "${ROOT_DIR}/LICENSE" ] && cp "${ROOT_DIR}/LICENSE" "${BUILD_DIR}/usr/share/doc/roze/copyright"
[ -f "${ROOT_DIR}/docs/GRAMMAR.md" ] && cp "${ROOT_DIR}/docs/GRAMMAR.md" "${BUILD_DIR}/usr/share/doc/roze/"
[ -f "${ROOT_DIR}/docs/STDLIB.md" ] && cp "${ROOT_DIR}/docs/STDLIB.md" "${BUILD_DIR}/usr/share/doc/roze/"

# Build .deb archive
mkdir -p "${ROOT_DIR}/dist"
dpkg-deb --build "${BUILD_DIR}" "${ROOT_DIR}/dist/roze_${VERSION}_${ARCH}.deb"

echo "✅ Created: ${ROOT_DIR}/dist/roze_${VERSION}_${ARCH}.deb"
