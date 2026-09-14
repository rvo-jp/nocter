# Executor-Safe Filesystem Completion

This document owns the cross-responsibility design for v0.50.0 Phase 2 filesystem operations.
Public declarations and observable behavior remain in `development/std/fs`; this document fixes
how path inputs and target results cross the executor boundary without creating a second
filesystem policy.

## Owned Path Jobs

An asynchronous path operation validates each borrowed spelling through `std/path.validate`
before constructing a job. The generated constructor checks allocation-size arithmetic, appends
one terminal NUL byte per path, and copies every byte into the job's trailing owned region before
admission. A one-path job has one owned path region. Rename has two consecutive owned paths and one
runtime-contract offset naming the second path. The worker never retains an authored string or a
pointer into the future's caller.

Path mutation uses the existing bounded file-service admission, wake descriptor, state machine,
and completion record. It does not reserve file-retirement capacity because it creates no
descriptor owner. Queued cancellation destroys the owned input immediately; running abandonment
leaves the job with the service until the worker publishes and destroys its unobserved result.

Mutation is attempted exactly once. Retrying unlink, rename, directory creation, or directory
removal after an interrupted result could apply a second mutation after the first invocation
already completed. Both the generated worker and explicit blocking surface preserve the returned
target failure instead.

Metadata uses the same owned-path admission and appends aligned worker-only `stat` scratch storage
to the job. The worker retries an interrupted read-only query, decodes the target record once, and
publishes only portable classification, length, and normalized timestamp facts in the uniform
completion. Standard source cannot inspect the scratch bytes or reconstruct the Darwin layout.

## Authority Boundaries

`nocter-runtime-contract` owns the closed operation vocabulary, per-operation owned-byte shape,
and job layout. `nocter-target-program` owns each primitive's checked source signature. The
standard profile owns the sole mapping from a semantic primitive role to its physical declaration.
ARM64 selects that role and consumes the runtime layout; it does not infer operations from source
names. `std/path` owns UTF-8/NUL validation, `std/internal/io` maps the uniform completion, and
`std/fs` owns public operation policy.

The host Darwin service is lifecycle conformance, not another public implementation. Its typed
payload variants prove that ownerless path jobs use the same bounded admission and cancellation
rules as file-owner jobs without exposing queue records to the generated target or standard
source.

## Remaining Result Families

Directory acquisition requires an owner with infallible retirement. Canonicalization, link
reading, and directory steps require owned byte outputs with explicit initialized lengths. These
shapes extend the service contract rather than placing target layouts or caller pointers into
`std/fs`.

Recursive creation, removal, and walking compose the one-entry operations. They must use an
explicit bounded stack or stream owner and document partial mutation on failure; recursion cannot
accumulate an implicit unbounded call stack or materialize an entire tree before publication.
