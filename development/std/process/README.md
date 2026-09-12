# Child Processes and Standard Streams

This chapter defines child-process ownership and standard-stream behavior in `std/process`. A
command owns its executable path, arguments, configuration, and finite input. It can execute as a
closed asynchronous or blocking operation, or transfer one child and its configured stream
endpoints to the caller.

The compiler-checked [public declarations](index.nct) are the sole authority for exact signatures.

`Command.new` copies one exact executable-path spelling. The spelling must be nonempty, valid UTF-8,
and free of NUL bytes. It is interpreted relative to the child working directory when it is not
absolute. The standard library does not search `PATH`; a spelling such as `tool` therefore means
the relative path `tool`, while `./tool` names the same-directory executable explicitly.

`arg` validates and copies one exact argument. A rejected argument leaves the command unchanged.
The child receives the command path as argument zero and the added values in insertion order as
later arguments. The argument vector is terminated according to the target ABI, but the terminator
is not a source-visible argument.

`status` and `status_blocking` consume the command, start one child, and wait for that child to
terminate. Unless command configuration replaces a value, the child inherits the parent's current
working directory, environment byte vector, standard input, standard output, and standard error as
they exist at launch. Environment entries are inherited without UTF-8 decoding or reconstruction.
No shell parses the path or arguments, and no text is joined into a command line. The unqualified
name is executor-safe; the `_blocking` name is the explicit synchronous twin.

`spawn` and `spawn_blocking` consume a command and a `ProcessIo` policy. Each returns one owning
`Child` only after the close-on-exec report proves successful executable replacement. `spawn`
awaits that report through descriptor readiness without blocking the executor; `spawn_blocking`
waits through the synchronous descriptor adapter.

`ProcessIo.inherit` selects inherited descriptors for all streams. `ProcessIo.piped` selects three
directional pipes. The `stdin`, `stdout`, and `stderr` mutators replace those choices independently
with `Stdio.inherit`, `Stdio.null`, or `Stdio.pipe`. Null streams connect to the target null device;
they do not create a parent endpoint.

A returned `Child` uniquely owns one process lifecycle record and every configured endpoint not
yet transferred. Each `take_*` method transfers its endpoint at most once. Destroying the child
closes untaken endpoints, terminates an unobserved process, and transfers its sole reaping
obligation to the target cleanup service without synchronously waiting. Calling `wait` or
`wait_blocking` also closes untaken endpoints before observing the process, so a child cannot
remain blocked on output that its parent elected not to read.

`try_wait` observes only that exact child without waiting. It returns `none` while the child is
still running and caches an ordinary or signal terminal status before returning it. Repeated
`try_wait` calls and a later `wait` or `wait_blocking` return the cached terminal state without a
second kernel observation. An observation failure leaves the owner live so consuming wait or
destruction still retains the cleanup obligation. An interrupted nonwaiting observation is retried;
it does not become a spurious `none`. The shared wait classifier also requires the returned child
identity to match the exact owned child before decoding terminal status.

`terminate` requests the target's graceful termination action and `kill` requests forced
termination. Neither method consumes the child, closes an endpoint, waits, or reaps. A caller may
continue transferring pipe data and must still call a wait operation when it requires the final
status; otherwise destruction transfers cleanup as usual. Calling either method after cached
terminal observation succeeds without another target request.

`ChildStdin` implements `Writer` and `BlockingWriter`; `ChildStdout` and `ChildStderr` implement
`Reader` and `BlockingReader`. Both execution surfaces share one endpoint ownership state and one
read/write result classifier. The asynchronous methods suspend on compiler-owned descriptor
readiness and never drive a blocking syscall. Closing an endpoint is idempotent.

## Exit Status

`ExitStatus` represents one observed terminal state. `success` is true exactly when `code` is
present and equal to zero. `code` contains the target-reported ordinary exit code and returns
`none` when a signal terminated the child. `signal` contains the terminating signal number and
returns `none` after ordinary exit. Exactly one of `code` and `signal` is present.

The standard library waits only for the child it created. An interrupted wait is retried. Stopped
or continued states are not returned as terminal statuses.

## Failure Boundary

Invalid source text is rejected before process creation with `std.process.invalid_input`. Failure
to create the private launch-report channel or child returns `std.process.spawn_failed`. If the
child cannot execute the requested path, the parent receives that failure through the private
channel and returns one of:

- `std.process.not_found` when the executable path does not resolve;
- `std.process.permission_denied` when execution is denied;
- `std.process.invalid_input` when the target rejects the executable or argument representation;
- `std.process.spawn_failed` for another launch failure.

A nonzero child exit and signal termination are successful observations represented by
`ExitStatus`; they are not `T!` failures. Failure to observe the already-created child's terminal
state returns `std.process.wait_failed`. Failure to request graceful or forced termination returns
`std.process.termination_failed`.

The launch-report channel is close-on-exec. A successful exec closes it without a payload. A failed
exec writes only the target error fact needed by the parent and then terminates without returning
to Nocter destruction, allocation, or user code. Consequently, an intentional child exit code
cannot be mistaken for an exec failure.

## Allocation and Blocking

Construction and `arg` own their copied text in the current allocation context and follow the
ordinary allocation-abort policy. Their `T!` layer reports validation, not recoverable allocation.
All launch operations may allocate metadata before creating the child and do not publish
`noalloc`. `spawn`, `status`, and `output` are executor-safe. `spawn_blocking`, `status_blocking`,
and `output_blocking` may synchronously wait and therefore carry `blocking`. Endpoint close and
transfer are allocation-free and nonblocking. `try_wait`, `terminate`, and `kill` are
allocation-free and do not synchronously wait; their target operations may still fail.
Asynchronous endpoint I/O and `Child.wait` are executor-safe. `ExitStatus` inspection is
allocation-free and nonblocking.

No child is created until all target arguments, pointers, and the launch-report channel can be
prepared. After process creation, the child path performs only target operations required to close
descriptors, execute the new image, report exec failure, and terminate.

## Runnable Example

The repository [subprocess-status example](../../../examples/subprocess-status/index.nct)
constructs an exact `./helper.sh` command, passes one argument without command-line joining, calls
the explicit blocking twin, and reports the typed nonzero exit status. Its helper is a
repository-owned executable fixture rather than a program selected through `PATH`.

The [subprocess-pipeline example](../../../examples/subprocess-pipeline/index.nct) connects two
children through generic executor-safe `io.copy`, concurrently captures the producer's diagnostic
stream and both consumer output streams, and observes both children under one finite timeout. Both
producer streams exceed ordinary pipe capacity, so the example qualifies structured progress and
cleanup through public contracts rather than a process-specific transfer loop.

## Responsibility Boundaries

`std/process` owns `Command`, `ProcessIo`, child and endpoint states, argument validation and
ownership, launch policy, public errors, wait retry, and status decoding. Closed descriptor,
process-launch, and process-observation primitives are source-private to this module; another
standard module cannot invoke them with unproved ownership or reinterpret their target facts.
The compiler owns process-entry context access, semantic reactor interests, and selected target
operations. It does not know `Command`, `Child`, `Stdio`, public error codes, or wait-status
encoding.

The inherited environment-vector address is an immutable process-entry fact. It may cross one
private compiler-owned primitive role into trusted standard-library source so `exec` can preserve
entries byte for byte. User packages cannot access that address, and the standard library must not
reconstruct the inherited environment through the UTF-8 public query API.

## Output Capture

The owning closed operations can capture standard output and standard error simultaneously without
exposing an independently owned child process.

The `Output` fields and both output-operation signatures are defined only by the
[public declarations](index.nct).

`output` and `output_blocking` consume the command, start exactly one child, capture that child's
standard output and standard error, wait for the child to terminate, and return both complete byte
streams with the terminal status. Standard input, the current working directory, and the
environment remain inherited exactly as for the corresponding status operation. The unqualified
name is executor-safe; `_blocking` identifies the synchronous twin.

Captured streams contain arbitrary bytes. They are not required to be UTF-8 and are therefore
represented by `Vec<u8>`. A caller that requires text validates it explicitly, for example with
`String.from_utf8(&output.stdout)`. Each vector preserves the byte order written to its descriptor.
No ordering relationship between standard output and standard error is defined.

`Output` is an ordinary owning value. Its public fields let a caller inspect the copyable status,
borrow either stream, or move the captured buffers without a process resource remaining live.

### Concurrent Drain Contract

The parent must observe both captured descriptors while the child can still run. It must not read
one stream to completion before servicing the other: a child may fill either finite pipe buffer
while waiting for the parent to drain it. The asynchronous operation gives stdin, stdout, stderr,
and child observation independent structured computations under one parent future. The blocking
twin uses the sole three-direction readiness coordinator and gives every ready direction one
bounded operation per observation. Neither surface defines a cross-stream merge order.

A readiness notification does not itself mean end of stream. The parent reads a bounded chunk from
a ready descriptor and considers that descriptor complete only after a read returns end of file.
Hangup may accompany unread bytes and does not discard them. Interrupted readiness or read
operations are retried without starving the other stream.

Capture ends after both descriptors reach end of file and the created child has been observed in a
terminal state. If a descendant inherits either descriptor, that descendant can delay end of file
until it closes the inherited copy. This is the ordinary operating-system pipe-lifetime rule, not
an implicit descendant-discovery protocol.

### Launch and Failure Boundary

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

Invalid command input retains the errors defined by the basic
[Failure Boundary](#failure-boundary). Child setup and output-pipe
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

### Allocation and Blocking

Both output operations may allocate without a source-visible upper bound while collecting either
stream. They use the ordinary current allocation context and follow the standard allocation-abort
policy; `T!` reports process and I/O failures rather than recoverable allocation exhaustion.

`output` suspends on descriptor and process interests without blocking an executor thread.
`output_blocking` may block until the child terminates and every inherited captured descriptor
closes, and explicitly carries `blocking`.

### Responsibility Boundaries

`std/process` owns public capture semantics, buffer ownership, failure precedence, and both closed
lifecycles. Its target-independent command representation does not know readiness record layouts
or syscall numbers.

Target-specific standard source owns descriptor-normalization policy, redirection and readiness
records, and one-attempt result classification. The selected target owns syscall identities and
native calling conventions behind exact primitive roles. One shared private pipe abstraction owns
close-on-exec creation and descriptor lifetime; output capture must not construct a second pipe
protocol beside the launch-report implementation.

The compiler exposes closed target-operation roles and immutable process-entry facts. It
does not know `Output`, distinguish standard output from standard error, decode readiness events,
or choose public process failures.

## Command Configuration

A command can select its working directory, construct an exact child environment, and provide
finite standard-input bytes while retaining the closed asynchronous and blocking lifecycles.

The exact configuration operations are defined only by the
[compiler-checked `Command` declarations](index.nct).

Configuration mutates only the request. It does not create a child. Calling `current_dir`, `env`,
or `input` again replaces the previous value. Replacement becomes visible only after the complete
new value has been prepared. `clear_env` also discards earlier explicit environment changes;
subsequent `env` calls build an exact environment from empty state.

If `input` is never called, the child inherits standard input. Calling it with an empty view is
different: the child receives a pipe that reaches end of file without any bytes. Both status
operations continue to inherit standard output and standard error. Both output operations continue
to capture both streams.

### Working Directory

`current_dir` accepts one nonempty valid UTF-8 path without a NUL byte and copies it into the
command. Invalid source input returns `std.process.invalid_input` without changing the earlier
configuration.

The child changes directory after installing configured standard descriptors and before executable
replacement. A relative executable spelling is therefore resolved from the configured child
directory. Failure to enter the directory returns `std.process.current_directory_failed` and
cannot be mistaken for executable rejection or child exit.

### Environment Construction

An environment name must be nonempty valid UTF-8 and contain neither `=` nor NUL. A value must be
valid UTF-8 and contain no NUL; it may be empty and may contain `=`. Invalid input returns
`std.process.invalid_input` without partially changing the command.

Unless `clear_env` was called, untouched inherited entries cross the child boundary byte for byte,
including entries that the public UTF-8 query API cannot decode. Setting or removing a name removes
every inherited entry with that exact byte name. The last explicit operation for a name wins, and
the prepared child vector contains at most one entry for that name. Environment-vector order is
not public behavior.

All owned `name=value` storage and the terminating pointer vector are prepared before child
creation. The child does not allocate, validate text, query the public environment API, or rebuild
the environment after `fork`.

### Finite Input and Concurrent Output

Configured input is copied when `input` is called and remains owned by the command until execution.
The parent closes the input pipe after writing every byte. If the child closes its read side early,
the parent treats the resulting broken pipe as the child's decision not to consume the remaining
input; terminal status and captured output remain observable.

When an output operation and configured input are combined, stdin writes and stdout/stderr reads
must progress concurrently. Writing all input before reading either output stream, or reading one
output stream to completion before servicing the others, is forbidden because finite pipes can
deadlock in either direction. Structured tasks provide asynchronous fairness; the sole blocking
pipe-group coordinator provides synchronous fairness.

The implementation suppresses process-wide `SIGPIPE` termination only for its owned input writer.
It must not change the calling process's global signal disposition. Interrupted operations are
retried. A non-broken-pipe write failure returns `std.process.input_failed` after both output pipes
are drained or closed and the exact child is observed.

### Launch and Failure Precedence

The private close-on-exec launch report distinguishes input-descriptor setup, captured-output setup,
working-directory rejection, and executable rejection. No stage reserves or interprets an ordinary
child exit code.

Every successfully created child is observed exactly once. When more than one internal failure is
present, public selection uses this order:

1. failure to observe the exact child;
2. a reported child setup or executable-replacement failure;
3. captured-output readiness or read failure;
4. finite-input write failure other than broken pipe.

This order prevents an earlier transport failure from hiding loss of child ownership and preserves
the specific cause of a rejected launch.

### Allocation and Blocking

Configuration copies use the current allocation context and follow the ordinary allocation-abort
policy. The `T!` results report invalid process text or operating-system failures, not recoverable
allocation exhaustion. Configuration methods do not publish `noalloc`.

The `_blocking` terminal operations may block until the child terminates. `output_blocking` may
additionally wait for descendants that inherited a captured output descriptor. The unqualified
operations suspend through the executor instead. Finite input creates neither a public size limit
nor a separate progress API.

### Responsibility Boundaries

`std/process` owns command configuration, prepared `argv` and environment storage, stream policy,
failure precedence, and the complete create-and-reap lifecycle. Target-specific standard-library
source owns child setup policy, readiness records, no-SIGPIPE requirements, and native result
classification. The selected target owns the exact `chdir` and descriptor-installation operations.

One shared launch path creates public `Child` and endpoint owners for spawn and closed operations.
Asynchronous closed operations compose ordinary endpoint methods and exact observation with
structured tasks. Only the blocking output twin owns a pipe-group readiness coordinator, and that
coordinator cannot launch or observe a process. The compiler exposes closed target-operation roles
and immutable process-entry facts; it does not know command configuration, environment edits, pipe
direction, closed-operation composition, or public process errors.

## Process Context

`std/process.arg_count`, `arg`, `environment_count`, `environment`, and `env` publish `noalloc`
while querying process-lifetime storage. Their exact types remain owned by the
[public declarations](index.nct).

Out-of-range indexed queries return `none`; invalid process encoding returns `error`. `args`
remains the allocating convenience that collects all arguments.

## Non-goals

This contract does not add `PATH` search, shell parsing, caller-provided descriptors, merged output,
capture or input size limits, process groups, terminal control, arbitrary signals, or another
target. Process timeouts remain later v0.49.0 work.
