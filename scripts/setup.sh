#!/bin/bash
set -e

PLUGIN_ROOT="${CLAUDE_PLUGIN_ROOT:-$(dirname "$(dirname "$0")")}"
BIN_DIR="${PLUGIN_ROOT}/bin"
BINARY_NAME="no-comment-hook"
REPO="chenhunghan/no-comment-hook"

get_latest_version() {
  curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | \
    grep '"tag_name"' | sed -E 's/.*"tag_name": *"[^"]*-v([0-9]+\.[0-9]+\.[0-9]+)".*/\1/'
}

get_installed_version() {
  if [[ -x "$BIN_DIR/$BINARY_NAME" ]]; then
    "$BIN_DIR/$BINARY_NAME" --version 2>/dev/null || echo ""
  else
    echo ""
  fi
}

INSTALLED_VERSION=$(get_installed_version)
LATEST_VERSION=$(get_latest_version)

if [[ -x "$BIN_DIR/$BINARY_NAME" ]] && [[ -n "$INSTALLED_VERSION" ]] && [[ -n "$LATEST_VERSION" ]]; then
  INSTALLED_NORMALIZED="${INSTALLED_VERSION#v}"
  LATEST_NORMALIZED="${LATEST_VERSION#v}"
  if [[ "$INSTALLED_NORMALIZED" == "$LATEST_NORMALIZED" ]]; then
    exit 0
  fi
fi

if [[ -z "$LATEST_VERSION" ]] && [[ -x "$BIN_DIR/$BINARY_NAME" ]]; then
  exit 0
fi

mkdir -p "$BIN_DIR"

OS=$(uname -s)
ARCH=$(uname -m)

case "$OS-$ARCH" in
  Darwin-arm64)
    PLATFORM="aarch64-apple-darwin"
    ;;
  Darwin-x86_64)
    PLATFORM="x86_64-apple-darwin"
    ;;
  Linux-x86_64)
    PLATFORM="x86_64-unknown-linux-gnu"
    ;;
  Linux-aarch64)
    PLATFORM="aarch64-unknown-linux-gnu"
    ;;
  *)
    echo "{\"continue\": true, \"systemMessage\": \"[no-comment-hook] unsupported platform $OS-$ARCH\"}"
    exit 0
    ;;
esac

RELEASE_URL="https://github.com/${REPO}/releases/latest/download/${BINARY_NAME}-${PLATFORM}.tar.gz"

cd "$BIN_DIR"
if curl -fsSL "$RELEASE_URL" | tar xz 2>/dev/null; then
  chmod +x "$BIN_DIR/$BINARY_NAME"
  echo "{\"continue\": true, \"systemMessage\": \"[no-comment-hook] binary installed ($LATEST_VERSION)\"}"
else
  echo "{\"continue\": true, \"systemMessage\": \"[no-comment-hook] failed to download binary from $RELEASE_URL\"}"
fi

exit 0
