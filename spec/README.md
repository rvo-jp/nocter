# Nocter Specification

This directory is the sole normative source for the current Nocter language, supported platform
boundary, command-line interface, and editor behavior. The checked standard-library source and its
documentation are owned separately by the [Standard Library](../development/std/README.md). The
catalogs below recommend a reading order; generated navigation follows the published directory
structure and does not use those links as a page registry. File names describe responsibilities
and never encode release history.

Nocter is a statically typed, value-centered systems language designed for self-contained native
executables, explicit contracts, and a small installation surface. The language favors one
canonical source style and makes machine-readable diagnostics and editor semantics part of the
public contract.

## Start Here

- [Design Principles](principles.md) explains the criteria used to resolve design choices.
- [Language Overview](overview.md) introduces the language as a coherent whole.
- [Language](language/README.md) defines source syntax and program semantics.
- [Standard Library](../development/std/README.md) defines portable public modules and APIs next to
  their checked declarations.
- [Platform](platform/README.md) defines ABI, target, primitive, and distribution boundaries.
- [Tooling](tooling/README.md) defines diagnostics, formatting, CLI, testing, and editor behavior.
- [Runnable Examples](../examples/README.md) contains complete programs checked against released
  behavior.

## Contract Status

The working tree specifies the published v0.41.0 contract. The
[release index](../releases/README.md) owns publication status, downloads, and version summaries;
repository tags preserve the exact specification for earlier releases.

A chapter states current behavior unless a section is explicitly labeled **Future Direction** or
**Non-goal**. Milestones, implementation plans, reviews, and release qualification evidence are
non-normative and live under [`development/history/`](../development/history/README.md).

## Editing Policy

- Put each public rule in exactly one owning chapter and link to it instead of restating it.
- Keep language semantics, library APIs, platform boundaries, and tooling contracts in their
  respective directories.
- Keep examples close to the rule they explain, while complete runnable packages remain under
  [`examples/`](../examples/README.md).
- Do not encode chronology or implementation phases in chapter names or current-contract prose.
- Mark proposals under an explicit **Future Direction** heading.
- Keep compiler mechanisms and repository workflows under [`development/`](../development/README.md).
