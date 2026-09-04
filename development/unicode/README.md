# Unicode Data Generation

This directory owns the reproducible transformation from the pinned Unicode Character Database to
the generated Nocter source consumed by `std/internal/unicode`. It does not define public text
semantics; those belong to [`spec/35-static-unicode-text.md`](../../spec/35-static-unicode-text.md).

`manifest.json` is the input authority. It records the Unicode version, canonical source URL, byte
length, and SHA-256 digest for every tracked file under `inputs/17.0.0/`. The generator verifies all
of those fields before parsing data and never accesses the network.

Regenerate and check the committed product from the repository root:

```sh
node development/unicode/generate.js --write
node development/unicode/generate.js --check
node development/unicode/test.js
```

Generation parses the complete pinned property and casing corpus, rejects unknown
locale-independent casing conditions, validates every scalar lookup and mapping bound, and writes
`development/std/internal/unicode/tables.nct` atomically only when its exact bytes change. Normal
compiler, standard-library, and package builds consume that committed source directly.

Updating Unicode is one reviewed change: replace all input files, update every manifest field, run
the generator, and update the public specification to name the new observable Unicode version.
