#!/bin/sh
set -eu

count=0
while IFS= read -r line; do
    [ "$line" = "pipeline" ] || exit 31
    printf '%s\n' "$line"
    count=$((count + 1))
done
printf 'count=%s\n' "$count" >&2
[ "$count" -eq 8192 ] || exit 32
