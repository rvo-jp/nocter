#ifndef STRING_NOCTER_H
#define STRING_NOCTER_H

#include "../nocter.h"
#include "fpconv/fpconv.h"
#define double_to_charp fpconv_dtoa

bool string_equal(const string *a, const string *b);

size_t ast_to_charp(ast expr, char *buf);
size_t long_to_charp(long i, char *buf);
string conv_str(char *buff, value val);

// String.length(): Int
value *string_length(value *tmp, value *this);

#endif