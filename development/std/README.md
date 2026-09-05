# Standard Library

This directory is the sole authority for the portable public modules distributed with Nocter.
Every module `index.nct` owns its compiler-checked public declarations and declaration doc
comments. A colocated README owns longer module behavior that would obscure the declaration list.
The two forms must not repeat signatures: declarations answer what can be called, while prose
explains observable behavior, failure, complexity, and cross-operation invariants.
The package identity itself is defined by the checked [root declaration](index.nct).

Language constructs and target primitives are referenced from their owning specification chapters
rather than redefined here. Generated contract pages show the exact checked `index.nct` source:
only declarations with public language visibility are public API, while visible `see` edges and
restricted declarations remain source-level module assembly and package contracts. Independent
package-only modules live under `std/internal` and are excluded from generated navigation. The
guides below cover modules whose observable behavior needs more explanation than declaration
comments.

## Behavior Guides

- [Borrowed Text](str/README.md)
- [Owned Strings](string/README.md)
- [Vectors](vec/README.md)
- [Slices](slice/README.md)
- [Iteration](iter/README.md)
- [Formatting](fmt/README.md)
- [I/O](io/README.md)
- [Filesystem](fs/README.md)
- [Integer Text](num/README.md)
- [Allocation and Failure](mem/README.md)
- [Recoverable Errors](error/README.md)
- [Pointer and Address Conversion](ptr/README.md)
- [Associative Collections](map/README.md)
- [JSON Values and Text](json/README.md)
- [Monotonic Time](time/README.md)
- [Synchronous Processes](process/README.md), including output capture and command configuration
- [Unicode Text and Scalars](char/README.md)
- [Native Assertions](testing/README.md)
