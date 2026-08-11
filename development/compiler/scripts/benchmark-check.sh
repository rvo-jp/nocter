#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
compiler_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)
development_dir=$(CDPATH= cd -- "$compiler_dir/.." && pwd)
repository_dir=$(CDPATH= cd -- "$development_dir/.." && pwd)

profile=${NOCTER_BENCHMARK_PROFILE:-release}
runs=${NOCTER_BENCHMARK_RUNS:-3}
source=${1:-"$repository_dir/examples/hello.nct"}

case "$profile" in
    dev)
        cargo_profile_args=
        profile_dir=debug
        ;;
    release)
        cargo_profile_args=--release
        profile_dir=release
        ;;
    *)
        echo "NOCTER_BENCHMARK_PROFILE must be dev or release" >&2
        exit 2
        ;;
esac

case "$runs" in
    ''|*[!0-9]*|0)
        echo "NOCTER_BENCHMARK_RUNS must be a positive integer" >&2
        exit 2
        ;;
esac

results=$(mktemp "${TMPDIR:-/tmp}/nocter-check-benchmark.XXXXXX")
trap 'rm -f "$results"' EXIT HUP INT TERM

cd "$compiler_dir"
# shellcheck disable=SC2086
cargo build $cargo_profile_args

compiler="$compiler_dir/target/$profile_dir/nocter"
run=1
while [ "$run" -le "$runs" ]; do
    echo "run $run/$runs" >&2
    NOCTER_HOME="$development_dir" NOCTER_INTERNAL_TIMINGS=1 \
        "$compiler" check "$source" 2>>"$results"
    run=$((run + 1))
done

cat "$results"
awk '
    /"phase":"command.total"/ {
        value = $0
        sub(/^.*"elapsed_us":/, "", value)
        sub(/,.*$/, "", value)
        print value
    }
' "$results" | sort -n | awk '
    { values[NR] = $1 }
    END {
        if (NR == 0) {
            print "missing command.total timing event" > "/dev/stderr"
            exit 1
        }
        if (NR % 2 == 1) {
            median = values[(NR + 1) / 2]
        } else {
            median = (values[NR / 2] + values[NR / 2 + 1]) / 2
        }
        printf "median command.total: %.3f s (%d runs)\n", median / 1000000, NR
    }
'
