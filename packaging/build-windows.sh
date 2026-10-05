#!/usr/bin/env bash
#
# Cross-compile the client for Windows x64 from Linux and wrap it in an
# Inno Setup installer.
#
#   ./packaging/build-windows.sh                # build + package (MSVC target)
#   ./packaging/build-windows.sh --setup        # install the missing tooling
#   ./packaging/build-windows.sh --toolchain gnu
#   ./packaging/build-windows.sh --skip-build   # repackage what is already built
#
#   # point the shipped client_conf.yaml at a real host
#   ./packaging/build-windows.sh --server play.example.com:5555 \
#                                --site http://play.example.com:8080
#
# Output: dist/windows/SetupRustibia-<version>.exe

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PKG_DIR="$ROOT/packaging"
OUT_DIR="$ROOT/dist/windows"
STAGE_DIR="$OUT_DIR/stage"

BIN_NAME="rustibia-client"
TOOLCHAIN="msvc"
DO_SETUP=0
SKIP_BUILD=0
VERSION=""
SERVER_ADDRESS=""
SITE_URL=""

# Inno Setup 6 rather than 7: it is the version that runs reliably under wine.
INNO_DL_PAGE="https://jrsoftware.org/isdl.php"
INNO_FALLBACK_URL="https://github.com/jrsoftware/issrc/releases/download/is-6_7_3/innosetup-6.7.3.exe"

log()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m warn\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31merror\033[0m %s\n' "$*" >&2; exit 1; }

usage() {
    awk 'NR>=3 && /^#/ { sub(/^# ?/, ""); print; next } NR>=3 { exit }' "${BASH_SOURCE[0]}"
    exit "${1:-0}"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --toolchain) TOOLCHAIN="${2:-}"; shift 2 ;;
        --toolchain=*) TOOLCHAIN="${1#*=}"; shift ;;
        --version) VERSION="${2:-}"; shift 2 ;;
        --version=*) VERSION="${1#*=}"; shift ;;
        --server) SERVER_ADDRESS="${2:-}"; shift 2 ;;
        --server=*) SERVER_ADDRESS="${1#*=}"; shift ;;
        --site) SITE_URL="${2:-}"; shift 2 ;;
        --site=*) SITE_URL="${1#*=}"; shift ;;
        --setup) DO_SETUP=1; shift ;;
        --skip-build) SKIP_BUILD=1; shift ;;
        -h|--help) usage 0 ;;
        *) die "unknown argument: $1 (try --help)" ;;
    esac
done

case "$TOOLCHAIN" in
    msvc) TARGET="x86_64-pc-windows-msvc" ;;
    gnu)  TARGET="x86_64-pc-windows-gnu" ;;
    *)    die "--toolchain must be 'msvc' or 'gnu', got '$TOOLCHAIN'" ;;
esac

if [[ -z "$VERSION" ]]; then
    # First `version = "..."` after the [package] header.
    VERSION="$(awk '/^\[package\]/{p=1;next} /^\[/{p=0} p && /^version[[:space:]]*=/{gsub(/[",]/,"",$3); print $3; exit}' "$ROOT/Cargo.toml")"
fi
[[ -n "$VERSION" ]] || die "could not read the package version from Cargo.toml"

# ---------------------------------------------------------------- prerequisites

APT_MISSING=()
CARGO_MISSING=()

need_apt() { command -v "$1" >/dev/null 2>&1 || APT_MISSING+=("$2"); }

check_prereqs() {
    command -v cargo >/dev/null 2>&1 || die "cargo not found; install Rust via rustup"

    if [[ "$TOOLCHAIN" == "msvc" ]]; then
        # cargo-xwin downloads the MSVC CRT + Windows SDK headers/libs and links
        # them with clang/lld, so no Windows machine or Visual Studio is needed.
        command -v cargo-xwin >/dev/null 2>&1 || CARGO_MISSING+=("cargo-xwin")
        need_apt clang clang
        need_apt lld-link lld
        need_apt llvm-rc llvm
    else
        need_apt x86_64-w64-mingw32-gcc mingw-w64
        # build.rs embeds the icon through windres on this target.
        need_apt x86_64-w64-mingw32-windres binutils-mingw-w64-x86-64
    fi

    # Inno Setup is a Windows program; wine runs its compiler well enough. Only
    # needed when there is no native iscc (some distros package one).
    if ! command -v iscc >/dev/null 2>&1; then
        need_apt wine wine
    fi
    need_apt curl curl
}

install_prereqs() {
    if ((${#CARGO_MISSING[@]})); then
        log "cargo install ${CARGO_MISSING[*]}"
        cargo install "${CARGO_MISSING[@]}"
    fi
    if ((${#APT_MISSING[@]})); then
        log "sudo apt-get install ${APT_MISSING[*]}"
        sudo apt-get install -y "${APT_MISSING[@]}"
    fi
    log "rustup target add $TARGET"
    rustup target add "$TARGET"
    install_inno
    log "setup done -- rerun without --setup to build"
}

report_missing() {
    ((${#APT_MISSING[@]} + ${#CARGO_MISSING[@]})) || return 0
    {
        echo
        echo "Missing tooling for the $TOOLCHAIN target. Run:"
        echo
        if ((${#APT_MISSING[@]})); then   echo "    sudo apt-get install ${APT_MISSING[*]}"; fi
        if ((${#CARGO_MISSING[@]})); then echo "    cargo install ${CARGO_MISSING[*]}"; fi
        echo "    rustup target add $TARGET"
        echo
        echo "Or let this script do it: ./packaging/build-windows.sh --setup"
    } >&2
    exit 1
}

# ------------------------------------------------------------------ inno setup

# Dedicated prefix: keeps Inno Setup out of the user's own ~/.wine (which may be
# configured for something else entirely) and makes the build reproducible.
WINEPREFIX="${WINEPREFIX:-${XDG_DATA_HOME:-$HOME/.local/share}/rustibia-packaging/wine}"
export WINEPREFIX
export WINEDEBUG="${WINEDEBUG:--all}"

init_wineprefix() {
    [[ -f "$WINEPREFIX/system.reg" ]] && return 0
    log "initialising wine prefix at $WINEPREFIX"
    mkdir -p "$WINEPREFIX"
    wineboot -i >/dev/null 2>&1 || die "wineboot failed for $WINEPREFIX"
}

find_iscc() {
    if command -v iscc >/dev/null 2>&1; then
        ISCC_CMD=(iscc)
        return 0
    fi
    local candidate
    for candidate in \
        "$WINEPREFIX/drive_c/Program Files (x86)/Inno Setup 6/ISCC.exe" \
        "$WINEPREFIX/drive_c/Program Files/Inno Setup 6/ISCC.exe"
    do
        if [[ -f "$candidate" ]]; then
            ISCC_CMD=(wine "$candidate")
            return 0
        fi
    done
    return 1
}

install_inno() {
    find_iscc && { log "Inno Setup already present"; return 0; }
    local installer="$OUT_DIR/is-setup.exe"
    mkdir -p "$OUT_DIR"
    init_wineprefix

    # The releases live on GitHub and the URL carries the version, so scrape the
    # current 6.x link off the download page and fall back to a pinned build.
    local url
    url="$(curl -fsSL "$INNO_DL_PAGE" \
        | grep -oE 'https://github\.com/jrsoftware/issrc/releases/download/is-6[^"]*\.exe' \
        | head -1)" || true
    [[ -n "$url" ]] || url="$INNO_FALLBACK_URL"

    log "downloading ${url##*/}"
    curl -fL --progress-bar -o "$installer" "$url"
    log "installing Inno Setup into $WINEPREFIX (silent)"
    wine "$installer" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- || true
    rm -f "$installer"
    find_iscc || die "Inno Setup install failed; install it manually under wine"
}

# ----------------------------------------------------------------------- build

build() {
    log "building $BIN_NAME $VERSION for $TARGET"
    cd "$ROOT"
    if [[ "$TOOLCHAIN" == "msvc" ]]; then
        # Accepting the Microsoft redistributable licence is what lets xwin pull
        # the SDK; see https://github.com/Jake-Shadle/xwin for the terms.
        XWIN_ACCEPT_LICENSE=1 cargo xwin build --profile production --target "$TARGET"
    else
        cargo build --profile production --target "$TARGET"
    fi
}

stage() {
    local exe="$ROOT/target/$TARGET/production/$BIN_NAME.exe"
    [[ -f "$exe" ]] || die "$exe not found -- drop --skip-build to build it first"

    log "staging into $STAGE_DIR"
    rm -rf "$STAGE_DIR"
    mkdir -p "$STAGE_DIR"
    cp "$exe" "$STAGE_DIR/"
    cp -r "$ROOT/assets" "$STAGE_DIR/assets"
    stage_config
    log "staged $(du -sh "$STAGE_DIR" | cut -f1)"
}

# The client reads client_conf.yaml from its install directory, so the address a
# tester connects to is decided here rather than at compile time. They can still
# edit the installed file afterwards -- the installer leaves an existing one alone.
stage_config() {
    local dest="$STAGE_DIR/client_conf.yaml"
    cp "$PKG_DIR/client_conf.yaml" "$dest"
    if [[ -n "$SERVER_ADDRESS" ]]; then
        sed -i "s|^server_address:.*|server_address: \"$SERVER_ADDRESS\"|" "$dest"
    fi
    if [[ -n "$SITE_URL" ]]; then
        sed -i "s|^site_url:.*|site_url: \"$SITE_URL\"|" "$dest"
    fi
    log "shipping $(grep -E '^(server_address|site_url):' "$dest" | tr '\n' ' ')"
}

package() {
    find_iscc || install_inno
    cp "$PKG_DIR/installer.iss" "$OUT_DIR/installer.iss"
    if [[ -f "$PKG_DIR/icon.ico" ]]; then cp "$PKG_DIR/icon.ico" "$OUT_DIR/icon.ico"; fi

    log "compiling the installer"
    # ISCC resolves relative paths against the .iss it is given, so run from the
    # output dir and keep every path relative -- that dodges wine path mangling.
    ( cd "$OUT_DIR" && "${ISCC_CMD[@]}" \
        "/DAppVersion=$VERSION" \
        "/DStageDir=stage" \
        "/DOutputDir=." \
        installer.iss )

    local setup="$OUT_DIR/SetupRustibia-$VERSION.exe"
    [[ -f "$setup" ]] || die "ISCC reported success but $setup is missing"
    log "installer ready: $setup ($(du -h "$setup" | cut -f1))"
}

check_prereqs
if ((DO_SETUP)); then
    install_prereqs
    exit 0
fi
report_missing

((SKIP_BUILD)) || build
stage
package
