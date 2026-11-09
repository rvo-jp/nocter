# Nocter Specification

This directory is the normative source for the published Nocter language, supported platform
boundary, command-line interface, and editor behavior. The checked standard-library source and its
documentation are owned separately by the [Standard Library](../std/README.md). The catalogs below recommend a reading order; each chapter owns one subject.

Nocter is a statically typed, value-centered systems language designed for self-contained native
executables, explicit contracts, and a small installation surface. The language favors one
canonical source style and makes machine-readable diagnostics and editor semantics part of the
public contract.

## Start Here

- [Design Principles](principles.md) explains the criteria used to resolve design choices.
- [Language Overview](overview.md) introduces the language as a coherent whole.
- [Language](language/README.md) defines source syntax and program semantics.
- [Standard Library](../std/README.md) defines portable public modules and APIs next to
  their checked declarations.
- [Platform](platform/README.md) defines ABI, target, primitive, and distribution boundaries.
- [Tooling](tooling/README.md) defines diagnostics, formatting, CLI, testing, and editor behavior.
- [Runnable Examples](../examples/README.md) contains complete programs checked against released
  behavior.

## Contract Status

The working tree specifies the published v0.72.0 contract. The
[release index](../releases/README.md) owns publication status, downloads, and version summaries;
repository tags preserve the exact specification for earlier releases.

Every chapter describes this published contract, including its supported scope and non-goals.
Unreleased development contracts and proposals are maintained separately from these user-facing
pages. Examples and standard-library links describe the same released product.
