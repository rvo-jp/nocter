#!/bin/bash

set -euo pipefail

script_directory="$(cd -- "$(dirname -- "$0")" && pwd -P)"
repository_root="$(cd -- "$script_directory/../.." && pwd -P)"
version="$(tr -d '\n' < "$script_directory/VERSION")"
archive_name="nocter-v${version}-arm64-darwin.tar.gz"

if [[ -n "$(git -C "$repository_root" status --porcelain --untracked-files=all)" ]]; then
  echo "release qualification requires a clean worktree" >&2
  exit 1
fi

release_tag="v$version"
if tagged_commit="$(git -C "$repository_root" rev-parse --verify "$release_tag^{commit}" 2>/dev/null)"; then
  current_commit="$(git -C "$repository_root" rev-parse HEAD)"
  if [[ "$tagged_commit" != "$current_commit" ]]; then
    echo "release $version is already fixed at $release_tag ($tagged_commit)" >&2
    echo "refusing to qualify the same release identity from $current_commit" >&2
    exit 1
  fi
fi

temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/nocter-qualification.XXXXXX")"
cleanup() {
  rm -rf -- "$temporary_root"
}
trap cleanup EXIT

first_output="$temporary_root/first"
second_output="$temporary_root/second"
mkdir -p "$first_output" "$second_output"
"$script_directory/package-local-release.sh" "$first_output"
"$script_directory/package-local-release.sh" "$second_output"
first_archive="$first_output/$archive_name"
second_archive="$second_output/$archive_name"

if ! cmp -s "$first_archive" "$second_archive"; then
  echo "independent package generations did not produce identical archives" >&2
  exit 1
fi

first_extract="$temporary_root/first-extract"
second_extract="$temporary_root/second-extract"
mkdir -p "$first_extract" "$second_extract"
tar -xzf "$first_archive" -C "$first_extract"
tar -xzf "$second_archive" -C "$second_extract"
if ! diff -r "$first_extract" "$second_extract" >/dev/null; then
  echo "independent package generations did not produce identical homes" >&2
  diff -r "$first_extract" "$second_extract" >&2 || true
  exit 1
fi

home="$second_extract/.nocter"
mapfile_path="$temporary_root/archive-entries.txt"
tar -tzf "$second_archive" > "$mapfile_path"
if grep -Ev '^(\.nocter|\.nocter/[^/].*)/?$' "$mapfile_path" | grep -q .; then
  echo "archive must contain exactly one relative .nocter root" >&2
  exit 1
fi
if grep -E '(^/|(^|/)\.\.(/|$))' "$mapfile_path" | grep -q .; then
  echo "archive contains an unsafe path" >&2
  exit 1
fi

cmp "$script_directory/VERSION" "$home/VERSION"
for input in LICENSE NOTICE; do
  cmp "$repository_root/$input" "$home/$input"
done
if [[ "$(file -b "$home/nocter")" != *"Mach-O 64-bit executable arm64"* ]]; then
  echo "packaged compiler is not an ARM64 Mach-O executable" >&2
  exit 1
fi

before_smoke="$temporary_root/before-smoke"
cp -R "$home" "$before_smoke"
package="$temporary_root/package"
environment=(env -u NOCTER_HOME)
version_output="$("${environment[@]}" "$home/nocter" --version)"
expected_version_output="$(printf 'Nocter\nrelease: %s\nhost: arm64-darwin\ndefault target: arm64-darwin' "$version")"
if [[ "$version_output" != "$expected_version_output" ]]; then
  echo "packaged compiler reported unexpected release identity" >&2
  printf 'expected:\n%s\nactual:\n%s\n' "$expected_version_output" "$version_output" >&2
  exit 1
fi
"${environment[@]}" "$home/nocter" doctor
"${environment[@]}" "$home/nocter" --help
"${environment[@]}" "$home/nocter" init "$package" --name release-smoke
"${environment[@]}" "$home/nocter" check --root "$package" --locked --offline
"${environment[@]}" "$home/nocter" test --root "$package" --locked --offline

first_graph="$temporary_root/graph-1.json"
second_graph="$temporary_root/graph-2.json"
"${environment[@]}" "$home/nocter" graph --root "$package" --locked --offline --format json > "$first_graph"
"${environment[@]}" "$home/nocter" graph --root "$package" --locked --offline --format json > "$second_graph"
cmp "$first_graph" "$second_graph"
node -e 'const fs=require("node:fs");const value=JSON.parse(fs.readFileSync(process.argv[1],"utf8"));if(value.schema!=="nocter.package_graph"||value.version!==1)process.exit(1)' "$first_graph"

"${environment[@]}" "$home/nocter" run --root "$package" --locked --offline
executable="$temporary_root/release-smoke"
"${environment[@]}" "$home/nocter" build --root "$package" --locked --offline --output "$executable"
"$executable"
node "$script_directory/verify-lsp.js" "$home/nocter" "$package" "$version"

if ! diff -r "$before_smoke" "$home" >/dev/null; then
  echo "packaged commands mutated the installed Nocter home" >&2
  diff -r "$before_smoke" "$home" >&2 || true
  exit 1
fi

dist="$repository_root/dist"
mkdir -p "$dist"
candidate="$dist/$archive_name"
temporary_candidate="$dist/.${archive_name}.qualified.$$"
install -m 644 "$second_archive" "$temporary_candidate"
mv -f -- "$temporary_candidate" "$candidate"
rm -rf -- "$dist/.nocter"
cp -R "$home" "$dist/.nocter"
(
  cd "$dist"
  shasum -a 256 "$archive_name" > SHA256SUMS
)

digest="$(shasum -a 256 "$candidate" | awk '{print $1}')"
size="$(stat -f %z "$candidate")"
standard_files="$(find "$home/std" -type f | wc -l | tr -d ' ')"
commit="$(git -C "$repository_root" rev-parse HEAD)"
printf 'Qualified %s\ncommit: %s\nbytes: %s\nsha256: %s\nstandard-library files: %s\n' \
  "$candidate" "$commit" "$size" "$digest" "$standard_files"
