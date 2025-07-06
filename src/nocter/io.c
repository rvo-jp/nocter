#include "../nocter.h"
#include <stdio.h>
#include "../utils/conv.h"
#include "../builtin.h"
#include "string.h"

// IO.print(...args: Array): void
value *io_print(value *tmp, value *this) {
    array *arrp = VAR_P[0].val.arrp;
    value *p = arrp->list;
    char buf[256];
    string str;

    for (size_t i = arrp->len; i; i --) {
        str = conv_str(buf, *p);
        fwrite(str.ptr, 1, str.len, stdout);
        fputc(' ', stdout);
        p ++;
    }
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
