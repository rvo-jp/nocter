#!/bin/bash

set -euo pipefail

script_directory="$(cd -- "$(dirname -- "$0")" && pwd -P)"
repository_root="$(cd -- "$script_directory/../.." && pwd -P)"
compiler_manifest="$repository_root/development/compiler/Cargo.toml"
version_file="$script_directory/VERSION"
release_file="$script_directory/RELEASE.json"
standard_root="$repository_root/development/std"
output_directory="${1:-$repository_root/dist}"

node "$repository_root/development/unicode/generate.js" --check

if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "arm64" ]]; then
  echo "release packaging requires an arm64-darwin host" >&2
  exit 1
fi

version="$(tr -d '\n' < "$version_file")"
archive_name="nocter-v${version}-arm64-darwin.tar.gz"

temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/nocter-package.XXXXXX")"
temporary_archive=""
cleanup() {
  rm -rf -- "$temporary_root"
  if [[ -n "$temporary_archive" && -e "$temporary_archive" ]]; then
    rm -f -- "$temporary_archive"
  fi
}
trap cleanup EXIT

tracked_list="$temporary_root/tracked-std.txt"
actual_list="$temporary_root/actual-std.txt"
(
  cd "$repository_root"
  git ls-files -- development/std | LC_ALL=C sort > "$tracked_list"
  find development/std -type f -print | LC_ALL=C sort > "$actual_list"
)
if ! cmp -s "$tracked_list" "$actual_list"; then
  echo "development/std must contain exactly its tracked regular files" >&2
  diff -u "$tracked_list" "$actual_list" >&2 || true
  exit 1
fi
if find "$standard_root" ! -type d ! -type f -print -quit | grep -q .; then
  echo "development/std may contain only regular files and directories" >&2
  exit 1
fi

echo "Building optimized compiler for $archive_name"
CARGO_TARGET_DIR="$temporary_root/cargo-target" \
  cargo build --locked --release --manifest-path "$compiler_manifest" \
    --package nocter --package nocter-content-integrity
compiler="$temporary_root/cargo-target/release/nocter"
content_integrity="$temporary_root/cargo-target/release/nocter-content-integrity"
if [[ ! -x "$compiler" ]]; then
  echo "release compiler was not produced at $compiler" >&2
  exit 1
fi
if [[ ! -x "$content_integrity" ]]; then
  echo "content-integrity tool was not produced at $content_integrity" >&2
  exit 1
fi

image_root="$temporary_root/image"
home="$image_root/.nocter"
mkdir -p "$home/std"
install -m 755 "$compiler" "$home/nocter"
install -m 644 "$version_file" "$home/VERSION"
install -m 644 "$repository_root/LICENSE" "$home/LICENSE"
install -m 644 "$repository_root/NOTICE" "$home/NOTICE"

while IFS= read -r tracked; do
  relative="${tracked#development/std/}"
  destination="$home/std/$relative"
  mkdir -p "$(dirname -- "$destination")"
  install -m 644 "$repository_root/$tracked" "$destination"
done < "$tracked_list"

compiler_digest="$("$content_integrity" file "$home/nocter")"
standard_digest="$("$content_integrity" tree "$home/std")"
node "$script_directory/render-manifest.js" \
  "$release_file" "$version_file" "$compiler_digest" "$standard_digest" "$home/MANIFEST.json"
"$home/nocter" doctor >/dev/null
"$home/nocter" check --file "$repository_root/examples/hello.nct" --offline >/dev/null

find "$image_root" -type d -exec chmod 755 {} +
find "$image_root" -type f ! -path "$home/nocter" -exec chmod 644 {} +
find "$image_root" -exec touch -t 200001010000 {} +

archive_list="$temporary_root/archive-list.txt"
(
  cd "$image_root"
  find .nocter -print | LC_ALL=C sort > "$archive_list"
)

mkdir -p "$output_directory"
output_directory="$(cd -- "$output_directory" && pwd -P)"
archive="$output_directory/$archive_name"
temporary_archive="$output_directory/.${archive_name}.tmp.$$"
COPYFILE_DISABLE=1 tar -c --no-recursion --format ustar --uid 0 --gid 0 --uname root --gname root \
  -f - -C "$image_root" -T "$archive_list" | gzip -9 -n > "$temporary_archive"
chmod 644 "$temporary_archive"
mv -f -- "$temporary_archive" "$archive"
temporary_archive=""

echo "$archive"
