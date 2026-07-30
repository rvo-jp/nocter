#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
compiler_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)
development_dir=$(CDPATH= cd -- "$compiler_dir/.." && pwd)
repo_dir=$(CDPATH= cd -- "$development_dir/.." && pwd)

packaging_dir="$development_dir/packaging"
std_source="$development_dir/std"
dist_dir="$repo_dir/dist"
home_dir="$repo_dir/dist/.nocter"
version=$(sed -n '1p' "$packaging_dir/VERSION")
archive_name="nocter-v${version}-arm64-darwin.tar.gz"
archive_path="$dist_dir/$archive_name"

cd "$compiler_dir"

cargo build --release

rm -rf "$home_dir"
mkdir -p "$home_dir"

install -m 755 "$compiler_dir/target/release/nocter" "$home_dir/nocter"
install -m 644 "$packaging_dir/VERSION" "$home_dir/VERSION"
install -m 644 "$packaging_dir/MANIFEST.json" "$home_dir/MANIFEST.json"
install -m 644 "$repo_dir/LICENSE" "$home_dir/LICENSE"
install -m 644 "$repo_dir/NOTICE" "$home_dir/NOTICE"
cp -R "$std_source" "$home_dir/std"

"$home_dir/nocter" doctor

rm -f "$archive_path"
(
    cd "$dist_dir"
    tar -czf "$archive_name" .nocter
)

printf 'Wrote %s\n' "$home_dir"
printf 'Wrote %s\n' "$archive_path"
