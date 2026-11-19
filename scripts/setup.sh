#!/bin/bash
set -e

PLUGIN_ROOT="${CLAUDE_PLUGIN_ROOT:-$(dirname "$(dirname "$0")")}"
BIN_DIR="${PLUGIN_ROOT}/bin"
BINARY_NAME="no-comment-hook"
BINARY="${BIN_DIR}/${BINARY_NAME}"
REPO="chenhunghan/no-comment-hook"

message() {
  echo "{\"continue\": true, \"systemMessage\": \"[no-comment-hook] $1\"}"
  exit 0
}

# The binary is pinned to this plugin's own version so it always matches the
# flags in hooks/hooks.json. release-please bumps plugin.json and cuts the
# matching tag in the same release.
VERSION=$(sed -nE 's/^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' \
  "${PLUGIN_ROOT}/.claude-plugin/plugin.json" 2>/dev/null | head -n 1)

INSTALLED=""
if [[ -x "$BINARY" ]]; then
  INSTALLED=$("$BINARY" --version 2>/dev/null || true)
fi

if [[ -z "$VERSION" ]]; then
  [[ -n "$INSTALLED" ]] && exit 0
  message "cannot read plugin version from .claude-plugin/plugin.json"
fi

if [[ "${INSTALLED#v}" == "$VERSION" ]]; then
  exit 0
fi

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) PLATFORM="aarch64-apple-darwin" ;;
  Darwin-x86_64) PLATFORM="x86_64-apple-darwin" ;;
  Linux-x86_64) PLATFORM="x86_64-unknown-linux-gnu" ;;
  Linux-aarch64 | Linux-arm64) PLATFORM="aarch64-unknown-linux-gnu" ;;
  *) message "unsupported platform $(uname -s)-$(uname -m)" ;;
esac

URL="https://github.com/${REPO}/releases/download/${BINARY_NAME}-v${VERSION}/${BINARY_NAME}-${PLATFORM}.tar.gz"

mkdir -p "$BIN_DIR"
# Staged inside BIN_DIR so the final mv is an atomic rename on one filesystem:
# a dropped download or a concurrent session never leaves a half-written binary.
STAGE=$(mktemp -d "${BIN_DIR}/.download.XXXXXX")
trap 'rm -rf "$STAGE"' EXIT

if curl -fsL --connect-timeout 5 --max-time 60 "$URL" -o "$STAGE/archive.tar.gz" &&
  tar xzf "$STAGE/archive.tar.gz" -C "$STAGE" "$BINARY_NAME" 2>/dev/null &&
  chmod +x "$STAGE/$BINARY_NAME" &&
  [[ "$("$STAGE/$BINARY_NAME" --version 2>/dev/null)" == "$VERSION" ]]; then
  mv -f "$STAGE/$BINARY_NAME" "$BINARY"
  message "binary installed ($VERSION)"
fi

if [[ -n "$INSTALLED" ]]; then
  message "could not download $VERSION, keeping $INSTALLED; will retry next session"
fi
message "could not download $VERSION from $URL; will retry next session"
