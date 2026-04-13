#!/usr/bin/env bash
# build-linux-cli.sh — Build localbolt-cli + bolt-daemon into a Linux tarball.
#
# LOCALBOLT-LINUX-CLI-IMPL-1
#
# Usage:
#   bash scripts/build-linux-cli.sh [--daemon-path /path/to/bolt-daemon]
#
# If --daemon-path is not provided, attempts to find or build bolt-daemon
# from the sibling bolt-daemon repo.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CLI_DIR="${REPO_ROOT}/native/linux/cli"
BUILD_DIR="${REPO_ROOT}/build/linux"
DAEMON_REPO="${REPO_ROOT}/../bolt-daemon"

CLI_VERSION="$(grep '^version' "${CLI_DIR}/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')"
ARCH="x86_64"
TARBALL_NAME="localbolt-cli-${CLI_VERSION}-${ARCH}-linux.tar.gz"

echo "=== localbolt-cli Linux build ==="
echo "  version:  ${CLI_VERSION}"
echo "  arch:     ${ARCH}"
echo ""

# ── Parse args ────────────────────────────────────────────────
DAEMON_PATH=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --daemon-path)
            DAEMON_PATH="$2"
            shift 2
            ;;
        *)
            echo "Unknown arg: $1" >&2
            exit 1
            ;;
    esac
done

# ── Build CLI ─────────────────────────────────────────────────
echo "[1/4] Building localbolt-cli..."
cd "${CLI_DIR}"
cargo build --release
CLI_BIN="${CLI_DIR}/target/release/localbolt-cli"

if [ ! -f "${CLI_BIN}" ]; then
    echo "ERROR: CLI binary not found at ${CLI_BIN}" >&2
    exit 1
fi
echo "  binary: ${CLI_BIN}"
echo "  size:   $(du -h "${CLI_BIN}" | cut -f1)"

# ── Find or build daemon ─────────────────────────────────────
echo ""
echo "[2/4] Locating bolt-daemon..."

if [ -n "${DAEMON_PATH}" ]; then
    if [ ! -f "${DAEMON_PATH}" ]; then
        echo "ERROR: daemon not found at ${DAEMON_PATH}" >&2
        exit 1
    fi
    DAEMON_BIN="${DAEMON_PATH}"
    echo "  provided: ${DAEMON_BIN}"
elif [ -d "${DAEMON_REPO}" ]; then
    echo "  building from ${DAEMON_REPO}..."
    cd "${DAEMON_REPO}"
    cargo build --release
    DAEMON_BIN="${DAEMON_REPO}/target/release/bolt-daemon"
    if [ ! -f "${DAEMON_BIN}" ]; then
        echo "ERROR: daemon build produced no binary" >&2
        exit 1
    fi
    echo "  binary: ${DAEMON_BIN}"
else
    # Check PATH
    if command -v bolt-daemon &>/dev/null; then
        DAEMON_BIN="$(command -v bolt-daemon)"
        echo "  found on PATH: ${DAEMON_BIN}"
    else
        echo "WARNING: bolt-daemon not found. Tarball will NOT include daemon." >&2
        echo "  Install bolt-daemon separately or use --daemon-path." >&2
        DAEMON_BIN=""
    fi
fi

# ── Stage tarball ─────────────────────────────────────────────
echo ""
echo "[3/4] Staging tarball..."

mkdir -p "${BUILD_DIR}"
STAGE_DIR="${BUILD_DIR}/localbolt-cli-${CLI_VERSION}"
rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}"

cp "${CLI_BIN}" "${STAGE_DIR}/localbolt-cli"
if [ -n "${DAEMON_BIN}" ]; then
    cp "${DAEMON_BIN}" "${STAGE_DIR}/bolt-daemon"
fi
cp "${CLI_DIR}/README.md" "${STAGE_DIR}/README.md"

# Generate install script
cat > "${STAGE_DIR}/install.sh" << 'INSTALL_EOF'
#!/usr/bin/env bash
# Install localbolt-cli + bolt-daemon to ~/.local/bin
set -euo pipefail
INSTALL_DIR="${HOME}/.local/bin"
mkdir -p "${INSTALL_DIR}"
cp localbolt-cli "${INSTALL_DIR}/"
chmod +x "${INSTALL_DIR}/localbolt-cli"
if [ -f bolt-daemon ]; then
    cp bolt-daemon "${INSTALL_DIR}/"
    chmod +x "${INSTALL_DIR}/bolt-daemon"
fi
echo "Installed to ${INSTALL_DIR}/"
echo "Ensure ${INSTALL_DIR} is in your PATH."
INSTALL_EOF
chmod +x "${STAGE_DIR}/install.sh"

echo "  staged: ${STAGE_DIR}/"
ls -la "${STAGE_DIR}/"

# ── Create tarball + checksum ─────────────────────────────────
echo ""
echo "[4/4] Creating tarball..."

cd "${BUILD_DIR}"
tar czf "${TARBALL_NAME}" "localbolt-cli-${CLI_VERSION}/"
sha256sum "${TARBALL_NAME}" > "${TARBALL_NAME}.sha256"

echo ""
echo "=== Build complete ==="
echo "  tarball:  ${BUILD_DIR}/${TARBALL_NAME}"
echo "  checksum: ${BUILD_DIR}/${TARBALL_NAME}.sha256"
echo "  size:     $(du -h "${BUILD_DIR}/${TARBALL_NAME}" | cut -f1)"
