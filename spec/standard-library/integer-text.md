# Integer Text

Every built-in integer owns the same decimal text surface:

```nct
construct i8 { pub noalloc func parse(text: &str): Self? }
construct i16 { pub noalloc func parse(text: &str): Self? }
construct i32 { pub noalloc func parse(text: &str): Self? }
construct i64 { pub noalloc func parse(text: &str): Self? }
construct isize { pub noalloc func parse(text: &str): Self? }
construct u8 { pub noalloc func parse(text: &str): Self? }
construct u16 { pub noalloc func parse(text: &str): Self? }
construct u32 { pub noalloc func parse(text: &str): Self? }
construct u64 { pub noalloc func parse(text: &str): Self? }
construct usize { pub noalloc func parse(text: &str): Self? }

instance i8 {
    pub method self.to_string(): String
    pub method self.try_to_string(allocator: &+TryAllocator): String! from allocator
}
```

The `i8` instance shape above applies identically to the other nine integer types. `parse` is
allocation-free and consumes the complete input. Unsigned types accept one or more ASCII digits.
Signed types additionally accept exactly one leading `-`. Empty input, a leading `+`, whitespace,
non-ASCII digits, another character, and a mathematical value outside the destination range return
`none`. Leading zeroes are valid, and negative zero produces zero.

`to_string` produces the shortest ordinary base-ten spelling, with `0` as the sole zero spelling
and a leading `-` only for a negative signed value. It uses the current allocation context and
aborts on allocation failure. `try_to_string` uses the supplied recoverable allocator and returns
its allocation failure. These operations and `Format` must use one decimal-generation authority;
parsing must scan an input once through one signed or unsigned decimal authority.

There are no type-named free-function aliases. This contract does not add floating-point values,
arbitrary radix parsing, locale rules, or a matrix of public integer-to-integer conversions.
