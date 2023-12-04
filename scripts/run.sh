#!/bin/sh

BINARY=__REPLACE__
PLATFORM=$(uname)

if [ -z "$BINARY" ]; then
    BINARY=bndl
fi

case $PLATFORM in
Linux)
    if [ $(uname -m) = "aarch64" ]; then
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

# Determine where on the filesystem the script is located, since it is most likely symlinked
LOCATION_ON_FILE_SYSTEM=$(dirname $([ -L $0 ] && readlink -f $0 || echo $0))
EXEC_BINARY=$LOCATION_ON_FILE_SYSTEM/../bin/$PLATFORM_BINARY

# Ensure the binary is executable
chmod +x $EXEC_BINARY

exec $EXEC_BINARY "$@"
