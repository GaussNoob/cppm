#!/bin/sh
# Download a verified GitHub release, then run its per-user installer.
set -eu

release_base='https://github.com/GaussNoob/cppm/releases'
version=''
bin_dir="${HOME}/.local/bin"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --version|--prefix|--bin-dir)
            [ "$#" -ge 2 ] || { echo "Missing value for $1" >&2; exit 2; }
            case "$1" in
                --version) version=${2#v} ;;
                --prefix) bin_dir="$2/bin" ;;
                --bin-dir) bin_dir=$2 ;;
            esac
            shift 2
            ;;
        --help)
            echo 'Usage: sh bootstrap.sh [--version X.Y.Z] [--prefix directory | --bin-dir directory]'
            exit 0 ;;
        *) echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done
[ "$(uname -s)" = Linux ] || { echo 'This installer requires Linux; use bootstrap.ps1 on Windows.' >&2; exit 1; }
case "$(uname -m)" in
    x86_64|amd64) arch=x86_64 ;;
    aarch64|arm64) arch=arm64 ;;
    *) echo 'Unsupported architecture; expected x86_64 or ARM64.' >&2; exit 1 ;;
esac
for tool in curl tar sha256sum; do
    command -v "$tool" >/dev/null 2>&1 || { echo "Install $tool first." >&2; exit 1; }
done
download() {
    curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' \
        --connect-timeout 10 --max-time 180 --retry 2 --output "$2" "$1"
}
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT
trap 'exit 1' HUP INT TERM
if [ -z "$version" ]; then
    download "$release_base/latest/download/VERSION" "$temporary/VERSION"
    version=$(cat "$temporary/VERSION")
fi
# Only stable versions are accepted, including when explicitly requested.
case "$version" in
    ''|*[!0-9.]*) echo 'Invalid release version; expected X.Y.Z.' >&2; exit 1 ;;
esac
printf '%s\n' "$version" | LC_ALL=C grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$' || {
    echo 'Invalid release version; expected X.Y.Z.' >&2; exit 1;
}
stem="cppm-$version-linux-$arch"
archive="$stem.tar.gz"
echo "Downloading cppm $version for Linux $arch..."
download "$release_base/download/v$version/$archive" "$temporary/$archive"
download "$release_base/download/v$version/$archive.sha256" "$temporary/$archive.sha256"
# Check exactly the requested archive; never trust filenames from a checksum file.
expected=$(cut -d ' ' -f 1 "$temporary/$archive.sha256")
printf '%s\n' "$expected" | LC_ALL=C grep -Eq '^[0-9a-fA-F]{64}$' || { echo 'Invalid SHA-256 file.' >&2; exit 1; }
actual=$(sha256sum "$temporary/$archive")
actual=${actual%% *}
[ "$expected" = "$actual" ] || { echo 'SHA-256 mismatch; installation cancelled.' >&2; exit 1; }
tar -xzf "$temporary/$archive" -C "$temporary"
binary="$temporary/$stem/cppm"
[ "$("$binary" --version)" = "cppm $version" ] || { echo 'Release binary version mismatch.' >&2; exit 1; }
sh "$temporary/$stem/install.sh" --bin-dir "$bin_dir"
echo 'Use cppm update to install future releases.'
