#include "nocter.h"
#include <stdio.h>
#include <math.h>
#include "builtin.h"
#include "utils/alloc.h"
#include "utils/conv.h"
#include "nocter/string.h"
#include "interpretor.h"

/**
 * 予定:
 * エラー修正系を優先・機能追加は後回し
 * 
 * - 識別子を--string型--(新たにid型)に すぐ
 * - Error型の整備 すぐ　try catch is
 */

static void free_val(value *valp);
static void refree_val(value *valp, value *tmp, value **save);


// verbose
static void free_array(array *arr) {
    if (VERBOSE) printf("\e[90mverbose: Deleting an array...\e[0m\n");

    arr->refcount --;
    if (arr->refcount == 0) {
        value *p = arr->list;
        while (arr->len --) free_val(p ++);
        free(arr->list);
        free(arr);
    }
}

static void refree_array(array *arr, value *tmp, value **save) {
    arr->refcount --;
    if (arr->refcount == 0) {
        value *p = arr->list;
        while (arr->len --) refree_val(p ++, tmp, save);
        free(arr->list);
        free(arr);
    }
}


static void free_obj(object *obj) {
    if (VERBOSE) printf("\e[90mverbose: Deleting an object...\e[0m\n");

    obj->refcount --;
    if (obj->refcount == 0) {
        while (obj->fld.p >= obj->fld.h) free_val(&obj->fld.p->val), obj->fld.p --;
        free(obj->fld.h);
        free(obj);
    }
}

static void refree_obj(object *obj, value *tmp, value **save) {
    if (VERBOSE) printf("\e[90mverbose: Deleting an object...\e[0m\n");

    obj->refcount --;
    if (obj->refcount == 0) {
        while (obj->fld.p >= obj->fld.h) refree_val(&obj->fld.p->val, tmp, save), obj->fld.p --;
        free(obj->fld.h);
        free(obj);
    }
}



static void free_func(func *fn) {
    if (fn->is_allocated) {
        if (VERBOSE) printf("\e[90mverbose: Deleting an func...\e[0m\n");
        fn->is_allocated = 0;
        free_val(fn->this);
    }
    fn->this = NULL;
}

static void refree_func(func *fn, value *tmp, value **save) {
    if (fn->is_allocated) {
        if (VERBOSE) printf("\e[90mverbose: Deleting an func safty...\e[0m\n");
        fn->is_allocated = 0;
        refree_val(fn->this, tmp, save);
    }
    fn->this = NULL;
}

static void free_string(string *str) {
    free(str->ptr);
    free(str);
}


static void free_val(value *valp) {

    if (valp->type == &STRING_OBJ) {
        free_string(valp->strp);
        return;
    }
    
    if (valp->type == &FUNC_OBJ) {
        free_func(valp->funcp);
        return;
    }
    
    if (valp->type == &ARRAY_OBJ) {
        free_array(valp->arrp);
        return;
    }

    if (valp->type == &OBJECT_OBJ) {
        free_obj(valp->objp);
        return;
    }
}

static void refree_val(value *valp, value *tmp, value **save) {
    if (valp == *save) {
        *tmp = *valp;
        *save = tmp;
        return;
    }

    if (valp->type == &STRING_OBJ) {
        free_string(valp->strp);
        return;
    }
    
    if (valp->type == &FUNC_OBJ) {
        refree_func(valp->funcp, tmp, save);
        return;
    }
    
    if (valp->type == &ARRAY_OBJ) {
        refree_array(valp->arrp, tmp, save);
        return;
    }
    
    if (valp->type == &OBJECT_OBJ) {
        refree_obj(valp->objp, tmp, save);
        return;
    }
}


static inline void free_gc(size_t varlen) {
    while (varlen != VAR_P - VAR_H) {
        free_val(&VAR_P->val);
        VAR_P --;
    }
}

static inline void refree_gc(size_t varlen, value *tmp, value **save) {
    while (varlen != VAR_P - VAR_H) {
        refree_val(&VAR_P->val, tmp, save);
        VAR_P --;
    }
}



static value dup_val(value val) {

    if (val.type == &STRING_OBJ) {
        string str = *val.strp, *strp = alloc(sizeof(string));
        str.ptr = allocpy(str.ptr, str.len + 1);
        *strp = str;
        val.strp = strp;
        return val;
    }
    
    if (val.type == &FUNC_OBJ) {
        free_func(val.funcp);
        return val;
    }

    if (val.type == &ARRAY_OBJ) {
        val.arrp->refcount ++;
        return val;
    }

    if (val.type == &OBJECT_OBJ) {
        val.objp->refcount ++;
        return val;
    }

    return val;
}




static void let(char *id, value val) {
    size_t varlen = VAR_P - VAR_H;
    if (varlen == VAR_SIZE - 1) {
        VAR_SIZE *= 2;
        if (VERBOSE) printf("\e[90mverbose: Expanded variable storage memory to %ld * sizeof(variable)\e[0m\n", VAR_SIZE);
        VAR_H = allocs(VAR_H, VAR_SIZE * sizeof(variable));
        VAR_P = VAR_H + varlen;
    }
    if (VERBOSE) printf("\e[90mverbose: Adding a new variable '%s'...\e[0m\n", id);
    *++ VAR_P = (variable){
        .id = id,
        .val = val
    };
}


static value *new_error(char *msg, size_t len, value *tmp) {
    // debug
    // printf("\e[1;31merror:\e[0;1m %s\e[0m\n", msg);
    // exit(1);

    string *strp = alloc(sizeof(string));
    strp->len = len;
    strp->ptr = allocpy(msg, len + 1);

    *tmp = (value){
        .type = &ERROR_OBJ,
        .strp = strp
    };
    return tmp;
}

// err
value *expr_val(chp ch, value *tmp, value *this) {
    return ch.valp;
}

// err
value *expr_native(chp ch, value *tmp, value *this) {
    return ch.native(tmp, this);
}

// err
value *expr_block(chp ch, value *tmp, value *this) {
    size_t varlen = VAR_P - VAR_H;

    for (size_t i = ch.astp[0].len; i; i --)  {
        ch.astp ++;
        statement res = ch.astp->stat_cmd(ch.astp->chld, tmp, this);
        if (res.type == RETURN) {
            refree_gc(varlen, tmp, &res.valp);
            return res.valp;
        }
    }

    free_gc(varlen);
    return &VOID_VALUE;
}

// err
value *expr_obj(chp ch, value *tmp, value *this) {
    size_t len = ch.idastp->len;
    object obj;
    obj.refcount = 1;
    obj.fld.h = alloc(sizeof(variable) * len);
    obj.fld.p = obj.fld.h - 1;
    obj.init = NULL_VALUE;

    while (len) {
        len --, ch.idastp ++;

        char *id = ch.idastp->id;

        value *ptr = ch.idastp->expr.expr_cmd(ch.idastp->expr.chld, tmp, this);
        if (ptr->type == &ERROR_OBJ) {
            while (obj.fld.p >= obj.fld.h) free_val(&obj.fld.p->val), obj.fld.p --;
            free(obj.fld.h);
            return ptr;
        }
        value val = ptr == tmp ? *tmp : dup_val(*ptr);

        if (id[0] == 'i' && id[1] == 'n' && id[2] == 'i' && id[3] == 't' && id[4] == 0) {
            free_val(&obj.init);
            obj.init = val;
            continue;
        }

        variable *p = obj.fld.p;
        for (;;) {
            if (p < obj.fld.h) {
                *++ obj.fld.p = (variable){ .id = id, .val = val };
                break;
            }
            if (strcmp(id, p->id) == 0) {
                free_val(&p->val);
                p->val = val;
                break;
            }
            p --;
        }
    }

    *tmp = (value){
        .type = &OBJECT_OBJ,
        .objp = alloc(sizeof(object))
    };
    *tmp->objp = obj;

    return tmp;
}

// done
value *expr_group(chp ch, value *tmp, value *this) {
    return ch.astp->expr_cmd(ch.astp->chld, tmp, this);
}


static inline value *make_array(ast *arg, array *arrp, value *tmp, value *this) {

    size_t maxlen = arg->len;
    *arrp = (array){
        .refcount = 1,
        .len = 0,
        .list = alloc(sizeof(value) * maxlen)
    };
    value *p = arrp->list;
    value *ptr;

    while (arrp->len < maxlen) {
        arg ++;
        if (arg->expr_cmd == expr_spread) {
            ptr = arg->chld.astp->expr_cmd(arg->chld.astp->chld, tmp, this);

            if (ptr->type == &ARRAY_OBJ) {
                maxlen += ptr->arrp->len - 1;
                size_t llen = p - arrp->list;
                arrp->list = allocs(arrp->list, sizeof(value) * maxlen);
                p = arrp->list + llen;

                if (ptr == tmp) {
                    ptr->arrp->refcount --;
                    if (ptr->arrp->refcount == 0) {
                        memcpy(p, ptr->arrp->list, sizeof(value) * ptr->arrp->len);
                        p += ptr->arrp->len;
                        arrp->len += ptr->arrp->len;
                        free(ptr->arrp->list);
                        free(ptr->arrp);
                        continue;
                    }
                }

                value *lp = ptr->arrp->list;
                while (arrp->len < maxlen) {
                    *p ++ = dup_val(*lp ++);
                    arrp->len ++;
                }
                continue;
            }
            if (ptr->type == &ERROR_OBJ) return ptr;

            char msg[NOCTER_LINE_MAX], *p = msg;
            memcpy(p, "Cannot spread '", 15), p += 15;
            p += ast_to_charp(*arg, p);
            memcpy(p, "'; not an array or string", 25), p += 25;
            *p = 0;
            return new_error(msg, p - msg, tmp);
        }

        ptr = arg->expr_cmd(arg->chld, tmp, this);
        if (ptr->type == &ERROR_OBJ) return ptr;

        *p ++ = (ptr == tmp) ? *tmp : dup_val(*ptr);
        arrp->len ++;
    }

    *tmp = (value){
        .type = &ARRAY_OBJ,
        .arrp = arrp
    };
    return tmp;
}

static inline void free_refarray(array *arrp) {
    value *p = arrp->list;
    while (arrp->len --) free_val(p ++);
    free(arrp->list);
}

static inline value *call_param(func *argp, ast *prm, value *tmp, value *this) {
    array arr;
    value *ptr = make_array(argp->arg, &arr, tmp, this);
    if (ptr->type == &ERROR_OBJ) return ptr;

    value *arg = arr.list;
    size_t arglen = arr.len;
    size_t prmlen = prm->len;

    while (prmlen) {
        prm ++, prmlen --;

        if (prm->expr_cmd == expr_ident) {
            if (arglen == 0) {
                free_refarray(&arr);
                char msg[NOCTER_LINE_MAX], *p = msg;
                memcpy(p, "Cannot call '", 13), p += 13;
                p += ast_to_charp(argp->expr, p);
                memcpy(p, "'; missing argument for parameter '", 35), p += 35;
                memcpy(p, prm->chld.ptr, strlen(prm->chld.ptr)), p += strlen(prm->chld.ptr);
                *p ++ = '\'';
                *p = 0;
                return new_error(msg, p - msg, tmp);
            }
            let(prm->chld.ptr, *arg);
            arg ++, arglen --;
        }
        if (prm->expr_cmd == expr_spread) {}
        if (prm->expr_cmd == expr_assign) {

        }
    }

    if (arglen) {
        free_refarray(&arr);
        char msg[NOCTER_LINE_MAX], *p = msg;
        memcpy(p, "Cannot call '", 13), p += 13;
        p += ast_to_charp(argp->expr, p);
        memcpy(p, "'; ", 3), p += 3;
        p += long_to_charp(arglen, p);
        memcpy(p, " unexpected argument", 20), p += 20;
        if (arglen > 1) *p ++ = 's';
        *p = 0;
        return new_error(msg, p - msg, tmp);
    }

    free(arr.list);
    return &NULL_VALUE;
}

value *expr_arr(chp ch, value *tmp, value *this) {
    if (VERBOSE) printf("\e[90mverbose: Making a new array...\e[0m\n");

    array *arrp = alloc(sizeof(array));
    value *ptr = make_array(ch.astp, arrp, tmp, this);
    if (ptr->type == &ERROR_OBJ) {
        free_array(arrp);
        return ptr;
    }

    return ptr;
}

value *expr_call(chp ch, value *tmp, value *this) {
    if (VERBOSE) puts("\e[90mverbose: Called\e[0m");
    value *ptr = ch.funcp->expr.expr_cmd(ch.funcp->expr.chld, tmp, this);

    if (ptr->type == &FUNC_OBJ) {
        func *fnp = ptr->funcp;

        size_t varlen = VAR_P - VAR_H;
        value *lptr = call_param(ch.funcp, fnp->arg, tmp, this);
        if (lptr->type == &ERROR_OBJ) {
            VAR_P = VAR_H + varlen;
            return lptr;
        }

        value *res = fnp->expr.expr_cmd(fnp->expr.chld, tmp, fnp->this);
        if (ptr == tmp) refree_func(fnp, tmp, &res);
        if (res == tmp) free_gc(varlen);
        else refree_gc(varlen, tmp, &res);

        return res;
    }

    if (ptr->type == &OBJECT_OBJ) {
        object *objp = ptr->objp;
        if (objp->init.type == &FUNC_OBJ) {
            func *fnp = objp->init.funcp;

            size_t varlen = VAR_P - VAR_H;
            value *lptr = call_param(ch.funcp, fnp->arg, tmp, this);
            if (lptr->type == &ERROR_OBJ) {
                VAR_P = VAR_H + varlen;
                return lptr;
            }

            field *fldp = alloc(sizeof(field));
            fldp->h = alloc(0);
            fldp->p = fldp->h - 1;

            value this = {
                .type = objp,
                .fldp = fldp
            };
            value *res = fnp->expr.expr_cmd(fnp->expr.chld, tmp, &this);
            free_gc(varlen);
            if (res == tmp) free_val(tmp);

            *tmp = this;
            return tmp;
        }

        char msg[NOCTER_LINE_MAX], *p = msg;
        memcpy(p, "cannot instantiate '", 20), p += 20;
        p += ast_to_charp(ch.funcp->expr, p);
        memcpy(p, "'; property 'init' is not a function", 36), p += 36;
        return new_error(msg, p - msg, tmp);;
    }

    if (ptr->type == &ERROR_OBJ) return ptr;

    char msg[NOCTER_LINE_MAX], *p = msg;
    memcpy(p, "cannot call '", 13), p += 13;
    p += ast_to_charp(ch.funcp->expr, p);
    memcpy(p, "'; not a function", 17), p += 17;
    *p = 0;
    return new_error(msg, p - msg, tmp);
}



/**
 * access
 */

static value *err_access_nullvoid(ast expr, char *id, int nullvoid, value *tmp) {
    char msg[NOCTER_LINE_MAX], *p = msg;
    memcpy(p, "cannot access '", 15), p += 15;
    size_t len = strlen(id);
    memcpy(p, id, len), p += len;
    memcpy(p, "'; '", 4), p += 4;
    p += ast_to_charp(expr, p);
    memcpy(p, "' is ", 5), p += 5;
    memcpy(p, nullvoid ? "void" : "null", 4), p += 4;
    *p = 0;
    return new_error(msg, p - msg, tmp);
}

static value *err_access_primitive(ast expr, char *id, value *tmp) {
    char msg[NOCTER_LINE_MAX], *p = msg;
    *p ++ = '\'';
    p += ast_to_charp(expr, p);
    memcpy(p, "' is primitive (cannot access '", 31), p += 31;
    size_t len = strlen(id);
    memcpy(p, id, len), p += len;
    *p ++ = '\'';
    *p ++ = ')';
    *p = 0;
    return new_error(msg, p - msg, tmp);
}

static value *err_access_out_of_bounds(ast expr, size_t index, size_t len, value *tmp) {
    char msg[NOCTER_LINE_MAX], *p = msg;
    memcpy(p, "index ", 6), p += 6;
    p += long_to_charp(index, p);
    memcpy(p, " out of bounds for '", 20), p += 20;
    p += ast_to_charp(expr, p);
    memcpy(p, "' (length is ", 13), p += 13;
    p += long_to_charp(len, p);
    *p ++ = ')';
    *p = 0;
    return new_error(msg, p - msg, tmp);
}

static value *err_access_not_int(dbexpr db, value *tmp) {
    char msg[NOCTER_LINE_MAX], *p = msg;
    *p ++ = '\'';
    p += ast_to_charp(db.rexpr, p);
    memcpy(p, "' is not an integer (cannot index '", 35), p += 35;
    p += ast_to_charp(db.lexpr, p);
    *p ++ = '\'';
    *p ++ = ')';
    *p = 0;
    return new_error(msg, p - msg, tmp);
}

static value *err_access_not_list(ast lexpr, value *tmp) {
    char msg[NOCTER_LINE_MAX], *p = msg;
    *p ++ = '\'';
    p += ast_to_charp(lexpr, p);
    memcpy(p, "' is not an array or string (cannot index)", 42), p += 42;
    *p = 0;
    return new_error(msg, p - msg, tmp);
}

static value *err_not_def(char *id, value *tmp) {
    char msg[NOCTER_LINE_MAX], *p = msg;
    *p ++ = '\'';
    size_t len = strlen(id);
    memcpy(p, id, len), p += len;
    memcpy(p, "' is not defined", 16), p += 16;
    *p = 0;
    return new_error(msg, p - msg, tmp);
}

static value *err_this(value *tmp) {
    static string _ERROR_STR_TIND_ = {
        .ptr = "'this' is not defined",
        .len = 21
    };
    static value _ERROR_TIND_ = {
        .type = &ERROR_OBJ,
        .strp = &_ERROR_STR_TIND_
    };
    *tmp = _ERROR_TIND_;
    return tmp;
}

static value *dot_access(ast expr, char *id, value *tmp, value *this) {
    value *ptr = expr.expr_cmd(expr.chld, tmp, this);
    if (ptr->type == &ERROR_OBJ) return ptr;
    if (ptr->type == NULL) {
        if (ptr == tmp) free_val(tmp);
        return err_access_nullvoid(expr, id, ptr->bit, tmp);
    }
    
    bool is_tmp = (ptr == tmp);
    field *fldp;
    char *cmp_id;
    
    if (*id == '#') {
        if (ptr->type == &OBJECT_OBJ || ptr->type == &STRING_OBJ || ptr->type == &INT_OBJ || ptr->type == &BOOL_OBJ || ptr->type == &FLOAT_OBJ || ptr->type == &FUNC_OBJ || ptr->type == &ARRAY_OBJ) return err_access_primitive(expr, id, tmp);
        fldp = ptr->fldp;
        cmp_id = id + 1;
    }
    else {
        fldp = (ptr->type == &OBJECT_OBJ) ? &ptr->objp->fld : &ptr->type->fld;
        cmp_id = id;
    }

    for (variable *p = fldp->p; p >= fldp->h; p --) {
        if (strcmp(cmp_id, p->id) == 0) {
            if (p->val.type == &FUNC_OBJ) {
                p->val.funcp->is_allocated = is_tmp;
                p->val.funcp->this = is_tmp ? valuedup(*tmp) : ptr;
                *tmp = p->val;
                return tmp;
            }
            value *res = &p->val;
            if (is_tmp) refree_val(tmp, tmp, &res);
            return res;
        }
    }

    if (is_tmp) free_val(tmp);
    char msg[NOCTER_LINE_MAX], *p = msg;
    *p ++ = '\'';
    p += ast_to_charp(expr, p);
    memcpy(p, "' has no property '", 19), p += 19;
    size_t len = strlen(id);
    memcpy(p, id, len), p += len;
    *p ++ = '\'';
    *p = 0;
    return new_error(msg, p - msg, tmp);
}

static value *dot_access_ptr(ast expr, char *id, value *tmp, value *this) {
    value *ptr = expr.expr_cmd(expr.chld, tmp, this);
    if (ptr->type == &ERROR_OBJ) return ptr;
    if (ptr->type == NULL) {
        if (ptr == tmp) free_val(tmp);
        return err_access_nullvoid(expr, id, ptr->bit, tmp);
    }
    if (ptr == tmp) {
        free_val(tmp);
        return tmp;
    }

    field *fldp;
    char *cmp_id;

    if (*id == '#') {
        if (ptr->type == &OBJECT_OBJ || ptr->type == &STRING_OBJ || ptr->type == &INT_OBJ || ptr->type == &BOOL_OBJ || ptr->type == &FLOAT_OBJ || ptr->type == &FUNC_OBJ || ptr->type == &ARRAY_OBJ) return err_access_primitive(expr, id, tmp);
        fldp = ptr->fldp;
        cmp_id = id + 1;
    }
    else {
        fldp = (ptr->type == &OBJECT_OBJ) ? &ptr->objp->fld : &ptr->type->fld;
        cmp_id = id;
    }

    for (variable *p = fldp->p; p >= fldp->h; p --) {
        if (strcmp(cmp_id, p->id) == 0) {
            free_val(&p->val);
            return &p->val;
        }
    }

    size_t len = fldp->p - fldp->h + 1;
    fldp->h = allocs(fldp->h, sizeof(variable) * (len + 1));
    fldp->p = fldp->h + len;
    fldp->p->id = cmp_id;
    return &fldp->p->val;
}

value *expr_dot(chp ch, value *tmp, value *this) {
    return dot_access(ch.idastp->expr, ch.idastp->id, tmp, this);
}

value *expr_dot_ptr(chp ch, value *tmp, value *this) {
    return dot_access_ptr(ch.idastp->expr, ch.idastp->id, tmp, this);
}

value *expr_access(chp ch, value *tmp, value *this) {
    value *rptr = ch.dbp->rexpr.expr_cmd(ch.dbp->rexpr.chld, tmp, this);
    
    if (rptr->type == &STRING_OBJ) {
        string *strp = rptr->strp;
        bool is_tmp = (rptr == tmp);
        value *res = dot_access(ch.dbp->lexpr, strp->ptr, tmp, this);
        if (is_tmp) {
            free(strp->ptr);
            free(strp);
        }
        return res;
    }
    if (rptr->type == &INT_OBJ) {
        long index = rptr->bit;
        value *lptr = ch.dbp->lexpr.expr_cmd(ch.dbp->lexpr.chld, tmp, this);

        if (lptr->type == &ARRAY_OBJ) {
            array *arr = lptr->arrp;
            if (index < 0 || index >= arr->len) {
                if (lptr == tmp) free_array(arr);
                return err_access_out_of_bounds(ch.dbp->lexpr, index, arr->len, tmp);
            }

            value *res = arr->list + index;
            if (lptr == tmp) refree_array(arr, tmp, &res);
            return res;
        }
        if (lptr->type == &ERROR_OBJ) return lptr;

        if (lptr == tmp) free_val(tmp);
        return err_access_not_list(ch.dbp->lexpr, tmp);
    }
    if (rptr->type == &ERROR_OBJ) return rptr;

    if (rptr == tmp) free_val(tmp);
    return err_access_not_int(*ch.dbp, tmp);
}

value *expr_access_ptr(chp ch, value *tmp, value *this) {
    value *rptr = ch.dbp->rexpr.expr_cmd(ch.dbp->rexpr.chld, tmp, this);
    
    if (rptr->type == &STRING_OBJ) {
        string *strp = rptr->strp;
        bool is_tmp = (rptr == tmp);
        value *res = dot_access_ptr(ch.dbp->lexpr, strp->ptr, tmp, this);
        if (is_tmp) {
            free(strp->ptr);
            free(strp);
        }
        return res;
    }
    if (rptr->type == &INT_OBJ) {
        long index = rptr->bit;
        value *lptr = ch.dbp->lexpr.expr_cmd(ch.dbp->lexpr.chld, tmp, this);

        if (lptr->type == &ARRAY_OBJ) {
            array *arr = lptr->arrp;
            if (lptr == tmp) {
                free_array(arr);
                return tmp;
            }
            
            if (index >= arr->len) {
                arr->list = allocs(arr->list, sizeof(value) * (index + 1));
                value *p = arr->list + arr->len;
                while (index > arr->len ++) *p ++ = NULL_VALUE;
                return p;
            }
            if (index < 0) return err_access_out_of_bounds(ch.dbp->lexpr, index, arr->len, tmp);

            return arr->list + index;
        }
        if (lptr->type == &ERROR_OBJ) return lptr;

        if (lptr == tmp) free_val(tmp);
        return err_access_not_list(ch.dbp->lexpr, tmp);
    }
    if (rptr->type == &ERROR_OBJ) return rptr;

    if (rptr == tmp) free_val(tmp);
    return err_access_not_int(*ch.dbp, tmp);
}

value *expr_this(chp ch, value *tmp, value *this) {
    if (this == NULL) return err_this(tmp);
    return this;
}

value *expr_this_ptr(chp ch, value *tmp, value *this) {
    if (this == NULL) return err_this(tmp);
    free_val(this);
    return this;
}

value *expr_ident(chp ch, value *tmp, value *this) {
    if (VERBOSE) printf("\e[90mverbose: Getting variable '%s'...\e[0m\n", (char *)ch.ptr);

    for (variable *p = VAR_P; p >= VAR_H; p --) {
        if (strcmp(ch.ptr, p->id) == 0) return &p->val;
    }
    return err_not_def(ch.ptr, tmp);
}

value *expr_ident_ptr(chp ch, value *tmp, value *this) {
    if (VERBOSE) printf("\e[90mverbose: Getting variable '%s'...\e[0m\n", (char *)ch.ptr);

    for (variable *p = VAR_P; p >= VAR_H; p --) {
        if (strcmp(ch.ptr, p->id) == 0) {
            free_val(&p->val);
            return &p->val;
        }
    }
    return err_not_def(ch.ptr, tmp);
}


/**
 * ptr
 */

value *expr_assign(chp ch, value *tmp, value *this) {
    value *ptr = ch.dbp->rexpr.expr_cmd(ch.dbp->rexpr.chld, tmp, this);
    if (ptr->type == &ERROR_OBJ) return ptr;
    value rval = (ptr == tmp) ? *tmp : dup_val(*ptr);

    ptr = ch.dbp->lexpr.expr_cmd(ch.dbp->lexpr.chld, tmp, this);
    if (ptr->type == &ERROR_OBJ) {
        free_val(&rval);
        return ptr;
    }

    *ptr = rval;
    return ptr;
}



/**
 * tmp operate
 */
static string VOID_NAME = (string){.ptr = "void", .len = 4};
static string NULL_NAME = (string){.ptr = "null", .len = 4};
static string INT_NAME = (string){.ptr = "integer", .len = 9};
static string FLOAT_NAME = (string){.ptr = "float", .len = 5};
static string STRING_NAME = (string){.ptr = "string", .len = 6};
static string BOOL_NAME = (string){.ptr = "boolean", .len = 7};
static string ARRAY_NAME = (string){.ptr = "array", .len = 5};
static string FUNCTION_NAME = (string){.ptr = "function", .len = 8};
static string CUSTOM_NAME = (string){.ptr = "user-defined", .len = 12};
static string OBJECT_NAME = (string){.ptr = "object", .len = 6};

static string cnv_type(value *ptr) {
    if (ptr->type == &INT_OBJ) return INT_NAME;
    if (ptr->type == &FLOAT_OBJ) return FLOAT_NAME;
    if (ptr->type == &STRING_OBJ) {
        free_string(ptr->strp);
        return STRING_NAME;
    }
    if (ptr->type == &ARRAY_OBJ) {
        free_array(ptr->arrp);
        return ARRAY_NAME;
    }
    if (ptr->type == &FUNC_OBJ) {
        free_array(ptr->funcp);
        return FUNCTION_NAME;
    }
    if (ptr->type == &OBJECT_OBJ) {
        free_obj(ptr->objp);
        return OBJECT_NAME;
    }


    return CUSTOM_NAME;
}





static string ADD_CMD = (string){.ptr = "add", .len = 3};
static string SUB_CMD = (string){.ptr = "subtract", .len = 8};

static value *err_operate(string cmd, string l, string r, dbexpr *dbp, value *tmp) {
    // cannot add integer and user-defined in `num + player`
    char msg[NOCTER_LINE_MAX], *p = msg;
    memcpy(p, "cannot ", 7), p += 7;
    memcpy(p, cmd.ptr, cmd.len), p += cmd.len;
    *p ++ = ' ';
    memcpy(p, l.ptr, l.len), p += l.len;
    memcpy(p, " and ", 5), p += 5;
    memcpy(p, r.ptr, r.len), p += r.len;
    memcpy(p, " in `", 5), p += 5;
    p += ast_to_charp(dbp->lexpr, p);
    memcpy(p, " + ", 5), p += 5;
    p += ast_to_charp(dbp->rexpr, p);
    *p ++ = '`';
    *p = 0;
    return new_error(msg, p - msg, tmp);
}

static inline void add_string(string str1, string str2, value *tmp) {
    string str;
    str.len = str1.len + str2.len;
    str.ptr = alloc(str.len + 1);

    memcpy(str.ptr, str1.ptr, str1.len);
    memcpy(str.ptr + str1.len, str2.ptr, str2.len + 1);

    tmp->type = &STRING_OBJ;
    tmp->strp = alloc(sizeof(string));
    *tmp->strp = str;
}

value *expr_add(chp ch, value *tmp, value *this) {
    value *lptr = ch.dbp->lexpr.expr_cmd(ch.dbp->lexpr.chld, tmp, this);
    
    if (lptr->type == &INT_OBJ) {
        long i = lptr->bit;
        value *rptr = ch.dbp->rexpr.expr_cmd(ch.dbp->rexpr.chld, tmp, this);
        
        if (rptr->type == &INT_OBJ) {
            *tmp = (value){ .type = &INT_OBJ, .bit = i + rptr->bit};
            return tmp;
        }
        if (rptr->type == &FLOAT_OBJ) {
            *tmp = (value){ .type = &FLOAT_OBJ, .db = (double)i + rptr->db};
            return tmp;
        }
        if (rptr->type == &STRING_OBJ) {
            char buf[32];
            size_t len = long_to_charp(i, buf);
            buf[len] = '\0';
            add_string((string){ .ptr = buf, .len = len }, *rptr->strp, tmp);
            free_string(rptr->strp);
            return tmp;
        }
        if (rptr->type == &ERROR_OBJ) return rptr;

        return err_operate(ADD_CMD, INT_NAME, cnv_type(rptr), ch.dbp, tmp);
    }

    if (lptr->type == &FLOAT_OBJ) {
        double f = lptr->db;
        value *rptr = ch.dbp->rexpr.expr_cmd(ch.dbp->rexpr.chld, tmp, this);

        if (rptr->type == &INT_OBJ) {
            *tmp = (value){ .type = &FLOAT_NAME, .db = f + (double)rptr->bit};
            return tmp;
        }
        if (rptr->type == &FLOAT_OBJ) {
            *tmp = (value){ .type = &FLOAT_OBJ, .db = f + rptr->db};
            return tmp;
        }
        if (rptr->type == &STRING_OBJ) {
            char buf[32];
            size_t len = double_to_charp(f, buf);
            buf[len] = '\0';
            add_string((string){ .ptr = buf, .len = len }, *rptr->strp, tmp);
            free_string(rptr->strp);
            return tmp;
        }
        if (rptr->type == &ERROR_OBJ) return rptr;

        return err_operate(ADD_CMD, FLOAT_NAME, cnv_type(rptr), ch.dbp, tmp);
    }

    if (lptr->type == &STRING_OBJ) {
        string *strp = lptr->strp;
        value *rptr = ch.dbp->rexpr.expr_cmd(ch.dbp->rexpr.chld, tmp, this);
        if (rptr->type == &ERROR_OBJ) return rptr;
        char buf[NOCTER_BUFF];
        add_string(*strp, conv_str(buf, *rptr), tmp);
        free_string(strp);
        free_val(rptr);
        return tmp;
    }

    if (lptr->type == &ERROR_OBJ) return lptr;

    value lval = *lptr;
    value *rptr = ch.dbp->rexpr.expr_cmd(ch.dbp->rexpr.chld, tmp, this);
    if (rptr->type == &STRING_OBJ) {
        string *strp = rptr->strp;
        char buf[NOCTER_BUFF];
        add_string(conv_str(buf, lval), *strp, tmp);
        free_val(&lval);
        free_string(strp);
        return tmp;
    }

    return err_operate(ADD_CMD, cnv_type(&lval), cnv_type(rptr), ch.dbp, tmp);
}

value *expr_subtract(chp ch, value *tmp, value *this) {
    value *lptr = ch.dbp->lexpr.expr_cmd(ch.dbp->lexpr.chld, tmp, this);

    if (lptr->type == &INT_OBJ) {
        long i = lptr->bit;
        value *rptr = ch.dbp->rexpr.expr_cmd(ch.dbp->rexpr.chld, tmp, this);
        
        if (rptr->type == &INT_OBJ) {
            *tmp = (value){ .type = &INT_OBJ, .bit = i - rptr->bit};
            return tmp;
        }
        if (rptr->type == &FLOAT_OBJ) {
            *tmp = (value){ .type = &FLOAT_OBJ, .db = (double)i - rptr->db};
            return tmp;
        }
        if (rptr->type == &ERROR_OBJ) return rptr;

        return err_operate(SUB_CMD, INT_NAME, cnv_type(rptr), ch.dbp, tmp);
    }

    if (lptr->type == &FLOAT_OBJ) {
        double f = lptr->bit;
        value *rptr = ch.dbp->rexpr.expr_cmd(ch.dbp->rexpr.chld, tmp, this);

        if (rptr->type == &INT_OBJ) {
            *tmp = (value){ .type = &FLOAT_NAME, .db = f - (double)rptr->bit};
            return tmp;
        }
        if (rptr->type == &FLOAT_OBJ) {
            *tmp = (value){ .type = &FLOAT_OBJ, .db = f - rptr->db};
            return tmp;
        }
        if (rptr->type == &ERROR_OBJ) return rptr;

        return err_operate(SUB_CMD, FLOAT_NAME, cnv_type(rptr), ch.dbp, tmp);
    }

    if (lptr->type == &ERROR_OBJ) return lptr;

    return err_operate(SUB_CMD, cnv_type(lptr), cnv_type(ch.dbp->rexpr.expr_cmd(ch.dbp->rexpr.chld, tmp, this)), ch.dbp, tmp);
}



// err
value *expr_spread(chp ch, value *tmp, value *this) {
    value *ptr = ch.astp->expr_cmd(ch.astp->chld, tmp, this);

    if (ptr->type == &ARRAY_OBJ) {
        array *arr = ptr->arrp;
        value *res = arr->list + arr->len - 1;
        if (ptr == tmp) refree_array(arr, tmp, &res);
        return res;
    }
    if (ptr->type == &ERROR_OBJ) return ptr;

    // cannot spread integer; expected array or string in `...arr`

    char msg[NOCTER_LINE_MAX], *p = msg;
    memcpy(p, "cannot spread ", 14), p += 14;
    string type = cnv_type(ptr);
    memcpy(p, type.ptr, type.len), p += type.len;
    memcpy(p, "; expected array or string in `...", 34), p += 34;
    p += ast_to_charp(*ch.astp, p);
    *p ++ = '`';
    *p = 0;
    return new_error(msg, p - msg, tmp);
}

// err
value *expr_comma(chp ch, value *tmp, value *this) {
    size_t i = ch.astp->len;
    
    value *ptr;
    for (;;) {
        ch.astp ++;
        i --;
        ptr = ch.astp->expr_cmd(ch.astp->chld, tmp, this);

        if (i == 0) return ptr;
        if (ptr->type == &ERROR_OBJ) return ptr;
        
        if (ptr == tmp) free_val(tmp);
    }
}




statement stat_expr(chp ch, value *tmp, value *this) {
    value *ptr = ch.astp->expr_cmd(ch.astp->chld, tmp, this);
    if (ptr->type == &ERROR_OBJ) {
        return (statement){
            .type = RETURN,
            .valp = ptr
        };
    }
    if (ptr == tmp) free_val(tmp);
    return (statement){ .type = VOID };
}

statement stat_block(chp ch, value *tmp, value *this) {
    size_t varlen = VAR_P - VAR_H;

    for (size_t i = ch.astp[0].len; i; i --)  {
        ch.astp ++;
        statement res = ch.astp->stat_cmd(ch.astp->chld, tmp, this);
        if (res.type == RETURN) {
            refree_gc(varlen, tmp, &res.valp);
            return res;
        }
    }

    free_gc(varlen);
    return (statement){ .type = VOID };
}

statement stat_let(chp ch, value *tmp, value *this) {
    value *ptr = ch.idastp->expr.expr_cmd(ch.idastp->expr.chld, tmp, this);
    if (ptr->type == &ERROR_OBJ) return (statement){
        .type = RETURN,
        .valp = ptr
    };
    let(ch.idastp->id, ptr == tmp ? *tmp : dup_val(*ptr));
    return (statement){ .type = VOID };
}

statement stat_return(chp ch, value *tmp, value *this) {
    return (statement){
        .type = RETURN,
        .valp = ch.astp->expr_cmd(ch.astp->chld, tmp, this)
    };
}

static statement err_if_while(value *ptr, value *tmp, ast code, string wi) {
    if (ptr->type == &ERROR_OBJ) return (statement){
        .type = RETURN,
        .valp = ptr
    };

    // cannot use integer as condition in `if i`

    char msg[NOCTER_LINE_MAX], *p = msg;
    memcpy(p, "cannot use ", 11), p += 11;
    string type = cnv_type(ptr);
    memcpy(p, type.ptr, type.len), p += type.len;
    memcpy(p, " as condition in `", 18), p += 18;
    memcpy(p, wi.ptr, wi.len), p += wi.len;
    *p ++ = ' ';
    p += ast_to_charp(code, p);
    *p ++ = '`';
    *p = 0;
    return (statement){
        .type = RETURN,
        .valp = new_error(msg, p - msg, tmp)
    };
}

string CMD_IF = (string){.ptr = "if", .len = 2};
string CMD_WHILE = (string){.ptr = "while", .len = 5};

statement stat_if(chp ch, value *tmp, value *this) {
    value *ptr = ch.dbp->lexpr.expr_cmd(ch.dbp->lexpr.chld, tmp, this);

    if (ptr->type != &BOOL_OBJ) return err_if_while(ptr, tmp, ch.dbp->lexpr, CMD_IF);
    return ptr->bit ? ch.dbp->rexpr.stat_cmd(ch.dbp->rexpr.chld, tmp, this)
    : (statement){ .type = VOID };
}

statement stat_if_else(chp ch, value *tmp, value *this) {
    value *ptr = ch.trp->cexpr.expr_cmd(ch.trp->cexpr.chld, tmp, this);

    if (ptr->type != &BOOL_OBJ) return err_if_while(ptr, tmp, ch.trp->cexpr, CMD_IF);
    return ptr->bit ? ch.trp->lexpr.stat_cmd(ch.trp->lexpr.chld, tmp, this)
    : ch.trp->rexpr.stat_cmd(ch.trp->rexpr.chld, tmp, this);
}

statement stat_while(chp ch, value *tmp, value *this) {
    value *ptr;

    while (1) {
        ptr = ch.dbp->lexpr.expr_cmd(ch.dbp->lexpr.chld, tmp, this);
        if (ptr->type != &BOOL_OBJ) return err_if_while(ptr, tmp, ch.dbp->lexpr, CMD_WHILE);
        if (ptr->bit) {
            statement res = ch.dbp->rexpr.stat_cmd(ch.dbp->rexpr.chld, tmp, this);
            if (res.type == RETURN || res.type == BREAK) return res;
        }
        else break;
    }

    return (statement){ .type = VOID };
}
