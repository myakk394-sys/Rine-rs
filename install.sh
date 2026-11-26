#!/usr/bin/env bash
set -euo pipefail

# Rine Installer for Linux x86_64
# Installs the `rine` binary, standalone DXVK + dxvk.conf, shader cache directory,
# and desktop entry into ~/.local/.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${HOME}/.local/bin"
DATA_DIR="${HOME}/.local/share/rine"
DXVK_DIR="${DATA_DIR}/dxvk"
CACHE_DIR="${DATA_DIR}/cache/dxvk"
APPS_DIR="${HOME}/.local/share/applications"

echo "=== Rine Win64 Compatibility Runtime Installer ==="

if [[ "$(uname -m)" != "x86_64" ]]; then
    echo "Error: Rine requires an x86_64 Linux system (detected: $(uname -m))."
    exit 1
fi

# Locate binary (either next to install.sh in release archive, or in target/release/rine)
RINE_SRC=""
if [[ -f "${SCRIPT_DIR}/rine" ]]; then
    RINE_SRC="${SCRIPT_DIR}/rine"
elif [[ -f "${SCRIPT_DIR}/target/release/rine" ]]; then
    RINE_SRC="${SCRIPT_DIR}/target/release/rine"
else
    echo "Release binary not found. Building with cargo..."
    if ! command -v cargo >/dev/null 2>&1; then
        echo "Error: cargo is not installed and prebuilt 'rine' binary was not found."
        exit 1
    fi
    (cd "${SCRIPT_DIR}" && cargo build --release --bin rine)
    RINE_SRC="${SCRIPT_DIR}/target/release/rine"
fi

# Check runtime libraries
echo "[1/4] Checking system runtime libraries..."
for lib in libX11.so.6 libSDL2-2.0.so.0 libvulkan.so.1; do
    if ! ldconfig -p 2>/dev/null | grep -q "$lib" && ! find /usr/lib /usr/lib64 -name "${lib}*" 2>/dev/null | grep -q .; then
        echo "  [WARN] ${lib} was not detected. Make sure X11, SDL2, and Vulkan loader packages are installed."
    fi
done

# Install binary
echo "[2/4] Installing binary to ${BIN_DIR}/rine..."
mkdir -p "${BIN_DIR}"
install -m 755 "${RINE_SRC}" "${BIN_DIR}/rine"

# Install standalone DXVK & dxvk.conf
echo "[3/4] Installing standalone DXVK & dxvk.conf to ${DXVK_DIR}..."
mkdir -p "${DXVK_DIR}" "${CACHE_DIR}"
DXVK_SRC=""
if [[ -d "${SCRIPT_DIR}/dxvk" ]]; then
    DXVK_SRC="${SCRIPT_DIR}/dxvk"
elif [[ -d "${SCRIPT_DIR}/assets/dxvk" ]]; then
    DXVK_SRC="${SCRIPT_DIR}/assets/dxvk"
fi

if [[ -n "${DXVK_SRC}" ]]; then
    cp -f "${DXVK_SRC}"/*.dll "${DXVK_DIR}/" 2>/dev/null || true
    if [[ -f "${DXVK_SRC}/dxvk.conf" ]]; then
        cp -f "${DXVK_SRC}/dxvk.conf" "${DXVK_DIR}/dxvk.conf"
    fi
else
    echo "  [WARN] DXVK assets directory not found; skipping DXVK copy."
fi

# Install desktop icon and entry for GUI
echo "[4/4] Creating desktop entry at ${APPS_DIR}/rine.desktop..."
ICON_DIR="${HOME}/.local/share/icons/hicolor/256x256/apps"
mkdir -p "${ICON_DIR}" "${APPS_DIR}"
if [[ -f "${SCRIPT_DIR}/assets/logo.png" ]]; then
    cp -f "${SCRIPT_DIR}/assets/logo.png" "${ICON_DIR}/rine.png"
fi
cat > "${APPS_DIR}/rine.desktop" <<EOF
[Desktop Entry]
Name=Rine
Comment=Lightweight In-Process Win64 Compatibility Runtime
Exec=${BIN_DIR}/rine --gui
Icon=rine
StartupWMClass=rine
Terminal=false
Type=Application
Categories=Game;System;
EOF
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -f -t "${HOME}/.local/share/icons/hicolor" >/dev/null 2>&1 || true
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "${APPS_DIR}" >/dev/null 2>&1 || true

echo ""
echo "Installation complete!"
if [[ ":${PATH}:" != *":${BIN_DIR}:"* ]]; then
    echo "Note: ${BIN_DIR} is not in your PATH. Add this to your ~/.bashrc or ~/.zshrc:"
    echo "  export PATH=\"\${HOME}/.local/bin:\${PATH}\""
fi
echo "Run GUI: rine --gui"
echo "Run EXE: rine /path/to/game.exe"
