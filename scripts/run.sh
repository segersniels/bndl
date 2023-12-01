#!/bin/sh

PLATFORM=$(uname)

if [ -z "$BINARY" ]; then
    BINARY=bndl
fi

case $PLATFORM in
Linux)
    if [[ $(uname -m) == "aarch64" ]]; then
    PLATFORM_BINARY="${BINARY}-aarch64-linux"
    else
    PLATFORM_BINARY="${BINARY}-amd64-linux"
    fi
    ;;
Darwin)
    PLATFORM_BINARY="${BINARY}-macos"
    ;;
*)
    echo "Unsupported platform: $PLATFORM"
    exit 0
    ;;
esac

exec ./bin/$PLATFORM_BINARY "$@"
