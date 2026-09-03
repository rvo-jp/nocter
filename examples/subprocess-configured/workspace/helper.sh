#!/bin/sh

[ -f ./working-directory-marker ] || exit 90
[ "$MODE" = "configured" ] || exit 91
[ "${REMOVED+x}" = "" ] || exit 92
printf '%s ' "$MODE"
/bin/cat
printf 'warning\n' >&2
