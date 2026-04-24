#!/bin/bash
# Cross-compile Python wheels for Android ARM64.
# Run this on a Mac/Linux build machine, then transfer wheels to the device.
#
# Prerequisites:
#   - Docker (uses a Termux-like build environment)
#   - OR: Android NDK installed (set ANDROID_NDK_HOME)
#
# Usage:
#   bash scripts/android/build-wheels.sh
#   # Then on device:
#   pip install /path/to/wheels/*.whl
#
# This avoids the 30-60 minute compilation on the phone for Rust/C packages.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
WHEEL_DIR="$REPO_ROOT/dist/android-arm64"
mkdir -p "$WHEEL_DIR"

# Packages that need compilation (have Rust/C extensions)
COMPILED_PACKAGES=(
    "pydantic-core"
    "jiter"
    "tiktoken"
    "asyncpg"
    "psycopg2-binary"
    "greenlet"
    "uvloop"
    "httptools"
)

echo "=== Building Android ARM64 wheels ==="
echo "Output: $WHEEL_DIR"
echo ""

# Check for Android NDK
if [ -n "${ANDROID_NDK_HOME:-}" ] && [ -d "$ANDROID_NDK_HOME" ]; then
    echo "Using Android NDK at: $ANDROID_NDK_HOME"
    BUILD_METHOD="ndk"
elif command -v docker &>/dev/null; then
    echo "Using Docker for cross-compilation"
    BUILD_METHOD="docker"
else
    echo "ERROR: Need either ANDROID_NDK_HOME or Docker installed"
    echo ""
    echo "Install Android NDK:"
    echo "  brew install android-ndk"
    echo "  export ANDROID_NDK_HOME=\$(brew --prefix)/share/android-ndk"
    echo ""
    echo "Or install Docker and try again."
    exit 1
fi

if [ "$BUILD_METHOD" = "docker" ]; then
    # Use a Termux-compatible Docker image to build wheels.
    # termux/termux-docker provides a full Termux environment.
    echo ">>> Building wheels in Docker (Termux environment)..."

    docker run --rm \
        --platform linux/arm64 \
        -v "$REPO_ROOT:/src" \
        -v "$WHEEL_DIR:/wheels" \
        python:3.13-slim \
        bash -c "
            apt-get update && apt-get install -y build-essential libpq-dev curl
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            source ~/.cargo/env
            pip wheel ${COMPILED_PACKAGES[*]} --wheel-dir /wheels
        "

    echo ""
    echo "=== Wheels built ==="
    ls -lh "$WHEEL_DIR"/*.whl 2>/dev/null || echo "No wheels found"
    echo ""
    echo "Transfer to device:"
    echo "  scp -P 8022 $WHEEL_DIR/*.whl user@device:~/wheels/"
    echo "  ssh -p 8022 user@device 'pip install ~/wheels/*.whl'"

elif [ "$BUILD_METHOD" = "ndk" ]; then
    # Cross-compile using Android NDK toolchain
    TOOLCHAIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64"
    export CC="$TOOLCHAIN/bin/aarch64-linux-android34-clang"
    export CXX="$TOOLCHAIN/bin/aarch64-linux-android34-clang++"
    export AR="$TOOLCHAIN/bin/llvm-ar"
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC"

    echo ">>> Building wheels with NDK cross-compilation..."
    for pkg in "${COMPILED_PACKAGES[@]}"; do
        echo "  Building $pkg..."
        pip wheel "$pkg" --wheel-dir "$WHEEL_DIR" --no-deps 2>&1 | tail -3
    done

    echo ""
    echo "=== Wheels built ==="
    ls -lh "$WHEEL_DIR"/*.whl 2>/dev/null || echo "No wheels found"
fi
