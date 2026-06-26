#!/usr/bin/env bash
# Build all release binaries: x86_64/aarch64/armv7 musl + x86_64 windows-gnu.
# Output stripped binaries into ./dist/
set -euo pipefail

DIST="$(pwd)/dist"
rm -rf "$DIST"
mkdir -p "$DIST"

build_linux_musl() {
    local target="$1"
    echo "==> $target"
    cargo build --release --target "$target"
    cp "target/$target/release/shell-remote" "$DIST/shell-remote-${target%%-unknown-linux-musl}"
    strip "$DIST/shell-remote-${target%%-unknown-linux-musl}" 2>/dev/null || true
}

build_armv7() {
    local target="armv7-unknown-linux-musleabihf"
    echo "==> $target (clang + self-contained musl)"
    local sc="$(rustc --print sysroot)/lib/rustlib/$target/lib/self-contained"
    CC_armv7_unknown_linux_musleabihf=clang \
    CXX_armv7_unknown_linux_musleabihf=clang++ \
    AR_armv7_unknown_linux_musleabihf=llvm-ar \
    CFLAGS_armv7_unknown_linux_musleabihf="--target=armv7-unknown-linux-musleabihf --sysroot=$sc" \
    cargo build --release --target "$target"
    cp "target/$target/release/shell-remote" "$DIST/shell-remote-armv7"
    strip "$DIST/shell-remote-armv7" 2>/dev/null || true
}

build_windows() {
    local target="x86_64-pc-windows-gnu"
    echo "==> $target (mingw, +crt-static)"
    cargo build --release --target "$target"
    cp "target/$target/release/shell-remote.exe" "$DIST/shell-remote-x86_64.exe"
}

build_linux_musl "x86_64-unknown-linux-musl"
build_linux_musl "aarch64-unknown-linux-musl"
build_armv7
build_windows

echo "==> dist:"
ls -lh "$DIST"
