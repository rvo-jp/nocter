#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
compiler_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)

cd "$compiler_dir"

cargo check
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
