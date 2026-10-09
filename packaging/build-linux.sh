#!/usr/bin/env bash
#
# Build the client for Linux x86-64 and package it as a .deb.
#
#   ./packaging/build-linux.sh                # build + package
#   ./packaging/build-linux.sh --skip-build   # repackage what is already built
#
# Output: dist/linux/rustibia_<version>_amd64.deb

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PKG_DIR="$ROOT/packaging/linux"
OUT_DIR="$ROOT/dist/linux"
STAGE_DIR="$OUT_DIR/stage"

BIN_NAME="rustibia-client"
SKIP_BUILD=0
VERSION=""

log()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
die()  { printf '\033[1;31merror\033[0m %s\n' "$*" >&2; exit 1; }

usage() {
    awk 'NR>=3 && /^#/ { sub(/^# ?/, ""); print; next } NR>=3 { exit }' "${BASH_SOURCE[0]}"
    exit "${1:-0}"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="${2:-}"; shift 2 ;;
        --version=*) VERSION="${1#*=}"; shift ;;
        --skip-build) SKIP_BUILD=1; shift ;;
        -h|--help) usage 0 ;;
        *) die "unknown argument: $1 (try --help)" ;;
    esac
done

if [[ -z "$VERSION" ]]; then
    VERSION="$(awk '/^\[package\]/{p=1;next} /^\[/{p=0} p && /^version[[:space:]]*=/{gsub(/[",]/,"",$3); print $3; exit}' "$ROOT/Cargo.toml")"
fi
[[ -n "$VERSION" ]] || die "could not read the package version from Cargo.toml"

command -v cargo >/dev/null 2>&1 || die "cargo not found; install Rust via rustup"
command -v dpkg-deb >/dev/null 2>&1 || die "dpkg-deb not found; sudo apt-get install dpkg"

build() {
    log "building $BIN_NAME $VERSION"
    cd "$ROOT"
    cargo build --profile production
}

stage() {
    local exe="$ROOT/target/production/$BIN_NAME"
    [[ -f "$exe" ]] || die "$exe not found -- drop --skip-build to build it first"

    log "staging into $STAGE_DIR"
    rm -rf "$STAGE_DIR"
    install -Dm755 "$exe" "$STAGE_DIR/opt/rustibia/$BIN_NAME"
    cp -r "$ROOT/assets" "$STAGE_DIR/opt/rustibia/assets"
    chmod -R u=rwX,go=rX "$STAGE_DIR/opt/rustibia/assets"
    install -Dm755 "$PKG_DIR/rustibia" "$STAGE_DIR/usr/bin/rustibia"
    install -Dm644 "$PKG_DIR/rustibia.desktop" "$STAGE_DIR/usr/share/applications/rustibia.desktop"
    install -Dm644 "$ROOT/packaging/icon.png" "$STAGE_DIR/usr/share/icons/hicolor/256x256/apps/rustibia.png"
    mkdir -p "$STAGE_DIR/DEBIAN"
    sed "s/@VERSION@/$VERSION/" "$PKG_DIR/control.in" > "$STAGE_DIR/DEBIAN/control"
    log "staged $(du -sh "$STAGE_DIR" | cut -f1)"
}

package() {
    local deb="$OUT_DIR/rustibia_${VERSION}_amd64.deb"
    dpkg-deb --root-owner-group --build "$STAGE_DIR" "$deb"
    log "package ready: $deb ($(du -h "$deb" | cut -f1))"
}

((SKIP_BUILD)) || build
stage
package
