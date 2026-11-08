#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
prefix="${HOME}/.local"
if [ "$#" -gt 0 ]; then
    if [ "$#" -ne 2 ] || [ "$1" != '--prefix' ]; then
        echo 'Usage: sh install.sh [--prefix directory]' >&2
        exit 2
    fi
    prefix=$2
fi
binary="$script_dir/cppm"
if [ ! -f "$binary" ]; then
    binary="$script_dir/../target/release/cppm"
fi
if [ ! -f "$binary" ]; then
    echo 'cppm binary not found. Extract the package or build with cargo build --release --locked.' >&2
    exit 1
fi
"$binary" --version
mkdir -p "$prefix/bin"
temporary=$(mktemp "$prefix/bin/.cppm.XXXXXX")
trap 'rm -f "$temporary"' EXIT HUP INT TERM
cp "$binary" "$temporary"
chmod 755 "$temporary"
mv -f "$temporary" "$prefix/bin/cppm"
echo "Installed in $prefix/bin/cppm"
case ":$PATH:" in
    *":$prefix/bin:"*) ;;
    *)
        echo "Add $prefix/bin to your shell's permanent PATH."
        echo 'For fish, use: fish_add_path /path/to/bin'
        ;;
esac
