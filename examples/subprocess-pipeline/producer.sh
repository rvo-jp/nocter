#!/bin/sh
set -eu

i=0
while [ "$i" -lt 8192 ]; do
    printf 'pipeline\n'
    printf 'progress\n' >&2
    i=$((i + 1))
done
