#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
bin_dir="${HOME}/.local/bin"
if [ "$#" -gt 0 ]; then
    if [ "$#" -ne 2 ]; then
        echo 'Usage: sh install.sh [--prefix directory | --bin-dir directory]' >&2
        exit 2
    fi
    case "$1" in
        --prefix) bin_dir="$2/bin" ;;
        --bin-dir) bin_dir=$2 ;;
        *) echo 'Expected --prefix or --bin-dir.' >&2; exit 2 ;;
    esac
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
mkdir -p "$bin_dir"
temporary=$(mktemp "$bin_dir/.cppm.XXXXXX")
trap 'rm -f "$temporary"' EXIT HUP INT TERM
cp "$binary" "$temporary"
chmod 755 "$temporary"
mv -f "$temporary" "$bin_dir/cppm"
echo "Installed in $bin_dir/cppm"
case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *)
        echo "Add $bin_dir to your shell's permanent PATH."
        echo 'For fish, use: fish_add_path /path/to/bin'
        ;;
esac
