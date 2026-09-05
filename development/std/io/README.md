# I/O

Every `Writer` receives the line adapter declared by the compiler-checked
[`Writer` contract](index.nct), in addition to exact text output.

`write_line` writes the complete input text followed by exactly one LF byte. It does not select a
platform newline, allocate a combined buffer, or promise that both writes are atomic. A failure may
be reported after an observable prefix. It inherits buffering and explicit-flush behavior from the
selected writer.

The process stream conveniences are symmetric; their exact declarations are owned by
[`index.nct`](index.nct).

`print` and `println` write to standard output; `eprint` and `eprintln` write to standard error. The
line forms append exactly one LF, including for empty input. These functions do not buffer, flush,
format arbitrary values, or allocate through a Nocter allocation context. They use the same
descriptor-write authority as `File.write`, including interruption retry, complete-write looping,
zero-progress rejection, and stable I/O errors. Formatting remains explicit through interpolation,
for example `io.println("count: ${count}")?`.

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
use std/io/buffer.BufReader

var input = BufReader.new(io.stdin())
let line = input.read_line()?
```

Each `BufReader` owns its unread bytes and line state. Creating two buffered wrappers for the same
process descriptor therefore creates two independent consumers; the library does not coordinate
their buffered state. EOF, CR/LF removal, UTF-8 validation, allocation failure, and terminal-state
behavior remain exactly the common `BufReader` contract.


## Byte I/O and Buffering

`Reader` and `Writer` define the shared byte-I/O contracts. `Reader.read` initializes no more than
the supplied buffer length and returns zero at end of stream. The `read_to_end` default method
collects bytes into independently owned `Vec<u8>` storage. A reader that reports an impossible byte
count fails with `std.io.invalid_read_count`. The `read_to_string` default method uses the same
collector and validates the complete result as UTF-8 before returning an independently owned
`String`.

`Writer.write_text` is a default adapter from UTF-8 text to the complete-byte `write` contract.
`BufReader` and `BufWriter` in `std/io/buffer` own their buffering storage and receive these common
operations through static interface dispatch. A buffered writer reports I/O failure only through
an explicit `flush` or `close`; dropping it discards unflushed bytes because destruction cannot
return an error. Successful flush clears the buffer only after the underlying write succeeds. A
failed flush makes the writer terminal because the destination may already have accepted an
unreported prefix; retrying the complete retained buffer could duplicate output. Explicit close
also makes the wrapper terminal, and later write or flush operations fail with `std.io.closed`.
A requested writer capacity of zero is normalized to one byte.

`BufReader` additionally exposes the line-oriented text operations declared in the
[`std/io/buffer` contract](buffer/index.nct).

`read_line` returns an owned string, `none` only when end of stream is reached before another byte,
and an error through the outer `!` layer. `read_line_into` clears `destination` before observing the
stream, writes the next line into the same allocation when its retained capacity is sufficient,
and returns `true`; it returns `false` with an empty destination for the same clean end-of-stream
condition. An empty line therefore returns an empty present `String` or `true`, not end of stream.

Line results exclude the terminating LF byte. One CR byte immediately before that LF is also
excluded; a lone CR and every other byte are retained. EOF after line bytes returns that final
unterminated line once. Repeated line reads after EOF or explicit `close` return `none` or `false`,
and byte reads through `Reader` return zero.

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
