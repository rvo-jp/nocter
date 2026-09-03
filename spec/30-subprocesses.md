# Synchronous Subprocesses

**v0.30.0 contract.** The declarations and behavior in this chapter are current for the development
source tree. Published v0.29.0 artifacts do not provide `Command` or `ExitStatus`; publication
status remains owned by the [release index](../releases/README.md).

This chapter defines the first subprocess boundary in `std/process`. It deliberately starts with
synchronous execution over inherited process state. A command owns its executable-path and
argument spellings, launches exactly one child, and waits for that child to terminate.

## Public Surface

```nct
pub struct Command

pub copy struct ExitStatus

construct Command {
    pub func new(path: &str): Self!
}

instance Command {
    pub method &+self.arg(value: &str): void!
    pub method self.status(): ExitStatus!
}

instance ExitStatus {
    pub noalloc method self.success(): bool
    pub noalloc method self.code(): i32?
    pub noalloc method self.signal(): i32?
}
```

`Command.new` copies one exact executable-path spelling. The spelling must be nonempty, valid UTF-8,
and free of NUL bytes. It is interpreted relative to the child working directory when it is not
absolute. The standard library does not search `PATH`; a spelling such as `tool` therefore means
the relative path `tool`, while `./tool` names the same-directory executable explicitly.

`arg` validates and copies one exact argument. A rejected argument leaves the command unchanged.
The child receives the command path as argument zero and the added values in insertion order as
later arguments. The argument vector is terminated according to the target ABI, but the terminator
is not a source-visible argument.

`status` consumes the command, starts one child, and waits for that child to terminate. The child
inherits the parent's current working directory, environment byte vector, standard input, standard
output, and standard error as they exist at launch. Environment entries are inherited without
UTF-8 decoding or reconstruction. No shell parses the path or arguments, and no text is joined into
a command line.

The initial API has no independently owned `Child` handle. This makes waiting part of the operation
that creates the child, so ordinary source cannot accidentally discard a live child or leave a
terminated child unreaped. It also keeps process creation distinct from future pipe and
nonblocking-process contracts.

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
state returns `std.process.wait_failed`.

The launch-report channel is close-on-exec. A successful exec closes it without a payload. A failed
exec writes only the target error fact needed by the parent and then terminates without returning
to Nocter destruction, allocation, or user code. Consequently, an intentional child exit code
cannot be mistaken for an exec failure.

## Allocation and Blocking

Construction and `arg` own their copied text in the current allocation context and follow the
ordinary allocation-abort policy. Their `T!` layer reports validation, not recoverable allocation.
`status` may allocate launch metadata before creating the child and may block until termination; it
does not publish `noalloc` or a nonblocking guarantee. `ExitStatus` inspection is allocation-free.

No child is created until all target arguments, pointers, and the launch-report channel can be
prepared. After process creation, the child path performs only target operations required to close
descriptors, execute the new image, report exec failure, and terminate.

## Runnable Example

The repository [subprocess-status example](../examples/subprocess-status/index.nct) constructs an
exact `./helper.sh` command, passes one argument without command-line joining, waits, and reports
the typed nonzero exit status. Its helper is a repository-owned executable fixture rather than a
program selected through `PATH`.

## Responsibility Boundaries

`std/process` owns `Command`, argument validation and ownership, synchronous launch policy, public
errors, wait retry, and status decoding. Target-specific standard-library source owns raw process
syscall constants, close-on-exec channel layout, and one-attempt fork, exec, wait, read, and write
facts. The compiler owns only process-entry context access and generic target syscall lowering; it
does not know `Command`, `ExitStatus`, public error codes, or wait-status encoding.

The inherited environment-vector address is an immutable process-entry fact. It may cross one
private compiler-owned primitive role into trusted standard-library source so `exec` can preserve
entries byte for byte. User packages cannot access that address, and the standard library must not
reconstruct the inherited environment through the UTF-8 public query API.

## Non-goals

The v0.30.0 boundary does not add `PATH` search, shell execution, environment edits, working-
directory overrides, standard-stream redirection, pipes, output capture, an asynchronous `Child`
handle, polling, signals sent by the parent, process groups, terminal control, or another target.
Each requires a separate ownership and failure contract rather than an option added speculatively
to this synchronous operation.
