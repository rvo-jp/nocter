#include "../nocter.h"
#include <stdio.h>
#include "../utils/conv.h"
#include "../builtin.h"
#include "string.h"

// IO.print(any): void
value *io_print(value *tmp, value *this) {
    char buf[256];
    string str = conv_str(buf, VAR_P[0].val);

    fwrite(str.ptr, 1, str.len, stdout);
    fputc('\n', stdout);

    return &VOID_VALUE;
}

// IO.puts(msg: String): void
value *io_puts(value *tmp, value *this) {
    string str = *VAR_P[0].val.strp;

    fwrite(str.ptr, 1, str.len, stdout);
    fputc('\n', stdout);

    return &VOID_VALUE;
}
