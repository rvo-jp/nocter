# Recursive Text Search

This package is the v0.19.0 reference application. It recursively searches regular UTF-8 files
using only public standard-library APIs:

```sh
nocter build
./text-search NEEDLE ROOT
```

`ROOT` must name a readable directory. Symbolic links and non-regular entries are skipped; links
are never followed. A matching line is written as `relative/path:line:text`, with one-based line
numbers. Successful output is ordered by the UTF-8 byte order of each path relative to `ROOT`,
independently of filesystem enumeration order. Lines retain their text but omit the LF or CRLF
terminator recognized by `BufReader`.

The process returns 0 when at least one line matches, 1 when no line matches, and 2 for invalid
arguments or a recoverable traversal, input, or output failure. Usage and failures go to standard
error. An input failure may occur after earlier matching output has already been written.

The application retains owned paths for regular files and pending directories so it can produce
deterministic output. It closes each directory stream before descending into a child, then reads
one file at a time with a reusable line destination. It never retains a complete file. Memory is
therefore bounded by the discovered UTF-8 path data, directory traversal state, and the largest
line encountered rather than by total file contents. Successful match output passes through one
bounded `BufWriter`; normal completion explicitly flushes it, while an output failure remains a
recoverable command failure.
