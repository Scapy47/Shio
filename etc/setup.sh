#!/usr/bin/env sh

# Detect OS using uname
OS_NAME="$(uname -s)"

# Default value
PLATFORM="unknown"

case "$OS_NAME" in
    Linux)
        # Check if running inside Termux (Android)
        if [ -n "$PREFIX" ] && echo "$PREFIX" | grep -qi "termux"; then
            PLATFORM="android"
        else
            PLATFORM="linux"
        fi
        ;;
    Darwin)
        PLATFORM="macos"
        ;;
    CYGWIN*|MINGW*|MSYS*)
        PLATFORM="windows"
        ;;
    *)
        PLATFORM="unknown"
        ;;
esac

# Export environment variable
export MY_PLATFORM="$PLATFORM"

# Optional: set additional variables per platform
case "$MY_PLATFORM" in
    android)
	export SHIO_PLAYER_CMD="termux-open {url} --content-type video"
        ;;
    linux)
        export SHIO_PLAYER_CMD="mpv {url}"
        ;;
    windows)
        export SHIO_PLAYER_CMD="mpv {url}"
        ;;
    *)
        export SHIO_PLAYER_CMD="mpv {url}"
        ;;
esac

echo "Detected platform: $MY_PLATFORM"
echo "MY_ENV_VAR set to: $SHIO_PLAYER_CMD"
