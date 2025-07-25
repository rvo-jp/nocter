#include "../nocter.h"
#include "../utils/alloc.h"
#include "../builtin.h"
#include "../interpretor.h"
#include "fpconv/fpconv.h"

bool string_equal(const string *a, const string *b) {
    if (a == b) return true;
    if (a->len != b->len) return false;
    return memcmp(a->ptr, b->ptr, a->len) == 0;
}

size_t ast_to_charp(ast expr, char *buf) {
    if (expr.expr_cmd == expr_ident) {
        size_t len = strlen(expr.chld.ptr);
        memcpy(buf, expr.chld.ptr, len);
        return len;
    }

    if (expr.expr_cmd == expr_dot) {
        size_t len = ast_to_charp(expr.chld.idastp->expr, buf);
        buf[len] = '.';
        size_t len2 = strlen(expr.chld.idastp->id);
        memcpy(buf + len + 1, expr.chld.idastp->id, len2);

        return len + 1 + len2;
    }

    // else if (expr.expr_cmd == expr_val) {
    //     c = conv_str(c, *(vl *)expr.dat);
    // }
    else printf("@ debug: ast_to_charp %lld\n", (uintptr_t)expr.expr_cmd), exit(1);
}

size_t long_to_charp(long i, char *buf) {
    static const char digit_table[200] = {
        '0', '0', '0', '1', '0', '2', '0', '3', '0', '4', '0', '5', '0', '6', '0', '7', '0', '8', '0', '9',
        '1', '0', '1', '1', '1', '2', '1', '3', '1', '4', '1', '5', '1', '6', '1', '7', '1', '8', '1', '9',
        '2', '0', '2', '1', '2', '2', '2', '3', '2', '4', '2', '5', '2', '6', '2', '7', '2', '8', '2', '9',
        '3', '0', '3', '1', '3', '2', '3', '3', '3', '4', '3', '5', '3', '6', '3', '7', '3', '8', '3', '9',
        '4', '0', '4', '1', '4', '2', '4', '3', '4', '4', '4', '5', '4', '6', '4', '7', '4', '8', '4', '9',
        '5', '0', '5', '1', '5', '2', '5', '3', '5', '4', '5', '5', '5', '6', '5', '7', '5', '8', '5', '9',
        '6', '0', '6', '1', '6', '2', '6', '3', '6', '4', '6', '5', '6', '6', '6', '7', '6', '8', '6', '9',
        '7', '0', '7', '1', '7', '2', '7', '3', '7', '4', '7', '5', '7', '6', '7', '7', '7', '8', '7', '9',
        '8', '0', '8', '1', '8', '2', '8', '3', '8', '4', '8', '5', '8', '6', '8', '7', '8', '8', '8', '9',
        '9', '0', '9', '1', '9', '2', '9', '3', '9', '4', '9', '5', '9', '6', '9', '7', '9', '8', '9', '9'
    };

    bool neg = i < 0;
    uint64_t u = neg? -i : i;

    char s[32], *last = s + 32, *p = last;

    while (u >= 100) {
        const uint64_t old = u;
        u /= 100;
        const uint32_t r = (uint32_t)(old - u * 100);
        *-- p = digit_table[2 * r + 1];
        *-- p = digit_table[2 * r];
    }
    if (u >= 10) {
        *-- p = digit_table[2 * u + 1];
        *-- p = digit_table[2 * u];
    }
    else {
        *-- p = '0' + (char)u;
    }

    if (neg) *-- p = '-';

    size_t len = last - p;
    memcpy(buf, p, len);
    return len;
}

string conv_str(char *buff, value val) {
    if (val.type == &STRING_OBJ) return *val.strp;
    
    if (val.type == &INT_OBJ) {
        size_t len = long_to_charp(val.bit, buff);
        buff[len] = '\0';

        return (string){
            .ptr = buff,
            .len = len
        };
    }

    if (val.type == &FLOAT_OBJ) {
        int len = fpconv_dtoa(val.db, buff);
        buff[len] = '\0';

        return (string){
            .ptr = buff,
            .len = len
        };
    }

    if (val.type == &BOOL_OBJ) {
        return val.bit ? (string){"true", 4} : (string){"false", 5};
    }

    if (val.type == NULL) {
        return val.bit ? (string){"", 0} : (string){"null", 4};
    }

    if (val.type == &ARRAY_OBJ) {
        puts("@ error: conv_str: array"), exit(1);
    }
    
    if (val.type == &FUNC_OBJ) {
        puts("@ error: conv_str: function"), exit(1);
    }
    
    puts("@ error: conv_str: other"), exit(1);
}

// String.init(any): String
value *string_init(value *tmp, value *this) {
    value *val = &VAR_P[0].val;
    if (val->type == &STRING_OBJ) return val;
    if (val->type == &INT_OBJ) {
        char buf[32];
        size_t len = long_to_charp(val->bit, buf);
        buf[len] = '\0';

        *tmp = (value){
            .type = &STRING_OBJ,
            .strp = stringdup((string){
                .ptr = buf,
                .len = len
            })
        };
        return tmp;
    }
}

// String.length(): Int
value *string_length(value *tmp, value *this) {
    *tmp = (value){
        .type = &INT_OBJ,
        .bit = this->strp->len
    };
    return tmp;
}

// String.replaceAll(old: String, new: String): String
value *string_replace_all(value *tmp, value *this) {
    string src = *this->strp;
    string old = *VAR_P[-1].val.strp;
    string new = *VAR_P[0].val.strp;

    if (old.len == 0 || src.len < old.len) return this;

    size_t count = 0;
    for (char *p = src.ptr, *limit = src.ptr + src.len - old.len; p <= limit;) {
        if (memcmp(p, old.ptr, old.len) == 0) {
            count ++;
            p += old.len;
        }
        else p ++;
    }
    
    if (count == 0) return this;

    string res;
    res.len = src.len + count * (new.len - old.len);
    res.ptr = alloc(res.len + 1);
    res.ptr[res.len] = '\0';

    char *p = src.ptr, *buf = res.ptr;
    while (count) {
        if (memcmp(p, old.ptr, old.len) == 0) {
            memcpy(buf, new.ptr, new.len);
            p += old.len;
            buf += new.len;
            count --;
        }
        else *buf ++ = *p ++;
    }
    for (char *limit = src.ptr + src.len; p < limit;) *buf ++ = *p ++;
    *buf = '\0';

    *tmp = (value){
        .type = &STRING_OBJ,
        .strp = stringdup(res)
    };
    return tmp;
}


