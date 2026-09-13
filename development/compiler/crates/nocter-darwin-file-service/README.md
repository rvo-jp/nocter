# nocter-darwin-file-service

## Responsibility

Own executable host conformance for typed Darwin file-operation jobs and cancellation-safe file
retirement over the generic bounded blocking services.

## Contract

The adapter owns file access modes, operation inputs, initialized read output, partial write facts,
position results, and host I/O failures. Each job owns its path, byte input, or retirement-aware
file owner. It cannot retain a caller buffer. Every result that keeps a file usable returns the same
owner; dropping a queued input, ignored output, or cancelled completion sends that owner through
its pre-reserved cleanup service.

The adapter does not define public `std/io` names or errors, task scheduling, compiler primitive
identity, worker lifecycle, queue layout, or wake encoding. It composes those responsibilities
through `nocter-blocking-runtime` and `nocter-darwin-blocking-service` only.

This Rust crate is conformance evidence for the generated Darwin file adapter. Generated Nocter
executables do not link it.

## Invariants

- A retirement permit is acquired before open can produce a host file owner.
- Read output owns only its initialized prefix; write output records the exact completed prefix even
  when a later write fails.
- Seek, truncate, flush, read, and write always return the file owner before public error policy can
  inspect their operation fact.
- Job cancellation drops all immediately available ownership and never asks a caller to select a
  cleanup path.
- Explicit close uses one exact retirement identity; ordinary owner destruction uses the same
  bounded cleanup queue without retaining a completion.
- Service shutdown stops and drains operation jobs before testing retirement shutdown, so queued or
  completed job ownership cannot be hidden from cleanup accounting.
