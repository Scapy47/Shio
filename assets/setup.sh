#!/usr/bin/env sh
set -e

OWNER="Scapy47"
REPO="Shio"
BASE_URL="https://github.com/$OWNER/$REPO/releases/latest/download"

case "$(uname -m)" in
  x86_64)         ARCH="x86_64" ;;
  arm64|aarch64)  echo "arm64 and aarch64 architecture are currently Unsupported"; exit 1 ;;
  *)              echo "Unsupported architecture"; exit 1 ;;
esac

case "$(uname)" in
  Darwin) OS="macOS" ;;
  Linux)  OS="Linux" ;;
  *)      echo "Unsupported OS"; exit 1 ;;
esac

FILENAME="shio-${OS}-${ARCH}"

printf "Try shio before installation? (!! Run directly !!) (y/n): "
while true; do
    read -r choice
    case "$choice" in
        y|Y)
            TMP_DIR=$(mktemp -d)
            trap 'rm -rf "$TMP_DIR"' EXIT
            curl -fL -o "$TMP_DIR/shio" "$BASE_URL/$FILENAME" || { echo "Failed to Download $BASE_URL/$FILENAME"; exit 1; }
            chmod +x "$TMP_DIR/shio"
            "$TMP_DIR/shio" "$@"
            printf "Proceed with installation? (y/n): "
            read -r install_choice
            case "$install_choice" in
                y|Y) break ;;
                *)   exit 0 ;;
            esac
            ;;
        n|N)
            exit 0
            ;;
        *)
            echo "Please answer y or n."
            ;;
    esac
done

INSTALL_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
FINAL_PATH="$INSTALL_DIR/shio"
mkdir -p "$INSTALL_DIR"

echo "Downloading to $FINAL_PATH"
curl -fL -o "$FINAL_PATH" "$BASE_URL/$FILENAME" || { echo "Download failed"; exit 1; }
chmod +x "$FINAL_PATH"
echo "Installed to $FINAL_PATH"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    echo ""
    echo "Warning: $INSTALL_DIR is not in your PATH"
    echo "Add to your shell config (~/.bashrc, ~/.zshrc, etc.):"
    echo '  export PATH="$HOME/.local/bin:$PATH"'
    ;;
esac

echo ""
echo "Run 'shio --version' to verify."
echo ""
echo "To enable playback, add one of the following to your shell config:"
echo '  # mpv'
echo '  export SHIO_PLAYER_CMD="mpv --user-agent={user_agent} --http-header-fields=\"Referer: {referer}\" {url}"'
echo '  # VLC'
echo '  export SHIO_PLAYER_CMD="vlc --http-user-agent={user_agent} --http-referrer={referer} {url}"'
