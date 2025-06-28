#ifndef NOCTER_H
#define NOCTER_H

#include <stdlib.h>
#include <stdbool.h>
#include <string.h>
#include <stdio.h>

#define NOCTER_BUFF 256
#define NOCTER_LINE_MAX 128


typedef struct string string;
typedef struct object object;
typedef struct field field;
typedef struct func func;
typedef struct array array;

typedef struct value {
    object *type;
    union {
        // void *ptr;
        string *strp;   // string
        func *funcp;    // func
        object *objp;   // object
        field *fldp;    // field
        array *arrp;    // array
        FILE *filep;    // file pointer
        long bit;       // int, bool
        double db;      // float
    };
} value;

#define VOID 0
#define RETURN 1
#define BREAK 2
typedef struct statement {
    uint8_t type;
    value *valp;
} statement;

typedef struct variable {
    char *id;
    value val;
} variable;

typedef struct ast ast;
typedef struct idast idast;
typedef value *(native_fn)(value *, value *);
typedef struct dbexpr dbexpr;
typedef struct trexpr trexpr;
typedef struct callexpr callexpr;

typedef union chp {
    void *ptr;
    value *valp;
    ast *astp;
    idast *idastp;
    func *funcp;
    native_fn *native;
    dbexpr *dbp;
    trexpr *trp;
    callexpr *callp;
} chp;

typedef struct ast {
    union {
        size_t len;
        value *(*expr_cmd)(chp, value *, value *);
        statement (*stat_cmd)(chp, value *, value *);
    };
    chp chld;
} ast;

typedef struct dbexpr {
    ast lexpr;
    ast rexpr;
} dbexpr;

typedef struct trexpr {
    ast cexpr;
    ast lexpr;
    ast rexpr;
} trexpr;

typedef struct callexpr {
    ast expr;
    ast *args;
} callexpr;

typedef struct idast {
    union {
        size_t len;
        char *id;
    };
    ast expr;
} idast;

typedef struct param {
    char *id;
    union {
        ast *expr_type;
        value *type;
    };
    ast *assigned;
    bool is_spread;
    // bool is_const;
} param;

typedef struct func {
    param *prm;
    size_t prmlen;
    ast expr;
    bool is_allocated;
    value *this;
} func;

typedef struct field {
    variable *p;
    variable *h;
} field;

typedef struct object {
    field fld;
    size_t refcount;
    // value init;
    func *init;
    string *kind;
} object;

typedef struct string {
    char *ptr;
    size_t len;
} string;

typedef struct array {
    value *list;
    size_t len;
    size_t refcount;
} array;


extern int VERBOSE;
extern int CHECK;
extern char **ARGS;
extern char *VERSION;

extern variable *VAR_P;
extern variable *VAR_H;
extern size_t VAR_SIZE;

#endif