#include "nocter.h"
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
object ERROR_THROW;
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

string VOID_KIND_NAME = (string){.ptr = "void", .len = 4};
string NULL_KIND_NAME = (string){.ptr = "null", .len = 4};
string INT_KIND_NAME = (string){.ptr = "integer", .len = 9};
string FLOAT_KIND_NAME = (string){.ptr = "float", .len = 5};
string STRING_KIND_NAME = (string){.ptr = "string", .len = 6};
string BOOL_KIND_NAME = (string){.ptr = "boolean", .len = 7};
string ARRAY_KIND_NAME = (string){.ptr = "array", .len = 5};
string FUNCTION_KIND_NAME = (string){.ptr = "function", .len = 8};
string CUSTOM_KIND_NAME = (string){.ptr = "user-defined", .len = 12};
string OBJECT_KIND_NAME = (string){.ptr = "object", .len = 6};

static inline object newobj(variable *objs, size_t len, string *kind) {
    object obj;
    obj.fld.h = allocpy(objs, sizeof(variable) * len);
    obj.fld.p = obj.fld.h + len - 1;
    obj.refcount = 1;
    obj.init = NULL;
    obj.kind = kind;
    return obj;
}

static inline value newfunc(param *prm, size_t prmlen, ast expr) {
    return (value){
        .type = &FUNC_OBJ,
        .funcp = funcdup((func){
            .prm = allocpy(prm, sizeof(param) * prmlen),
            .prmlen = prmlen,
            .expr = expr,
            .this = NULL
        })
    };
}

ast INT_AST;
ast STRING_AST;
ast ARRAY_AST;

// register
void builtin(statlist *stat) {

    INT_AST = VALUE(((value){ .type = &OBJECT_OBJ, .objp = &INT_OBJ }));
    STRING_AST = VALUE(((value){ .type = &OBJECT_OBJ, .objp = &STRING_OBJ }));
    ARRAY_AST = VALUE(((value){ .type = &OBJECT_OBJ, .objp = &ARRAY_OBJ }));

    add_stat(stat, LET("Int", INT_AST));
    INT_OBJ = newobj((variable[]){
    }, 0, &INT_KIND_NAME);

    add_stat(stat, LET("String", STRING_AST));
    STRING_OBJ = newobj((variable[]){
        { .id = "length", .val = newfunc((param[]){}, 0, NATIVE(string_length)) },
        { .id = "replaceAll", .val = newfunc((param[]){
            {.id = "old", .type = &STRING_AST, .assigned = NULL, .is_spread = false},
            {.id = "new", .type = &STRING_AST, .assigned = NULL, .is_spread = false}
        }, 2, NATIVE(string_replace_all)) }
    }, 2, &STRING_KIND_NAME);

    add_stat(stat, LET("Array", ARRAY_AST));
    ARRAY_OBJ = newobj((variable[]){
    }, 0, &ARRAY_KIND_NAME);

    add_stat(stat, LET("File", VALUE(((value){
        .type = &OBJECT_OBJ,
        .objp = &FILE_OBJ
    }))));
    FILE_OBJ = newobj((variable[]){
        { .id = "open", .val = newfunc((param[]){
            {.id = "path", .type = &STRING_AST, .assigned = NULL, .is_spread = false},
            {.id = "mode", .type = &STRING_AST, .assigned = NULL, .is_spread = false}
        }, 2, NATIVE(file_open)) },
        { .id = "close", .val = newfunc((param[]){}, 0, NATIVE(file_close)) },
        { .id = "read", .val = newfunc((param[]){}, 0, NATIVE(file_read)) },
        { .id = "write", .val = newfunc((param[]){
            {.id = "text", .type = &STRING_AST, .assigned = NULL, .is_spread = false}
        }, 1, NATIVE(file_write)) }
    }, 4, &CUSTOM_KIND_NAME);

    add_stat(stat, LET("IO", VALUE(((value){
        .type = &OBJECT_OBJ,
        .objp = objdup(newobj((variable[]){
            { .id = "print", .val = newfunc((param[]){
                {.id = "args", .type = &ARRAY_AST, .assigned = NULL, .is_spread = true}
            }, 1, NATIVE(io_print)) },
            { .id = "puts", .val = newfunc((param[]){
                {.id = "msg", .type = &STRING_AST, .assigned = NULL, .is_spread = false}
            }, 1, NATIVE(io_puts)) }
        }, 2, &CUSTOM_KIND_NAME))
    }))));
}


bool register_lib(char *id, statlist *stat, implist *imp) {

    if (id[0] == 't' && id[1] == 'i' && id[2] == 'm' && id[3] == 'e' && id[4] == '.' && id[5] == 'n' && id[6] == 'c' && id[7] == 't' && id[8] == 0) {
        if (add_imp(id, imp)) {
            add_stat(stat, LET("Time", VALUE(((value){
                .type = &OBJECT_OBJ,
                .objp = objdup(newobj((variable[]){
                    { .id = "now", .val = newfunc((param[]){}, 0, NATIVE(time_now)) },
                    { .id = "sleep", .val = newfunc((param[]){
                        {.id = "ms", .type = &INT_AST, .assigned = NULL, .is_spread = false}
                    }, 1, NATIVE(time_sleep)) }
                }, 2, &CUSTOM_KIND_NAME))
            }))));
        }
        return true;
    }

    return false;
}

