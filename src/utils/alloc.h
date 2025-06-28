#ifndef UTILS_ALLOC_H
#define UTILS_ALLOC_H

#include "../nocter.h"
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

static inline void *alloc(size_t size) {
    void *mem = malloc(size);
    if (__builtin_expect(mem == NULL, 0)) {
        fputs("\e[1;31mfatal error:\e[0;1m Memory allocation failed\e[0m\n", stderr);
        exit(1);
    }
    return mem;
}

static inline void *allocs(void *ptr, size_t size) {
    void *mem = realloc(ptr, size);
    if (mem == NULL) {
        puts("\e[1;31mfatal error:\e[0;1m Memory allocation failed\e[0m");
        exit(1);
    }
    return mem;
}

static inline void *allocpy(void *ptr, size_t size) {
    void *mem = alloc(size);
    memcpy(mem, ptr, size);
    return mem;
}


static inline idast *idastdup(const idast expr) {
    idast *mem = alloc(sizeof(idast));
    *mem = expr;
    return mem;
}

static inline ast *astdup(const ast expr) {
    ast *mem = alloc(sizeof(ast));
    *mem = expr;
    return mem;
}

static inline object *objdup(const object expr) {
    object *mem = alloc(sizeof(object));
    *mem = expr;
    return mem;
}

static inline dbexpr *dbexprdup(const dbexpr db) {
    dbexpr *mem = alloc(sizeof(dbexpr));
    *mem = db;
    return mem;
}

static inline trexpr *trexprdup(const trexpr tr) {
    trexpr *mem = alloc(sizeof(trexpr));
    *mem = tr;
    return mem;
}

static inline value *valuedup(const value val) {
    value *mem = alloc(sizeof(value));
    *mem = val;
    return mem;
}

static inline func *funcdup(const func fn) {
    func *mem = alloc(sizeof(func));
    *mem = fn;
    return mem;
}

static inline callexpr *calldup(const callexpr fn) {
    callexpr *mem = alloc(sizeof(callexpr));
    *mem = fn;
    return mem;
}

static inline array *arrdup(const array fn) {
    array *mem = alloc(sizeof(array));
    *mem = fn;
    return mem;
}

static inline string *stringdup(const string str) {
    string *mem = alloc(sizeof(string));
    *mem = str;
    return mem;
}

#endif