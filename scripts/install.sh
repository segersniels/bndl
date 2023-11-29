#!/bin/sh

OWNER=segersniels
BINARY=bndl
PLATFORM=$(uname)
BIN_DIRECTORY=/usr/local/bin

function determine_platform_binary() {
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
}

function download_binary() {
    url="https://github.com/${OWNER}/${BINARY}/releases/latest/download/${PLATFORM_BINARY}"
    path="${BIN_DIRECTORY}/${BINARY}"

    echo "Downloading ${PLATFORM_BINARY}..."

    if which wget >/dev/null ; then
        sudo wget --quiet -O $path $url
    elif which curl >/dev/null ; then
        sudo curl -s -L $url -o $path
    else
        echo "Unable to download, neither `wget` nor `curl` are available."
    fi

    sudo chmod +x $path

    echo "Installed at ${path}"
}

sudo -v
determine_platform_binary
download_binary
