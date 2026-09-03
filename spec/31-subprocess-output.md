# Captured Subprocess Output

**v0.31.0 standard-library contract.** The declarations and behavior in this chapter are available
in published v0.31.0 artifacts. Publication status remains owned by the
[release index](../releases/README.md).

This chapter extends the owning synchronous operation in `std/process` with simultaneous standard-
output and standard-error capture. It does not add an independently owned child process.

## Public Surface

```nct
pub struct Output {
    pub status: ExitStatus
    pub stdout: Vec<u8>
    pub stderr: Vec<u8>
}

instance Command {
    pub method self.output(): Output!
}
```

`output` consumes the command, starts exactly one child, captures that child's standard output and
standard error, waits for the child to terminate, and returns both complete byte streams with the
terminal status. Standard input, the current working directory, and the environment remain
inherited exactly as for `Command.status`.

Captured streams contain arbitrary bytes. They are not required to be UTF-8 and are therefore
represented by `Vec<u8>`. A caller that requires text validates it explicitly, for example with
`String.from_utf8(&output.stdout)`. Each vector preserves the byte order written to its descriptor.
No ordering relationship between standard output and standard error is defined.

`Output` is an ordinary owning value. Its public fields let a caller inspect the copyable status,
borrow either stream, or move the captured buffers without a process resource remaining live.

## Concurrent Drain Contract

The parent must observe both captured descriptors while the child can still run. It must not read
one stream to completion before servicing the other: a child may fill either finite pipe buffer
while waiting for the parent to drain it. When both descriptors are readable, the implementation
services them in a deterministic order but promises no cross-stream merge order.

A readiness notification does not itself mean end of stream. The parent reads a bounded chunk from
a ready descriptor and considers that descriptor complete only after a read returns end of file.
Hangup may accompany unread bytes and does not discard them. Interrupted readiness or read
operations are retried without starving the other stream.

Capture ends after both descriptors reach end of file and the created child has been observed in a
terminal state. If a descendant inherits either descriptor, that descendant can delay end of file
until it closes the inherited copy. This is the ordinary operating-system pipe-lifetime rule, not
an implicit descendant-discovery protocol.

## Launch and Failure Boundary

The private close-on-exec launch report distinguishes three outcomes without reserving a child exit
code:

- successful replacement of the child image, reported by clean channel closure;
- failure while installing captured standard descriptors, reported with its setup stage and target
  error number;
- rejection of the executable image, reported with its exec stage and target error number.

All pipes and capture storage needed before launch are prepared before the child is created. Raw
descriptors are normalized away from standard input, standard output, and standard error before
fork, so a parent with a previously closed standard descriptor cannot make redirection overwrite a
different live pipe endpoint.

Invalid command input retains the errors defined by
[Synchronous Subprocesses](30-subprocesses.md#failure-boundary). Child setup and output-pipe
creation failures return `std.process.capture_failed`. Executable rejection retains its existing
specific process error. A readiness or read failure also returns `std.process.capture_failed`.
Failure to observe the created child's terminal state returns `std.process.wait_failed`.

No capture, launch-report, or executable-rejection path may return before attempting to wait for
the exact child created by the operation. After a capture failure, the parent closes both captured
read descriptors first so the child cannot remain blocked writing to an abandoned pipe. A kernel
failure from the exact-child wait remains the only condition under which the library cannot prove
that terminal observation completed.

Nonzero exit and signal termination remain successful observations represented by `ExitStatus`.
Captured bytes written before either terminal condition are returned normally.

## Allocation and Blocking

`output` may allocate without a source-visible upper bound while collecting either stream. It uses
the ordinary current allocation context and follows the standard allocation-abort policy; `T!`
reports process and I/O failures rather than recoverable allocation exhaustion.

The operation may block until the child terminates and every inherited captured descriptor closes.
It publishes neither `noalloc` nor a nonblocking guarantee.

## Responsibility Boundaries

`std/process` owns public capture semantics, buffer ownership, failure precedence, and the complete
synchronous lifecycle. Its target-independent command representation does not know readiness
record layouts or syscall numbers.

Target-specific standard source owns descriptor normalization, redirection constants, readiness
record layout, syscall selection, and one-attempt raw transitions. One shared private pipe
abstraction owns close-on-exec creation and descriptor lifetime; output capture must not construct a
second pipe protocol beside the launch-report implementation.

The compiler continues to expose only generic syscall roles and immutable process-entry facts. It
does not know `Output`, distinguish standard output from standard error, decode readiness events,
or choose public process failures.

## Non-goals

The v0.31.0 boundary does not add an asynchronous `Child`, incremental stream access, input
redirection, caller-provided descriptors, merged output, a capture-size limit, a timeout, `PATH`
search, shell execution, environment edits, working-directory overrides, parent-sent signals, or
another target.
