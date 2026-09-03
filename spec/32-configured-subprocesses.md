# Configured Synchronous Subprocesses

**v0.32.0 standard-library contract.** The declarations and behavior in this chapter are available
in published v0.32.0 artifacts. Publication status remains owned by the
[release index](../releases/README.md).

The completed operation lets one owning command select its working directory, construct an
exact child environment, and provide finite standard-input bytes while retaining the closed
`status` and `output` lifecycles. It will not introduce an independently owned child process.

## Public Surface

The existing `Command` surface will gain:

```nct
instance Command {
    /// Sets the working directory used before executable resolution.
    pub method &+self.current_dir(path: &str): void!

    /// Sets one child environment value, replacing an equal name.
    pub method &+self.env(name: &str, value: &str): void!

    /// Removes one child environment name if it is inherited or explicitly set.
    pub method &+self.remove_env(name: &str): void!

    /// Removes every inherited and explicitly configured environment entry.
    pub method &+self.clear_env(): void

    /// Copies finite bytes that will replace inherited standard input.
    pub method &+self.input(value: &[u8]): void
}
```

Configuration mutates only the request. It does not create a child. Calling `current_dir`, `env`,
or `input` again replaces the previous value. Replacement becomes visible only after the complete
new value has been prepared. `clear_env` also discards earlier explicit environment changes;
subsequent `env` calls build an exact environment from empty state.

If `input` is never called, the child inherits standard input. Calling it with an empty view is
different: the child receives a pipe that reaches end of file without any bytes. `status` continues
to inherit standard output and standard error. `output` continues to capture both streams.

## Working Directory

`current_dir` accepts one nonempty valid UTF-8 path without a NUL byte and copies it into the
command. Invalid source input returns `std.process.invalid_input` without changing the earlier
configuration.

The child changes directory after installing configured standard descriptors and before executable
replacement. A relative executable spelling is therefore resolved from the configured child
directory. Failure to enter the directory returns `std.process.current_directory_failed` and
cannot be mistaken for executable rejection or child exit.

## Environment Construction

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

## Finite Input and Concurrent Output

Configured input is copied when `input` is called and remains owned by the command until execution.
The parent closes the input pipe after writing every byte. If the child closes its read side early,
the parent treats the resulting broken pipe as the child's decision not to consume the remaining
input; terminal status and captured output remain observable.

When `output` and configured input are combined, stdin writes and stdout/stderr reads must progress
within one readiness loop. Writing all input before reading either output stream, or reading one
output stream to completion before servicing the others, is forbidden because finite pipes can
deadlock in either direction. Each ready direction receives one bounded operation before another
poll so no stream can starve the others.

The implementation suppresses process-wide `SIGPIPE` termination only for its owned input writer.
It must not change the calling process's global signal disposition. Interrupted operations are
retried. A non-broken-pipe write failure returns `std.process.input_failed` after both output pipes
are drained or closed and the exact child is observed.

## Launch and Failure Precedence

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

## Allocation and Blocking

Configuration copies use the current allocation context and follow the ordinary allocation-abort
policy. The `T!` results report invalid process text or operating-system failures, not recoverable
allocation exhaustion. The new configuration methods do not publish `noalloc`.

Both terminal operations may block until the child terminates. `output` may additionally wait for
descendants that inherited a captured output descriptor. Finite input does not create a public
size limit or an asynchronous progress API.

## Responsibility Boundaries

`std/process` owns command configuration, prepared `argv` and environment storage, stream policy,
failure precedence, and the complete create-and-reap lifecycle. Target-specific standard-library
source owns raw `chdir`, descriptor installation, readiness records, no-SIGPIPE descriptor setup,
and syscall classification.

One command-I/O session owns every configured pipe and all three direction states. It replaces the
capture-only two-descriptor session rather than creating a second polling and cleanup authority.
The compiler continues to expose only generic syscall roles and immutable process-entry facts; it
does not know command configuration, environment edits, pipe direction, or public process errors.

## Non-goals

v0.32.0 does not add `PATH` search, shell parsing, asynchronous children, incremental stream access,
caller-provided descriptors, merged output, capture or input size limits, timeouts, parent-sent
signals, process groups, terminal control, or another target.
