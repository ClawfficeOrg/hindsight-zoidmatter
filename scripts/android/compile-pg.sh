#!/bin/bash
# Cross-compile PostgreSQL 18 for Android ARM64 using the Android NDK.
#
# This produces PG binaries with:
# - Correct prefix path (/data/data/io.hs.pgdb/files/usr)
# - initdb patched to find "libpostgres.so" instead of "postgres"
# - Android-compatible shared memory (mmap, unnamed POSIX semaphores)
# - All binaries named as lib*.so for Android SELinux compatibility
#
# Prerequisites:
#   - Android NDK: brew install --cask android-ndk
#   - Docker (for cross-compilation environment)
#
# Usage: bash scripts/android/compile-pg.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTPUT_DIR="$REPO_ROOT/dist/android-arm64/pg-compiled"
PG_VERSION="18.2"
APP_PREFIX="/data/data/io.hs.pgdb/files/usr"

NDK_HOME="${ANDROID_NDK_HOME:-/opt/homebrew/share/android-ndk}"
TOOLCHAIN="$NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64"

if [ ! -d "$TOOLCHAIN" ]; then
    echo "ERROR: Android NDK not found at $NDK_HOME"
    exit 1
fi

mkdir -p "$OUTPUT_DIR"

echo "=== Cross-compiling PostgreSQL $PG_VERSION for Android ARM64 ==="
echo "  NDK: $NDK_HOME"
echo "  Prefix: $APP_PREFIX"
echo "  Output: $OUTPUT_DIR"
echo ""

# Download PG source if needed
PG_SRC="/tmp/postgresql-${PG_VERSION}"
if [ ! -d "$PG_SRC" ]; then
    echo ">>> Downloading PostgreSQL $PG_VERSION..."
    curl -sL "https://ftp.postgresql.org/pub/source/v${PG_VERSION}/postgresql-${PG_VERSION}.tar.bz2" -o /tmp/pg.tar.bz2
    cd /tmp && tar xjf pg.tar.bz2
fi

# Apply patches
echo ">>> Applying Android patches..."
cd "$PG_SRC"

# Patch 1: initdb looks for "libpostgres.so" instead of "postgres"
# Also patch pg_ctl and error messages
sed -i '' 's/find_other_exec(argv0, "postgres"/find_other_exec(argv0, "libpostgres.so"/g' \
    src/bin/initdb/initdb.c \
    src/bin/pg_ctl/pg_ctl.c

sed -i '' 's/"program \\"%s\\" is needed by %s"/"program \\"%s\\" is needed by %s"/g' \
    src/bin/initdb/initdb.c

# Patch 2: Default to mmap for dynamic shared memory on Android
cat >> src/include/pg_config_manual.h << 'EOF'

/* Android: force mmap for dynamic shared memory */
#ifdef __ANDROID__
#define DEFAULT_DYNAMIC_SHARED_MEMORY_TYPE DSM_IMPL_MMAP
#endif
EOF

# Patch 3: Replace /tmp with app-writable path
sed -i '' "s|/tmp|${APP_PREFIX}/tmp|g" src/bin/initdb/initdb.c

echo ">>> Configuring..."
CC="$TOOLCHAIN/bin/aarch64-linux-android24-clang"
CXX="$TOOLCHAIN/bin/aarch64-linux-android24-clang++"
AR="$TOOLCHAIN/bin/llvm-ar"
RANLIB="$TOOLCHAIN/bin/llvm-ranlib"
STRIP="$TOOLCHAIN/bin/llvm-strip"

# Minimal configure - no ICU, no XML, no SSL for now (reduces deps)
./configure \
    --host=aarch64-linux-android \
    --prefix="$APP_PREFIX" \
    --without-icu \
    --without-libxml \
    --without-openssl \
    --without-readline \
    --without-zlib \
    --disable-nls \
    --disable-rpath \
    CC="$CC" \
    CXX="$CXX" \
    AR="$AR" \
    RANLIB="$RANLIB" \
    STRIP="$STRIP" \
    CFLAGS="-fstack-protector-strong -Os" \
    LDFLAGS="-L${APP_PREFIX}/lib" \
    USE_UNNAMED_POSIX_SEMAPHORES=1 \
    pgac_cv_prog_cc_LDFLAGS_EX_BE__Wl___export_dynamic=yes \
    pgac_cv_prog_cc_LDFLAGS__Wl___as_needed=yes \
    2>&1 | tail -10

echo ">>> Building..."
make -j$(nproc) 2>&1 | tail -5

echo ">>> Installing to staging..."
STAGING="/tmp/pg-android-staging"
rm -rf "$STAGING"
make DESTDIR="$STAGING" install 2>&1 | tail -5

echo ">>> Packaging..."
# Copy binaries with lib*.so naming for Android
mkdir -p "$OUTPUT_DIR/jniLibs/arm64-v8a"
for bin in postgres initdb createdb psql pg_isready pg_ctl pg_dump; do
    src="$STAGING/$APP_PREFIX/bin/$bin"
    if [ -f "$src" ]; then
        cp "$src" "$OUTPUT_DIR/jniLibs/arm64-v8a/lib${bin}.so"
        echo "  lib${bin}.so"
    fi
done

# Copy shared libraries
mkdir -p "$OUTPUT_DIR/lib"
cp -a "$STAGING/$APP_PREFIX/lib/"*.so* "$OUTPUT_DIR/lib/" 2>/dev/null || true
cp -a "$STAGING/$APP_PREFIX/lib/postgresql/" "$OUTPUT_DIR/lib/postgresql/" 2>/dev/null || true

# Copy share data
mkdir -p "$OUTPUT_DIR/share"
cp -a "$STAGING/$APP_PREFIX/share/postgresql/" "$OUTPUT_DIR/share/postgresql/"

# Package as tar for assets
cd "$OUTPUT_DIR"
tar czf "$REPO_ROOT/dist/android-arm64/pg-compiled.tar.gz" lib/ share/

echo ""
echo "=== Build complete ==="
ls -lh "$REPO_ROOT/dist/android-arm64/pg-compiled.tar.gz"
echo ""
echo "Binaries:"
ls -lh "$OUTPUT_DIR/jniLibs/arm64-v8a/"
echo ""
echo "To use in APK:"
echo "  cp -r $OUTPUT_DIR/jniLibs/arm64-v8a/* hindsight-android-sdk/app/src/main/jniLibs/arm64-v8a/"
echo "  cp $REPO_ROOT/dist/android-arm64/pg-compiled.tar.gz hindsight-android-sdk/app/src/main/assets/postgres-arm64.tar.gz"
