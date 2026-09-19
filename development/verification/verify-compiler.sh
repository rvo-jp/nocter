#!/bin/bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: development/verification/verify-compiler.sh [--fast|--full]

  --fast  Run the change-feedback gate without the long-running end-to-end suites.
  --full  Run the complete compiler gate (default).
EOF
}

verification_tier="full"
case "${1-}" in
  "") ;;
  --fast) verification_tier="fast" ;;
  --full) ;;
  --help|-h)
    usage
    exit 0
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac

if [[ "$#" -gt 1 ]]; then
  usage >&2
  exit 2
fi

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

available_test_threads="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 1)"
case "$available_test_threads" in
  ''|*[!0-9]*) available_test_threads=1 ;;
esac
if [[ "$available_test_threads" -lt 1 ]]; then
  test_threads=1
elif [[ "$available_test_threads" -gt 4 ]]; then
  test_threads=4
else
  test_threads="$available_test_threads"
fi

node "$repository_root/development/verification/verify-repository-metadata.js"
node "$repository_root/development/benchmarks/test.js"
node "$repository_root/development/unicode/test.js"
node "$repository_root/development/unicode/generate.js" --check
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets \
  --exclude nocter \
  --exclude nocter-command \
  --exclude nocter-language-server \
  --exclude nocter-native-session

if [[ "$verification_tier" == "full" ]]; then
  cargo test --locked -p nocter --all-targets
  cargo test --locked -p nocter-command --all-targets
  cargo test --locked -p nocter-native-session --all-targets
  cargo test --locked -p nocter-language-server --all-targets -- --test-threads="$test_threads"
  cargo check --locked --workspace --no-default-features
  RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps
fi

echo "Compiler $verification_tier verification passed in disposable target: $CARGO_TARGET_DIR"
