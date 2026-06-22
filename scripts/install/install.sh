#!/bin/sh
# Pick standalone installer
# Usage: curl -fsSL https://github.com/vividcode-ai/pick/releases/latest/download/install.sh | sh

set -e

REPO="vividcode-ai/pick"
PICK_HOME="${HOME}/.pick"
PACKAGES_DIR="${PICK_HOME}/packages/standalone"
RELEASES_DIR="${PACKAGES_DIR}/releases"
BIN_DIR="${PICK_HOME}/bin"

# Detect platform
UNAME_S=$(uname -s)
UNAME_M=$(uname -m)

case "${UNAME_S}" in
    Linux)  OS="linux" ;;
    Darwin) OS="macos" ;;
    *)      echo "Unsupported OS: ${UNAME_S}"; exit 1 ;;
esac

case "${UNAME_M}" in
    x86_64|amd64) ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *)            echo "Unsupported architecture: ${UNAME_M}"; exit 1 ;;
esac

TARGET="${OS}-${ARCH}"
ARCHIVE="pick-package-${TARGET}.tar.gz"

# Fetch latest release info from GitHub API
echo "Fetching latest release info..."
LATEST=$(curl -sSL "https://api.github.com/repos/${REPO}/releases/latest" 2>&1) || true
if [ -z "${LATEST}" ]; then
    echo "Error: Could not fetch release info from GitHub (curl failed with no output)."
    exit 1
fi

# Check if the response is an error (e.g. rate limited)
echo "${LATEST}" | grep -q '"message"' && echo "${LATEST}" | grep -q '"documentation_url"' && {
    echo "Error: GitHub API responded with an error:"
    echo "${LATEST}" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('message','unknown error'))" 2>/dev/null || echo "${LATEST}"
    exit 1
}

VERSION=$(echo "${LATEST}" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": "//' | sed 's/".*//' | sed 's/^v//')
if [ -z "${VERSION}" ]; then
    echo "Error: Could not parse version from release info."
    exit 1
fi

echo "Latest version: ${VERSION}"

# Download archive
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/v${VERSION}/${ARCHIVE}"
echo "Downloading ${DOWNLOAD_URL}..."
mkdir -p "${RELEASES_DIR}/${VERSION}-${TARGET}"
curl -fsSL "${DOWNLOAD_URL}" -o "/tmp/${ARCHIVE}"

# Download and verify checksum
CHECKSUMS_URL="https://github.com/${REPO}/releases/download/v${VERSION}/pick-package_SHA256SUMS"
if curl -fsSL "${CHECKSUMS_URL}" -o "/tmp/pick-package_SHA256SUMS" 2>/dev/null; then
    EXPECTED=$(grep "${ARCHIVE}" /tmp/pick-package_SHA256SUMS | awk '{print $1}')
    if [ -n "${EXPECTED}" ]; then
        if command -v sha256sum >/dev/null 2>&1; then
            ACTUAL=$(sha256sum "/tmp/${ARCHIVE}" | awk '{print $1}')
        elif command -v shasum >/dev/null 2>&1; then
            ACTUAL=$(shasum -a 256 "/tmp/${ARCHIVE}" | awk '{print $1}')
        else
            echo "Warning: No sha256sum/shasum found, skipping checksum verification."
            ACTUAL="${EXPECTED}"
        fi
        if [ "${ACTUAL}" != "${EXPECTED}" ]; then
            echo "Error: Checksum mismatch!"
            echo "  Expected: ${EXPECTED}"
            echo "  Actual:   ${ACTUAL}"
            exit 1
        fi
        echo "Checksum verified."
    fi
    rm -f /tmp/pick-package_SHA256SUMS
fi

# Extract
tar -xzf "/tmp/${ARCHIVE}" -C "${RELEASES_DIR}/${VERSION}-${TARGET}"
rm -f "/tmp/${ARCHIVE}"

# Update current symlink
rm -f "${PACKAGES_DIR}/current"
ln -sf "${RELEASES_DIR}/${VERSION}-${TARGET}/pick" "${PACKAGES_DIR}/current"

# Update bin symlink
mkdir -p "${BIN_DIR}"
ln -sf "${PACKAGES_DIR}/current" "${BIN_DIR}/pick"

# Clean up old releases (keep last 2)
echo "Cleaning up old releases..."
ls -t "${RELEASES_DIR}" | tail -n +3 | while read -r old; do
    rm -rf "${RELEASES_DIR}/${old}"
done

# Add to PATH in shell profile
add_to_path() {
    local profile_file="$1"
    local line="export PATH=\"\${PATH}:${BIN_DIR}\""
    if [ -f "${profile_file}" ]; then
        if grep -q "${BIN_DIR}" "${profile_file}" 2>/dev/null; then
            return 0
        fi
        echo "" >> "${profile_file}"
        echo "# Pick CLI" >> "${profile_file}"
        echo "${line}" >> "${profile_file}"
        echo "  Added ${BIN_DIR} to ${profile_file}"
    fi
}

echo ""
echo "Adding ${BIN_DIR} to PATH..."
add_to_path "${HOME}/.profile"
case "${SHELL}" in
    *zsh*) add_to_path "${HOME}/.zshrc" ;;
    *bash*) add_to_path "${HOME}/.bashrc" ;;
esac
export PATH="${PATH}:${BIN_DIR}"

echo ""
echo "Pick v${VERSION} installed successfully!"
echo "Run 'pick update' to check for future updates."
echo "Run 'pick' to start a new session."
