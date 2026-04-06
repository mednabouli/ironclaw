#!/usr/bin/env bash
# IronClaw installer — downloads the latest pre-built binary.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/mednabouli/ironclaw/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/mednabouli/ironclaw/main/install.sh | bash -s -- --version v0.1.1
#   curl -fsSL https://raw.githubusercontent.com/mednabouli/ironclaw/main/install.sh | bash -s -- --prefix ~/.local
#
# Environment variables:
#   IRONCLAW_VERSION  — override version (e.g. "v0.1.1")
#   IRONCLAW_PREFIX   — install prefix (default: /usr/local)
set -euo pipefail

REPO="mednabouli/ironclaw"
VERSION="${IRONCLAW_VERSION:-}"
PREFIX="${IRONCLAW_PREFIX:-/usr/local}"

# ── Parse arguments ─────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) VERSION="$2"; shift 2 ;;
    --prefix)  PREFIX="$2";  shift 2 ;;
    --help|-h)
      echo "Usage: install.sh [--version vX.Y.Z] [--prefix /path]"
      exit 0
      ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

# ── Detect OS ───────────────────────────────────────────────────────────────
detect_os() {
  local os
  os="$(uname -s)"
  case "$os" in
    Linux*)  echo "linux" ;;
    Darwin*) echo "macos" ;;
    MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
    *) echo "Unsupported OS: $os" >&2; exit 1 ;;
  esac
}

# ── Detect architecture ────────────────────────────────────────────────────
detect_arch() {
  local arch
  arch="$(uname -m)"
  case "$arch" in
    x86_64|amd64)   echo "x86_64" ;;
    aarch64|arm64)   echo "aarch64" ;;
    armv7l|armv7)    echo "armv7" ;;
    *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
  esac
}

# ── Resolve latest version from GitHub API ──────────────────────────────────
resolve_version() {
  if [[ -n "$VERSION" ]]; then
    echo "$VERSION"
    return
  fi
  local latest
  if command -v curl >/dev/null 2>&1; then
    latest="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/')"
  elif command -v wget >/dev/null 2>&1; then
    latest="$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/')"
  else
    echo "Error: curl or wget is required" >&2
    exit 1
  fi
  if [[ -z "$latest" ]]; then
    echo "Error: could not determine latest version" >&2
    exit 1
  fi
  echo "$latest"
}

# ── Download ────────────────────────────────────────────────────────────────
download() {
  local url="$1" dest="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$dest"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$dest" "$url"
  fi
}

# ── Main ────────────────────────────────────────────────────────────────────
main() {
  local os arch version archive_name url tmpdir bin_dir

  os="$(detect_os)"
  arch="$(detect_arch)"
  version="$(resolve_version)"

  echo "Installing IronClaw ${version} for ${os}/${arch}..."

  archive_name="ironclaw-${os}-${arch}.tar.gz"
  if [[ "$os" == "windows" ]]; then
    archive_name="ironclaw-windows-x86_64.zip"
  fi

  url="https://github.com/${REPO}/releases/download/${version}/${archive_name}"

  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' EXIT

  echo "Downloading ${url}..."
  download "$url" "${tmpdir}/${archive_name}"

  # Verify SHA256 if checksum file is available
  local sha_url="${url}.sha256"
  if download "$sha_url" "${tmpdir}/${archive_name}.sha256" 2>/dev/null; then
    echo "Verifying checksum..."
    local expected actual
    expected="$(awk '{print $1}' "${tmpdir}/${archive_name}.sha256")"
    if command -v sha256sum >/dev/null 2>&1; then
      actual="$(sha256sum "${tmpdir}/${archive_name}" | awk '{print $1}')"
    elif command -v shasum >/dev/null 2>&1; then
      actual="$(shasum -a 256 "${tmpdir}/${archive_name}" | awk '{print $1}')"
    fi
    if [[ -n "${actual:-}" && "$expected" != "$actual" ]]; then
      echo "Error: checksum mismatch!" >&2
      echo "  expected: $expected" >&2
      echo "  actual:   $actual" >&2
      exit 1
    fi
    echo "Checksum OK."
  fi

  # Extract
  echo "Extracting..."
  if [[ "$os" == "windows" ]]; then
    unzip -q "${tmpdir}/${archive_name}" -d "${tmpdir}"
  else
    tar xzf "${tmpdir}/${archive_name}" -C "${tmpdir}"
  fi

  # Install
  bin_dir="${PREFIX}/bin"
  if [[ -w "$bin_dir" ]] || [[ -w "$PREFIX" ]]; then
    mkdir -p "$bin_dir"
    install -m 755 "${tmpdir}/ironclaw" "${bin_dir}/ironclaw"
  else
    echo "Installing to ${bin_dir} (requires sudo)..."
    sudo mkdir -p "$bin_dir"
    sudo install -m 755 "${tmpdir}/ironclaw" "${bin_dir}/ironclaw"
  fi

  echo ""
  echo "IronClaw ${version} installed to ${bin_dir}/ironclaw"
  echo ""

  # Check if bin_dir is in PATH
  if ! echo "$PATH" | tr ':' '\n' | grep -qx "$bin_dir"; then
    echo "WARNING: ${bin_dir} is not in your PATH."
    echo "Add it with:  export PATH=\"${bin_dir}:\$PATH\""
    echo ""
  fi

  echo "Run 'ironclaw --help' to get started."
}

main
