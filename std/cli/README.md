# Command-Line Applications

The compiler-checked [`std/cli` contract](index.nct) is the sole authority for exact public
declarations. This guide defines the schema, parsing, result, presentation, and process-adapter
behavior shared by those declarations.

## Responsibility Boundary

`std/process` remains the sole source of operating-system arguments, their order, and UTF-8
decoding failures. `std/cli` receives already separated text arguments and owns only command-line
grammar. It never reads a shell command string, performs shell expansion, prints output, or
terminates the process.

An `Application` owns copied schema text. A `ParsedArguments` owns every accepted option and
positional value, so queries never read process state or parse token spelling again. The same
application schema drives parsing, compact usage, and complete help generation.

## Grammar

- `--name` selects one long flag or begins one long value option.
- `--name=value` supplies a value in the same argument; the value may be empty.
- `-x` selects one exact short flag or value option.
- A short value option consumes the following argument.
- A long value option without `=` consumes the following argument.
- `--` ends option recognition; every later argument is positional.
- `-` is always positional.
- Combined short forms such as `-abc` are rejected rather than interpreted differently according
  to the registered flags.
- A consumed option value is data even when it begins with `-`.

Option names and short names are unique within one application. Positionals are assigned from left
to right. Required positionals must precede optional positionals. A repeated option retains every
value in command-line order; every other duplicate option is rejected. Unknown options, absent
option values, missing required positionals, and surplus positionals are failures with stable
`std.cli.*` codes.

One schema level may contain positionals or named subcommands, but not both. A command token selects
one owned child schema, and every remaining token is classified by that child. Child schemas may
own further commands, so nesting does not require another parser or representation. Options before
the command belong to the parent; options after it belong to the selected child. A schema that
defines commands requires one to be selected.

## Presentation

`usage` and `help` render only the retained schema. They perform no process access and no output.
Applications may write the returned `String` through `std/io`, attach additional context, or use it
in tests. No implicit help, version, color, terminal detection, or exit policy exists.

## Non-goals

The contract does not add shell parsing, environment fallback, configuration files,
automatic value conversion, reflection, derive-like declarations, compiler-known CLI metadata,
terminal styling, or completion scripts.
