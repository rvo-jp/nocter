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

for source in "$repository_root"/examples/*.nct; do
  "${environment[@]}" "$home/nocter" check --file "$source" --offline
done
for example in "$repository_root"/examples/*; do
  if [[ -d "$example" && -f "$example/index.nct" ]]; then
    "${environment[@]}" "$home/nocter" check --root "$example" --locked --offline
  fi
done

text_banner_stdout="$temporary_root/text-banner.stdout"
text_banner_stderr="$temporary_root/text-banner.stderr"
expected_text_banner_stdout="$temporary_root/text-banner.expected.stdout"
expected_text_banner_stderr="$temporary_root/text-banner.expected.stderr"
text_banner_executable="$temporary_root/text-banner"
printf '==========\ntext: alpha-beta\nbytes: 10\n==========\n' > "$expected_text_banner_stdout"
: > "$expected_text_banner_stderr"
"${environment[@]}" "$home/nocter" build \
  --root "$repository_root/examples/text-banner" \
  --locked \
  --offline \
  --output "$text_banner_executable"
"$text_banner_executable" '  alpha beta  ' \
  > "$text_banner_stdout" \
  2> "$text_banner_stderr"
cmp "$expected_text_banner_stdout" "$text_banner_stdout"
cmp "$expected_text_banner_stderr" "$text_banner_stderr"

stdin_prefix_stdout="$temporary_root/stdin-prefix.stdout"
stdin_prefix_stderr="$temporary_root/stdin-prefix.stderr"
expected_stdin_prefix_stderr="$temporary_root/stdin-prefix.expected.stderr"
: > "$expected_stdin_prefix_stderr"
"${environment[@]}" "$home/nocter" run \
  --root "$repository_root/examples/stdin-prefix" \
  --locked \
  --offline \
  -- '> ' \
  < "$repository_root/examples/stdin-prefix/sample.txt" \
  > "$stdin_prefix_stdout" \
  2> "$stdin_prefix_stderr"
cmp "$repository_root/examples/stdin-prefix/sample-output.txt" "$stdin_prefix_stdout"
cmp "$expected_stdin_prefix_stderr" "$stdin_prefix_stderr"

subprocess_status_stdout="$temporary_root/subprocess-status.stdout"
subprocess_status_stderr="$temporary_root/subprocess-status.stderr"
expected_subprocess_status_stdout="$temporary_root/subprocess-status.expected.stdout"
expected_subprocess_status_stderr="$temporary_root/subprocess-status.expected.stderr"
printf 'helper exited with code 17\n' > "$expected_subprocess_status_stdout"
: > "$expected_subprocess_status_stderr"
"${environment[@]}" "$home/nocter" run \
  --root "$repository_root/examples/subprocess-status" \
  --locked \
  --offline \
  > "$subprocess_status_stdout" \
  2> "$subprocess_status_stderr"
cmp "$expected_subprocess_status_stdout" "$subprocess_status_stdout"
cmp "$expected_subprocess_status_stderr" "$subprocess_status_stderr"

subprocess_output_stdout="$temporary_root/subprocess-output.stdout"
subprocess_output_stderr="$temporary_root/subprocess-output.stderr"
expected_subprocess_output_stderr="$temporary_root/subprocess-output.expected.stderr"
: > "$expected_subprocess_output_stderr"
"${environment[@]}" "$home/nocter" run \
  --root "$repository_root/examples/subprocess-output" \
  --locked \
  --offline \
  > "$subprocess_output_stdout" \
  2> "$subprocess_output_stderr"
cmp "$repository_root/examples/subprocess-output/sample-output.txt" "$subprocess_output_stdout"
cmp "$expected_subprocess_output_stderr" "$subprocess_output_stderr"

subprocess_configured_stdout="$temporary_root/subprocess-configured.stdout"
subprocess_configured_stderr="$temporary_root/subprocess-configured.stderr"
expected_subprocess_configured_stderr="$temporary_root/subprocess-configured.expected.stderr"
: > "$expected_subprocess_configured_stderr"
(
  cd "$repository_root/examples/subprocess-configured"
  "${environment[@]}" "$home/nocter" run \
    --root . \
    --locked \
    --offline \
    > "$subprocess_configured_stdout" \
    2> "$subprocess_configured_stderr"
)
cmp "$repository_root/examples/subprocess-configured/sample-output.txt" \
  "$subprocess_configured_stdout"
cmp "$expected_subprocess_configured_stderr" "$subprocess_configured_stderr"

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

tampered_compiler_home="$temporary_root/tampered-compiler-home"
cp -R "$home" "$tampered_compiler_home"
printf '\0' >> "$tampered_compiler_home/nocter"
if env NOCTER_HOME="$tampered_compiler_home" "$home/nocter" doctor >/dev/null 2>&1; then
  echo "installed compiler content changed without invalidating the home" >&2
  exit 1
fi

tampered_standard_home="$temporary_root/tampered-standard-home"
cp -R "$home" "$tampered_standard_home"
printf '\n' >> "$tampered_standard_home/std/index.nct"
if env NOCTER_HOME="$tampered_standard_home" "$home/nocter" doctor >/dev/null 2>&1; then
  echo "installed standard-library content changed without invalidating the home" >&2
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
