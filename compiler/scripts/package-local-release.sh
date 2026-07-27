#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
compiler_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)
repo_dir=$(CDPATH= cd -- "$compiler_dir/.." && pwd)

packaging_dir="$repo_dir/packaging"
std_source="$repo_dir/std"
home_dir="$repo_dir/dist/.nocter"

cd "$compiler_dir"

cargo build --release

rm -rf "$home_dir"
mkdir -p "$home_dir"

install -m 755 "$compiler_dir/target/release/nocter" "$home_dir/nocter"
install -m 644 "$packaging_dir/VERSION" "$home_dir/VERSION"
install -m 644 "$packaging_dir/MANIFEST.json" "$home_dir/MANIFEST.json"
cp -R "$std_source" "$home_dir/std"

"$home_dir/nocter" doctor
