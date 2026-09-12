# I/O

Every `BlockingWriter` receives the line adapter declared by the compiler-checked
[`BlockingWriter` contract](index.nct), in addition to exact text output.

`write_line_blocking` writes the complete input text followed by exactly one LF byte. It does not
select a platform newline, allocate a combined buffer, or promise that both writes are atomic. A
failure may be reported after an observable prefix. It inherits buffering and explicit-flush
behavior from the selected writer.

The process stream conveniences are symmetric; their exact declarations are owned by
[`index.nct`](index.nct).

`print` and `println` write to standard output; `eprint` and `eprintln` write to standard error. The
line forms append exactly one LF, including for empty input. These functions do not buffer, flush,
format arbitrary values, or allocate through a Nocter allocation context. They use the same
descriptor-write authority as `File.write_blocking`, including interruption retry,
complete-write looping, zero-progress rejection, and stable I/O errors. All four functions are
`blocking`; `noalloc` remains an independent allocation guarantee. Formatting remains explicit
through interpolation, for example `io.println("count: ${count}")?`.

## Standard Input

`std/io` exposes the inherited process input through the same byte-reader contract used by files.
Its exact declaration is owned by [`index.nct`](index.nct).

`stdin` returns a non-owning `File` wrapper around the process standard-input descriptor. Closing
or dropping that value makes only that wrapper terminal; it does not close the process-global
descriptor and does not affect a separately acquired wrapper. `stdin` itself does not allocate,
read, wait, or validate UTF-8. Reads may block and report the same stable I/O failures as an opened
file.

The standard library does not publish a stateless `io.read_line` convenience. Such a function
could read beyond one line and either lose bytes or require hidden process-global buffering. Line
input instead uses the existing owning buffer explicitly:

```nct
use std/io
use std/io/buffer.BlockingBufReader

var input = BlockingBufReader.new(io.stdin())
let line = input.read_line_blocking()?
```

The containing callable must admit `blocking`, because `read_line_blocking` may wait for input.

Each `BlockingBufReader` owns its unread bytes and line state. Creating two buffered wrappers for
the same process descriptor therefore creates two independent consumers; the library does not
coordinate their buffered state. EOF, CR/LF removal, UTF-8 validation, allocation failure, and
terminal-state behavior remain exactly the common `BlockingBufReader` contract.


## Byte I/O and Buffering

`Reader` and `Writer` are the canonical executor-safe byte-stream contracts. Calling one of their
methods creates a lazy future; driving it cannot synchronously wait for external progress.
`Reader.read` initializes no more than the supplied mutable byte view and returns zero at EOF.
Its default `read_to_end` owns one scratch buffer and one independently owned result, rejects an
impossible byte count with `std.io.invalid_read_count`, and destroys both correctly on failure or
cancellation. `read_to_string` reuses that collector and validates the complete result as UTF-8.

`Writer.write` accepts the complete supplied byte view or reports failure after any already-written
prefix remains observable. The default `flush` has no retained state. `write_text` forwards the
UTF-8 bytes and `write_line` follows them with exactly one LF operation; neither constructs a
combined buffer. Transport-specific deadlines and configured timeouts are deliberately absent from
these interfaces because their meaning is not portable across a generic byte stream.

`copy` transfers from any `Reader` to any `Writer` until the reader returns end of stream;
`copy_blocking` provides the same policy for `BlockingReader` and `BlockingWriter`. Both allocate
one 8-KiB scratch buffer in the current allocation context, validate every implementation-reported
read count, write exactly the initialized prefix, and return the checked total number of bytes.
They do not flush the destination. Allocation follows the ordinary terminating allocation policy;
`T!` reports source, destination, invalid-count, or `std.io.byte_count_overflow` failure. A writer
failure may leave its already-observed prefix visible. Cancellation of the asynchronous operation
destroys the scratch buffer and leaves reader and writer usability to their own cancellation
contracts; the transfer loop owns neither stream.

`BufReader<R>` and `BufWriter<W>` in `std/io/buffer` are the owning generic adapters for these
executor-safe contracts. `BufReader.read` returns buffered progress without waiting for a second
chunk; it awaits the underlying `Reader` only when no accepted byte remains. Line operations retain
an incomplete line across suspension and return the same newline-normalized UTF-8 results as the
blocking surface. A cancelled line operation leaves its accepted prefix inside the reader. A later
line operation resumes that prefix, while a later byte read returns the prefix before consuming
newer bytes. The reader keeps one initialized byte allocation and publishes only the prefix covered
by separately committed logical length. A cancelled refill cannot update that length, so scratch
initialization cannot become received input. Whether the underlying reader itself remains usable
after cancellation is determined by that reader's own contract.

`BufWriter.write` accepts bytes into private bounded storage and flushes full chunks through its
underlying `Writer`. `flush` always propagates through the underlying writer, including when the
outer buffer is empty. It makes the wrapper terminal before awaiting output because cancellation or
failure may follow an externally visible prefix whose length is unknown. A successful write and
underlying flush restore the open state; cancellation or failure leaves later write and flush
operations reporting `std.io.closed`. `finish` performs the same fallible flush, consumes the
buffer, and returns the underlying writer. Dropping an unfinished buffer discards retained bytes.

`BlockingReader` and `BlockingWriter` define the shared byte-I/O contracts.
`BlockingReader.read_blocking` initializes no more than the supplied buffer length and returns zero
at end of stream. The
`read_to_end_blocking` default method collects bytes into independently owned `Vec<u8>` storage. A
reader that reports an impossible byte count fails with `std.io.invalid_read_count`. The
`read_to_string_blocking` default method uses the same collector and validates the complete result
as UTF-8 before returning an independently owned `String`.

Every `BlockingReader` and `BlockingWriter` operation admits `blocking`. A generic algorithm using
either interface retains that contract even when one concrete implementation happens to operate only on
memory. Construction of `stdin`, `stdout`, and `stderr`, and explicit `File.close`, do not wait and
remain unqualified.

`BlockingWriter.write_text_blocking` is a default adapter from UTF-8 text to the complete-byte
`write_blocking` contract.
`BlockingBufReader<R>` and `BlockingBufWriter<W>` in `std/io/buffer` own their buffering storage
and receive these common operations through static interface dispatch. They accept any underlying
type that implements the matching blocking interface; neither representation depends on `File`.
A buffered writer reports I/O failure only through an explicit `flush_blocking` or `finish`;
dropping it discards unflushed bytes because destruction cannot return an error. Successful flush
clears the buffer only after the underlying write succeeds. A failed flush makes the writer
terminal because the destination may already have accepted an unreported prefix; retrying the
complete retained buffer could duplicate output. Explicit close of the underlying stream is
performed after a successful `finish` returns that stream; the generic buffer does not invent a
close capability. Later write or flush operations after failure report `std.io.closed`.
A requested writer capacity of zero is normalized to one byte.

`BlockingBufReader` additionally exposes the line-oriented text operations declared in the
[`std/io/buffer` contract](buffer/index.nct).

`read_line_blocking` returns an owned string, `none` only when end of stream is reached before
another byte, and an error through the outer `!` layer. `read_line_into_blocking` clears
`destination` before observing the
stream, writes the next line into the same allocation when its retained capacity is sufficient,
and returns `true`; it returns `false` with an empty destination for the same clean end-of-stream
condition. An empty line therefore returns an empty present `String` or `true`, not end of stream.

Line results exclude the terminating LF byte. One CR byte immediately before that LF is also
excluded; a lone CR and every other byte are retained. EOF after line bytes returns that final
unterminated line once. Repeated line reads after EOF return `none` or `false`, and byte reads
through `BlockingReader.read_blocking` return zero. Consuming `finish` instead discards unread
buffered input and returns the underlying reader.

UTF-8 validation applies to the complete line after newline removal, so one scalar may cross any
number of partial underlying reads. Invalid input fails with `std.string.invalid_utf8`; it is not
replaced or lossily converted. The reusable destination is empty on invalid UTF-8, underlying read
failure, or recoverable allocation failure. Any such line-step failure terminates the buffered
reader because bytes may already have been consumed from its source. Later operations observe the
terminal state instead of retrying an ambiguous partial line.

The buffered reader retains its fixed read buffer and one reusable raw line buffer. It retains no
earlier completed line and never collects the complete file. Memory is bounded by the configured
read-buffer capacity plus the largest line observed and the caller's retained destination
capacity. A requested read-buffer capacity of zero is normalized to one byte so refill always
makes progress. Underlying interrupted reads retain the ordinary `File` retry behavior before a
line operation observes failure.
