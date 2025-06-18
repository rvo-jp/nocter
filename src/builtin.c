#include "nocter.h"
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "builtin.h"
#include "interpretor.h"
#include "utils/alloc.h"
#include "parser.h"

#include "nocter/string.h"
#include "nocter/time.h"
#include "nocter/io.h"
#include "nocter/int.h"
#include "nocter/file.h"

#define LET(ID, AST) ((ast){ .stat_cmd = stat_let, .chld.idastp = idastdup((idast){ .id = ID, .expr = AST }) })
#define LEN(n) ((ast){ .len = n })
#define ID(ID) ((ast){ .expr_cmd = expr_ident, .chld.ptr = ID })
#define NATIVE(FN) ((ast){ .expr_cmd = expr_native, .chld.native = FN })
#define VALUE(VAL) ((ast){ .expr_cmd = expr_val, .chld.valp = valuedup(VAL) })

object OBJECT_OBJ;
object STRING_OBJ;
object INT_OBJ;
object BOOL_OBJ;
object FLOAT_OBJ;
object FUNC_OBJ;
object ARRAY_OBJ;
object ERROR_OBJ;
object FILE_OBJ;

value VOID_VALUE = {
    .type = NULL,
    .bit = -1
};

value NULL_VALUE = {
    .type = NULL,
    .bit = 0
};

value TRUE_VALUE = {
    .type = &BOOL_OBJ,
    .bit = 1
};

value FALSE_VALUE = {
    .type = &BOOL_OBJ,
    .bit = 0
};

static inline object newobj(variable *objs, size_t len) {
    object obj;
    obj.fld.h = allocpy(objs, sizeof(variable) * len);
    obj.fld.p = obj.fld.h + len - 1;
    obj.refcount = 1;
    obj.init = NULL_VALUE;
    return obj;
}

static inline value newfunc(ast *arg, ast expr) {
    return (value){
        .type = &FUNC_OBJ,
        .funcp = funcdup((func){
            .arg = allocpy(arg, sizeof(ast) * (arg[0].len + 1)),
            .expr = expr,
            .this = NULL
        })
    };
}

// register
void builtin(statlist *stat) {

    add_stat(stat, LET("Int", VALUE(((value){
        .type = &OBJECT_OBJ,
        .objp = &INT_OBJ
    }))));
    INT_OBJ = newobj((variable[]){
    }, 0);

    add_stat(stat, LET("String", VALUE(((value){
        .type = &OBJECT_OBJ,
        .objp = &STRING_OBJ
    }))));
    STRING_OBJ = newobj((variable[]){
        { .id = "length", .val = newfunc((ast[]){LEN(0)}, NATIVE(string_length)) }
    }, 1);

    add_stat(stat, LET("File", VALUE(((value){
        .type = &OBJECT_OBJ,
        .objp = &FILE_OBJ
    }))));
    FILE_OBJ = newobj((variable[]){
        { .id = "open", .val = newfunc((ast[]){LEN(2), ID("path"), ID("mode")}, NATIVE(file_open)) },
        { .id = "close", .val = newfunc((ast[]){LEN(0)}, NATIVE(file_close)) },
        { .id = "read", .val = newfunc((ast[]){LEN(0)}, NATIVE(file_read)) },
        { .id = "write", .val = newfunc((ast[]){LEN(1), ID("text")}, NATIVE(file_write)) }
    }, 4);

    add_stat(stat, LET("IO", VALUE(((value){
        .type = &OBJECT_OBJ,
        .objp = objdup(newobj((variable[]){
            { .id = "print", .val = newfunc((ast[]){LEN(1), ID("any")}, NATIVE(io_print)) }
        }, 1))
    }))));
}


bool register_lib(char *id, statlist *stat, implist *imp) {

    if (id[0] == 't' && id[1] == 'i' && id[2] == 'm' && id[3] == 'e' && id[4] == '.' && id[5] == 'n' && id[6] == 'c' && id[7] == 't' && id[8] == 0) {
        if (add_imp(id, imp)) {
            add_stat(stat, LET("Time", VALUE(((value){
                .type = &OBJECT_OBJ,
                .objp = objdup(newobj((variable[]){
                    { .id = "now", .val = newfunc((ast[]){ LEN(0) }, NATIVE(time_now)) },
                    { .id = "sleep", .val = newfunc((ast[]){ LEN(1), ID("ms") }, NATIVE(time_sleep)) }
                }, 2))
            }))));
        }
        return true;
    }

    return false;
}

