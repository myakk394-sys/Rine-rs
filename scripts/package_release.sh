#!/usr/bin/env bash
set -euo pipefail

# Packages a standalone Rine release archive for Linux x86_64
# Output: dist/rine-linux-x86_64.tar.gz

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"
STAGE_DIR="${DIST_DIR}/rine-linux-x86_64"
ARCHIVE="${DIST_DIR}/rine-linux-x86_64.tar.gz"

echo "=== Building Rine Release Package ==="
cd "${ROOT_DIR}"

cargo build --release --bin rine

rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/dxvk"

install -m 755 "${ROOT_DIR}/target/release/rine" "${STAGE_DIR}/rine"
install -m 755 "${ROOT_DIR}/install.sh" "${STAGE_DIR}/install.sh"
cp -f "${ROOT_DIR}/README.md" "${STAGE_DIR}/README.md"
cp -f "${ROOT_DIR}/LICENSE" "${STAGE_DIR}/LICENSE"

if [[ -d "${ROOT_DIR}/assets/dxvk" ]]; then
    cp -f "${ROOT_DIR}/assets/dxvk"/*.dll "${STAGE_DIR}/dxvk/" 2>/dev/null || true
    if [[ -f "${ROOT_DIR}/assets/dxvk/dxvk.conf" ]]; then
        cp -f "${ROOT_DIR}/assets/dxvk/dxvk.conf" "${STAGE_DIR}/dxvk/dxvk.conf"
    fi
fi

tar -C "${DIST_DIR}" -czf "${ARCHIVE}" "rine-linux-x86_64"
rm -rf "${STAGE_DIR}"

echo "Release archive created: ${ARCHIVE}"
ls -lh "${ARCHIVE}"
