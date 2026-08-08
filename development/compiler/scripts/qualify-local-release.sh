#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
compiler_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)
development_dir=$(CDPATH= cd -- "$compiler_dir/.." && pwd)
repo_dir=$(CDPATH= cd -- "$development_dir/.." && pwd)
packaging_dir="$development_dir/packaging"
package_script="$script_dir/package-local-release.sh"

version=$(sed -n '1p' "$packaging_dir/VERSION")
archive_name="nocter-v${version}-arm64-darwin.tar.gz"
archive_path="$repo_dir/dist/$archive_name"
qualification_dir=$(mktemp -d)
trap 'rm -rf "$qualification_dir"' EXIT HUP INT TERM

first_archive="$qualification_dir/first.tar.gz"
first_extract="$qualification_dir/first"
second_extract="$qualification_dir/second"
package_root="$qualification_dir/package"
graph_one="$qualification_dir/graph-one.json"
graph_two="$qualification_dir/graph-two.json"
executable="$qualification_dir/fresh-app"
lsp_output="$qualification_dir/lsp.out"

mkdir -p "$first_extract" "$second_extract" "$package_root"

"$package_script"
cp "$archive_path" "$first_archive"
tar -xzf "$first_archive" -C "$first_extract"

"$package_script"
tar -xzf "$archive_path" -C "$second_extract"

if ! tar -tzf "$archive_path" | awk -F/ 'NF > 0 && $1 != ".nocter" { exit 1 }'; then
    printf 'archive contains an entry outside .nocter/\n' >&2
    exit 1
fi

diff -qr "$first_extract/.nocter" "$second_extract/.nocter"

home="$second_extract/.nocter"
compiler="$home/nocter"

cmp "$home/VERSION" "$packaging_dir/VERSION"
cmp "$home/MANIFEST.json" "$packaging_dir/MANIFEST.json"
cmp "$home/LICENSE" "$repo_dir/LICENSE"
cmp "$home/NOTICE" "$repo_dir/NOTICE"

env -u NOCTER_HOME "$compiler" --version | grep -F "Nocter $version"
env -u NOCTER_HOME "$compiler" doctor
env -u NOCTER_HOME "$compiler" --help >/dev/null

(
    cd "$package_root"
    env -u NOCTER_HOME "$compiler" init --name fresh-app
    env -u NOCTER_HOME "$compiler" check --locked --offline
    env -u NOCTER_HOME "$compiler" test --locked --offline
    env -u NOCTER_HOME "$compiler" graph --locked --offline --format json >"$graph_one"
    env -u NOCTER_HOME "$compiler" graph --locked --offline --format json >"$graph_two"
    cmp "$graph_one" "$graph_two"
    env -u NOCTER_HOME "$compiler" run --locked --offline
    env -u NOCTER_HOME "$compiler" build --executable fresh-app --locked --offline -o "$executable"
)

file "$executable" | grep -F 'Mach-O 64-bit executable arm64'
"$executable"

write_frame() {
    body=$1
    length=$(printf %s "$body" | LC_ALL=C wc -c | tr -d ' ')
    printf 'Content-Length: %s\r\n\r\n%s' "$length" "$body"
}

{
    write_frame '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}'
    write_frame '{"jsonrpc":"2.0","method":"initialized","params":{}}'
    write_frame '{"jsonrpc":"2.0","id":2,"method":"shutdown","params":null}'
    write_frame '{"jsonrpc":"2.0","method":"exit","params":null}'
} | env -u NOCTER_HOME "$compiler" lsp >"$lsp_output"

grep -F "\"version\":\"$version\"" "$lsp_output" >/dev/null
grep -F '"id":2,"result":null' "$lsp_output" >/dev/null

archive_size=$(wc -c <"$archive_path" | tr -d ' ')
archive_sha=$(shasum -a 256 "$archive_path" | awk '{ print $1 }')
std_count=$(find "$home/std" -type f | wc -l | tr -d ' ')

printf 'Qualified %s\n' "$archive_path"
printf 'Version: %s\n' "$version"
printf 'Bytes: %s\n' "$archive_size"
printf 'SHA-256: %s\n' "$archive_sha"
printf 'Standard-library files: %s\n' "$std_count"
