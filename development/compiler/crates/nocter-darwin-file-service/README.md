# nocter-darwin-file-service

## Responsibility

Own executable host conformance for typed Darwin file-operation jobs and cancellation-safe file
retirement over the generic bounded blocking services.

## Contract

The adapter owns file access modes, operation inputs, initialized read output, partial write facts,
position results, and target-level I/O failures. Each job owns its path, byte input, or
retirement-aware file owner. It cannot retain a caller buffer. Every result that keeps a file usable
returns the same owner; dropping a queued input, ignored output, or cancelled completion sends that
owner through its pre-reserved cleanup service.

Worker results contain `DarwinFileFailure` and `DarwinFileWriteFact` from the runtime contract,
never `std::io::Error`. Host APIs are reduced once to positive Darwin errno or a closed adapter
failure before publication. Generated target code therefore consumes the same raw fact vocabulary
instead of reconstructing a parallel error representation.

The adapter does not define public `std/io` names or errors, task scheduling, compiler primitive
identity, worker lifecycle, queue layout, or wake encoding. It composes lifecycle execution through
`nocter-blocking-runtime` and `nocter-darwin-blocking-service`, and consumes the same closed
file-operation vocabulary that generated target code receives from `nocter-runtime-contract`.

This Rust crate is conformance evidence for the generated Darwin file adapter. Generated Nocter
executables do not link it.

## Invariants

- A retirement permit is acquired before open can produce a host file owner.
- Exclusive creation either publishes one new owner or leaves every existing path byte unchanged.
- Read output owns only its initialized prefix; write output records the exact completed prefix even
  when a later write fails.
- Positioned read and write preserve the shared cursor while retaining the same initialized-prefix
  and partial-progress rules as their cursor-changing counterparts.
- Seek, truncate, durable synchronization, read, and write always return the file owner before
  public error policy can inspect their operation fact.
- Job cancellation drops all immediately available ownership and never asks a caller to select a
  cleanup path.
- Explicit close uses one exact retirement identity; ordinary owner destruction uses the same
  bounded cleanup queue without retaining a completion.
- Service shutdown stops and drains operation jobs before testing retirement shutdown, so queued or
  completed job ownership cannot be hidden from cleanup accounting.
