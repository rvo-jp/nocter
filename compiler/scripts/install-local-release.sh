#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
compiler_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)
repo_dir=$(CDPATH= cd -- "$compiler_dir/.." && pwd)

cd "$compiler_dir"

cargo build --release

mkdir -p "$repo_dir/.nocter"
install -m 755 "$compiler_dir/target/release/nocter" "$repo_dir/.nocter/nocter"

"$repo_dir/.nocter/nocter" doctor
