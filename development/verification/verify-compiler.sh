#!/bin/bash

set -euo pipefail

script_directory="$(cd -- "$(dirname -- "$0")" && pwd -P)"
repository_root="$(cd -- "$script_directory/../.." && pwd -P)"
compiler_root="$repository_root/development/compiler"
temporary_root="$(mktemp -d /tmp/nocter-compiler-verification.XXXXXX)"

cleanup() {
  case "$temporary_root" in
    /tmp/nocter-compiler-verification.*)
      rm -rf -- "$temporary_root"
      ;;
    *)
      echo "refusing to remove unexpected verification path: $temporary_root" >&2
      return 1
      ;;
  esac
}
trap cleanup EXIT

export CARGO_TARGET_DIR="$temporary_root/target"
cd "$compiler_root"

node "$repository_root/development/verification/verify-repository-metadata.js"
node "$repository_root/development/unicode/test.js"
node "$repository_root/development/unicode/generate.js" --check
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
cargo check --locked --workspace --no-default-features
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps

echo "Compiler verification passed in disposable target: $CARGO_TARGET_DIR"
