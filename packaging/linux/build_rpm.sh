#!/usr/bin/env bash
# ==============================================================================
# Build RPM package for Roze v1.0.0
# ==============================================================================

set -euo pipefail

VERSION="1.0.0"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"
RPMBUILD_DIR="${DIST_DIR}/rpmbuild"

echo "📦 Preparing RPM package for Roze v${VERSION}..."

mkdir -p "${RPMBUILD_DIR}"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
cp "${ROOT_DIR}/packaging/linux/roze.spec" "${RPMBUILD_DIR}/SPECS/"

# Create source tarball
TARBALL="roze-${VERSION}.tar.gz"
tar --exclude="./target" --exclude="./.git" --exclude="./dist" \
    --transform "s,^\.,roze-${VERSION}," \
    -czf "${RPMBUILD_DIR}/SOURCES/${TARBALL}" -C "${ROOT_DIR}" .

if command -v rpmbuild >/dev/null 2>&1; then
    rpmbuild --define "_topdir ${RPMBUILD_DIR}" -bb "${RPMBUILD_DIR}/SPECS/roze.spec"
    find "${RPMBUILD_DIR}/RPMS" -type f -name "*.rpm" -exec cp {} "${DIST_DIR}/" \;
    echo "✅ RPM packages generated in ${DIST_DIR}/"
else
    echo "⚠️ rpmbuild not found on system. Spec and source tarball created at ${RPMBUILD_DIR}/SOURCES/${TARBALL}"
fi
