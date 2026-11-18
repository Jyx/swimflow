#!/bin/sh
# swimflow installer: downloads a prebuilt release binary for this platform.
#
#   curl -fsSL https://raw.githubusercontent.com/Jyx/swimflow/main/install.sh | sh
#
# Linux and macOS are supported; Windows users download the .zip from
# https://github.com/Jyx/swimflow/releases instead.
#
# Environment (or the matching flag when piping through `sh -s --`):
#   SWIMFLOW_VERSION      release tag to install, e.g. v0.1.0 (default: latest)
#   SWIMFLOW_INSTALL_DIR  install directory (default: $HOME/.local/bin)
#
# Flags:
#   --version TAG   install this release tag instead of the latest one
#   --dir DIR       install into DIR instead of the default
#   -h, --help      show this help
#
# The archive is checked against the release's SHA256SUMS file when one is
# available; a mismatch is fatal.

set -eu

repo=Jyx/swimflow
bin=swimflow

version=${SWIMFLOW_VERSION:-}
install_dir=${SWIMFLOW_INSTALL_DIR:-$HOME/.local/bin}

usage() {
    cat <<'EOF'
swimflow installer: downloads a prebuilt release binary for this platform.

  curl -fsSL https://raw.githubusercontent.com/Jyx/swimflow/main/install.sh | sh

Environment:
  SWIMFLOW_VERSION      release tag to install, e.g. v0.1.0 (default: latest)
  SWIMFLOW_INSTALL_DIR  install directory (default: $HOME/.local/bin)

Flags (pass them after `sh -s --` when piping):
  --version TAG   install this release tag instead of the latest one
  --dir DIR       install into DIR instead of the default
  -h, --help      show this help
EOF
}

while [ $# -gt 0 ]; do
    case $1 in
        --version)
            [ $# -ge 2 ] || { echo "install.sh: --version needs a value" >&2; exit 2; }
            version=$2
            shift 2
            ;;
        --version=*)
            version=${1#--version=}
            shift
            ;;
        --dir)
            [ $# -ge 2 ] || { echo "install.sh: --dir needs a value" >&2; exit 2; }
            install_dir=$2
            shift 2
            ;;
        --dir=*)
            install_dir=${1#--dir=}
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "install.sh: unknown argument: $1 (try --help)" >&2
            exit 2
            ;;
    esac
done

die() {
    echo "install.sh: $*" >&2
    exit 1
}

need() {
    command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

need uname
need curl
need tar

os=$(uname -s 2>/dev/null || true)
arch=$(uname -m 2>/dev/null || true)

case $os in
    Linux) os=linux ;;
    Darwin) os=macos ;;
    *)
        die "unsupported operating system: ${os:-unknown}
Download a Windows binary from https://github.com/$repo/releases."
        ;;
esac

case $arch in
    x86_64|amd64) arch=x86_64 ;;
    arm64|aarch64) arch=arm64 ;;
    *) die "unsupported architecture: ${arch:-unknown}" ;;
esac

case $os/$arch in
    linux/x86_64) target=x86_64-unknown-linux-musl ;;
    linux/arm64) target=aarch64-unknown-linux-musl ;;
    macos/x86_64) target=x86_64-apple-darwin ;;
    macos/arm64) target=aarch64-apple-darwin ;;
    *) die "unsupported platform: $os/$arch" ;;
esac

if [ -z "$version" ]; then
    version=$(curl -fsSL "https://api.github.com/repos/$repo/releases/latest" |
        sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1) ||
        die "could not query the latest release"
    [ -n "$version" ] || die "could not determine the latest release; set SWIMFLOW_VERSION"
fi

archive="$bin-$version-$target.tar.gz"
base="https://github.com/$repo/releases/download/$version"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/$bin-install.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

echo "Downloading $archive"
curl -fsSL --retry 3 -o "$tmp/$archive" "$base/$archive" ||
    die "download failed: $base/$archive"

if curl -fsSL --retry 3 -o "$tmp/SHA256SUMS" "$base/SHA256SUMS" 2>/dev/null; then
    expected=$(awk -v f="$archive" '$2 == f { print $1 }' "$tmp/SHA256SUMS" | head -n 1)
    if [ -n "$expected" ]; then
        if command -v sha256sum >/dev/null 2>&1; then
            actual=$(sha256sum "$tmp/$archive" | awk '{ print $1 }')
        elif command -v shasum >/dev/null 2>&1; then
            actual=$(shasum -a 256 "$tmp/$archive" | awk '{ print $1 }')
        else
            actual=
            echo "install.sh: no sha256sum/shasum found; skipping checksum" >&2
        fi
        if [ -n "$actual" ] && [ "$actual" != "$expected" ]; then
            die "checksum mismatch for $archive (expected $expected, got $actual)"
        fi
    fi
else
    echo "install.sh: SHA256SUMS not available; skipping checksum" >&2
fi

tar -xzf "$tmp/$archive" -C "$tmp" || die "could not unpack $archive"
[ -f "$tmp/$bin" ] || die "archive did not contain $bin"

mkdir -p "$install_dir" || die "could not create $install_dir"
dest=$install_dir/$bin
# Install next to the target and rename, so an existing binary is replaced
# atomically (and a running one does not get clobbered mid-write).
cp "$tmp/$bin" "$dest.tmp.$$" || die "could not write to $install_dir"
chmod 0755 "$dest.tmp.$$"
mv -f "$dest.tmp.$$" "$dest" || die "could not install to $dest"

echo "Installed $dest"
case ":${PATH:-}:" in
    *":$install_dir:"*) ;;
    *)
        echo
        echo "$install_dir is not on your PATH. Add it with:"
        echo "  export PATH=\"$install_dir:\$PATH\""
        ;;
esac
"$dest" --version
