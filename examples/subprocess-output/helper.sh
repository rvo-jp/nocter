#!/bin/sh

[ "$#" -eq 1 ] || exit 90
[ "$1" = "Nocter capture" ] || exit 91
printf 'helper stdout\n'
printf 'helper warning\n' >&2
exit 23
