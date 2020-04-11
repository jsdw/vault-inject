#!/bin/bash
# A small script to build and package up the binaries for releasing
# Requires docker and git to be installed.

NAME=vault-inject
LINUX_TARGET=x86_64-unknown-linux-gnu
MAC_TARGET=x86_64-apple-darwin

build_linux () {
    TARGET=$LINUX_TARGET

    docker run \
        -it \
        --rm \
        --user "$(id -u)":"$(id -g)" \
        -v "$PWD":/code \
        -w /code rust:1.42.0 \
        cargo build --release --target $TARGET

    release_if_asked_to
}

build_macos () {
    TARGET=$MAC_TARGET

    # Build an image suitable for cross compiling the mac binary:
    docker build -f build/macos/Dockerfile -t vault-inject:macos build/macos/

    # Use this image to build our binary (similar to above):
    docker run \
        -it \
        --rm \
        --user "$(id -u)":"$(id -g)" \
        -v "$PWD":/code \
        vault-inject:macos

    release_if_asked_to
}

release_if_asked_to () {
    if [ "$RELEASE" == "yes" ]
    then
        tar czf $NAME-$LATEST_TAG-$TARGET.tar.gz --cd target/$TARGET/release $NAME
    fi
}

cleanup () {
    rm -rf target
    rm vault-inject*.tar.gz
    docker image rm rust:1.42.0 vault-inject:macos
}

# Check args are provided; usage instructions if not
if [ -z "$1" ]
then
    echo "Usage: docker-build.sh (linux|mac|all|clean|release) [--release]"
    echo ""
    echo "Commands:"
    echo "  linux   - build a linux binary"
    echo "  mac     - build a mac binary"
    echo "  all     - build a linux and mac binary"
    echo "  release - package up built binaries for release"
    echo "  clean   - delete everything that we created doing the above"
    echo ""
    echo "Flags:"
    echo "  --release - package up built binaries for a release after build"
    exit 1
fi

# Naive check that we're in the right folder:
if [ ! -f "Cargo.toml" ]
then
    echo "You appear to be in the wrong folder. Please run this from the root of the vault-inject project"
    exit 1
fi

# Do we want to package things for release?
if [ "$2" == "--release" -o "$1" == "release" ]
then
    RELEASE=yes
    LATEST_TAG=$(git describe --abbrev=0 --tags)
elif [ -n "$2" ]
then
    echo "\"$2\" provided but not understood; did you mean '--release'?"
fi

# Parse command:
if [ "$1" == "linux" ]
then
    build_linux
elif [ "$1" == "mac" ]
then
    build_macos
elif [ "$1" == "all" ]
then
    build_linux
    build_macos
elif [ "$1" == "release" ]
then
    TARGET=$LINUX_TARGET
    release_if_asked_to
    TARGET=$MAC_TARGET
    release_if_asked_to
elif [ "$1" == "clean" ]
then
    cleanup
else
    echo "\"$1\" provided but not understood; run with no args to see help"
    exit 1
fi