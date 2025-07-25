#include "nocter.h"
#include "builtin.h"
#include "utils/conv.h"
#include "interpretor.h"
#include "utils/alloc.h"
#include "nocter/time.h"
#include "nocter/os.h"
#include "nocter/string.h"

#include "parser.h"




typedef struct script {
    char *p;
    char *hd;
    string path;
} script;


#define IS_ID(c)        ((c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') || c == '_')
#define IS_INT(c)       (c >= '0' && c <= '9')
// #define ISNT_ID(c)      (c < '0' || (c > '9' && c < 'A') || )
#define ISNT_ID(ch)     !(IS_ID(ch) || IS_INT(ch))

#define IS_THIS(s)      ((s)[0] == 't' && (s)[1] == 'h' && (s)[2] == 'i' && (s)[3] == 's' && ISNT_ID((s)[4]))
#define IS_TRUE(s)      ((s)[0] == 't' && (s)[1] == 'r' && (s)[2] == 'u' && (s)[3] == 'e' && ISNT_ID((s)[4]))
#define IS_FALSE(s)     ((s)[0] == 'f' && (s)[1] == 'a' && (s)[2] == 'l' && (s)[3] == 's' && (s)[4] == 'e' && ISNT_ID((s)[5]))
#define IS_VOID(s)      ((s)[0] == 'v' && (s)[1] == 'o' && (s)[2] == 'i' && (s)[3] == 'd' && ISNT_ID((s)[4]))
#define IS_NULL(s)      ((s)[0] == 'n' && (s)[1] == 'u' && (s)[2] == 'l' && (s)[3] == 'l' && ISNT_ID((s)[4]))
#define IS_NAN(s)       ((s)[0] == 'n' && (s)[1] == 'a' && (s)[2] == 'n' && ISNT_ID((s)[3]))
#define IS_INF(s)       ((s)[0] == 'i' && (s)[1] == 'n' && (s)[2] == 'f' && ISNT_ID((s)[3]))

#define IS_LET(s)       ((s)[0] == 'l' && (s)[1] == 'e' && (s)[2] == 't' && ISNT_ID((s)[3]))
#define IS_IF(s)        ((s)[0] == 'i' && (s)[1] == 'f' && ISNT_ID((s)[2]))
#define IS_ELSE(s)      ((s)[0] == 'e' && (s)[1] == 'l' && (s)[2] == 's' && (s)[3] == 'e' && ISNT_ID((s)[4]))
#define IS_WHILE(s)     ((s)[0] == 'w' && (s)[1] == 'h' && (s)[2] == 'i' && (s)[3] == 'l' && (s)[4] == 'e' && ISNT_ID((s)[5]))
#define IS_DO(s)        ((s)[0] == 'd' && (s)[1] == 'o' && ISNT_ID((s)[2]))
#define IS_FOR(s)       ((s)[0] == 'f' && (s)[1] == 'o' && (s)[2] == 'r' && ISNT_ID((s)[3]))
#define IS_SWITCH(s)    ((s)[0] == 's' && (s)[1] == 'w' && (s)[2] == 'i' && (s)[3] == 't' && (s)[4] == 'c' && (s)[5] == 'h' && ISNT_ID((s)[6]))
#define IS_CASE(s)      ((s)[0] == 'c' && (s)[1] == 'a' && (s)[2] == 's' && (s)[3] == 'e' && ISNT_ID((s)[4]))
#define IS_DEFAULT(s)   ((s)[0] == 'd' && (s)[1] == 'e' && (s)[2] == 'f' && (s)[3] == 'a' && (s)[4] == 'u' && (s)[5] == 'l' && (s)[6] == 't' && ISNT_ID((s)[7]))
#define IS_RETURN(s)    ((s)[0] == 'r' && (s)[1] == 'e' && (s)[2] == 't' && (s)[3] == 'u' && (s)[4] == 'r' && (s)[5] == 'n' && ISNT_ID((s)[6]))
#define IS_BREAK(s)     ((s)[0] == 'b' && (s)[1] == 'r' && (s)[2] == 'e' && (s)[3] == 'a' && (s)[4] == 'k' && ISNT_ID((s)[5]))
#define IS_CONTINUE(s)  ((s)[0] == 'c' && (s)[1] == 'o' && (s)[2] == 'n' && (s)[3] == 't' && (s)[4] == 'i' && (s)[5] == 'n' && (s)[6] == 'u' && (s)[7] == 'e' && ISNT_ID((s)[8]))
#define IS_IMPORT(s)    ((s)[0] == 'i' && (s)[1] == 'm' && (s)[2] == 'p' && (s)[3] == 'o' && (s)[4] == 'r' && (s)[5] == 't' && ISNT_ID((s)[6]))

#define IS_ANY_KEYS(s)  (IS_THIS(s) || IS_TRUE(s) || IS_FALSE(s) || IS_VOID(s) || IS_NULL(s))
#define IS_ANY_STAT(s)  (IS_IMPORT(s) || IS_LET(s) || IS_IF(s) || IS_ELSE(s) || IS_WHILE(s) || IS_DO(s) || IS_FOR(s) || IS_SWITCH(s) || IS_CASE(s) || IS_DEFAULT(s) || IS_RETURN(s) || IS_BREAK(s) || IS_CONTINUE(s))


static ast parse_expr(script *code);
static ast parse_spread(script *code);
static ast parse_comma(script *code);



static void syntax_err(char *text, script *start, char *end) {

    size_t line = 1;
    char *c = start->p;

    for (;;) {
        c --;
        if (*c == '\n') line ++;
        if (c == start->hd) break;
    }
    
    c = start->p;
    for (;;) {
        c --;
        if (*c == '\n') {
            c ++;
            break;
        }
        if (c == start->hd) break;
    }
    while (*c == ' ') c ++;

    char ln1[NOCTER_LINE_MAX], ln2[NOCTER_LINE_MAX], *p1 = ln1, *p2 = ln2;

    memcpy(p1, "\e[90m", 5), p1 += 5;
    memcpy(p2, "\e[90m", 5), p2 += 5;
    size_t linelen = long_to_charp(line, p1);
    p1 += linelen;
    while (linelen --) *p2 ++ = ' ';
    memcpy(p1, " | \e[0m", 7), p1 += 7;
    memcpy(p2, " | \e[1;31m", 10), p2 += 10;

    while (c < start->p) {
        if (p1 - ln1 == NOCTER_LINE_MAX - 1) break;
        *p1 ++ = *c ++, *p2 ++ = ' ';
    }

    while (c < end) {
        if (p1 - ln1 == NOCTER_LINE_MAX - 1) break;
        if (*c == '\n') {
            c = end;
            while (*c != '\n') c --;
            do c ++;
            while (*c == ' ');
            memcpy(p1, "\e[90m(...)\e[0m", 14), p1 += 14;
            memcpy(p2, "^^^^^", 5), p2 += 5;
        }
        *p1 ++ = *c ++;
        *p2 ++ = '^';
    }

    if (c == end) {
        if (*c == 0 || *c == '\n') c ++;
        *p2 ++ = '^';
    }

    while (*c != 0 && *c != '\n') {
        *p1 ++ = *c ++;
    }

    *p1 = *p2 = 0;

    char cd[PATH_MAX];
    if (getcwd(cd, sizeof(cd)) == NULL) *cd = 0;
    size_t cdlen = strlen(cd);
    if (strncmp(start->path.ptr, cd, cdlen) == 0) {
        sprintf(cd, ".%s", start->path.ptr + cdlen);
        start->path.ptr = cd;
    }

    printf("\e[1;31merror:\e[0;1m %s\e[0m\n%s, line %zu:\n%s\n%s\e[0m\n", text, start->path.ptr, line, ln1, ln2);
    exit(1);
}

static void trim(script *code) {
    for (;;) {
        if (*code->p == ' ' || *code->p == '\n' || *code->p == '\r') {
            code->p ++;
            continue;
        }
        
        if (*code->p == '/' && code->p[1] == '/') {
            while (*code->p != '\n' && *code->p != 0) code->p ++;
            continue;
        }
        
        if (*code->p == '/' && code->p[1] == '*') {
            script start = *code;
            code->p += 2;
            while (code->p[0] != '*' || code->p[1] != '/') {
                if (*code->p == 0) syntax_err("unterminated comment block", &start, code->p);
                else code->p ++;
            }
            code->p += 2;
            continue;
        }
        
        break;
    }
}



static char *identifier(script *code) {
    script start = *code;
    char id[64], *p = id;
    do {
        if (p - id == 63) syntax_err("identifier too long", &start, code->p);
        *p ++ = *code->p ++;
    }
    while (IS_ID(*code->p) || IS_INT(*code->p));
    *p ++ = '\0';
    trim(code);

    return allocpy(id, sizeof(char) * (p - id));
}

// brackets end ) ] }
static void bracketsend(script *code, char end) {
    if (*code->p == end) code->p ++, trim(code);
    else {
        code->p --;
        for (;;) {
            if (*code->p == ' ' || *code->p == '\r') code->p --;
            else if (*code->p == '\n') {
                code->p --;
                for (char *o = code->p; *o != '\n'; o --) {
                    if (o[-1] == '/' && *o == '/') {
                        code->p = o - 2;
                        break;
                    }
                }
            }
            else if (code->p[-1] == '*' && *code->p == '/') {
                code->p -= 2;
                while (code->p[-1] != '/' || *code->p != '*') code->p --;
                code->p -= 2;
            }
            else break;
        }
        code->p ++;
        char msg[] = "expected ',' or ' ' after expression";
        msg[17] = end;
        syntax_err(msg, code, code->p);
    }
}

static ast *block(script *code, bool ret, bool loop, implist *imp);


// fn
static void skip(script *code) {
    code->p ++;
    while (*code->p && *code->p != ')') {
        if (*code->p == '(') {
            skip(code);
            continue;
        }

        if (*code->p == '\'' || *code->p == '"') {
            char s = *code->p;
            code->p ++;
            while (*code->p && *code->p != s) {
                if (*code->p == '\n') return;
                if (*code->p == '\\') {
                    code->p ++;
                    if (*code->p == '(') skip(code);
                    else code->p ++;
                }
                else code->p ++;
            }
            code->p ++;
            continue;
        }

        trim(code);
        code->p ++;
    }
    code->p ++;
}
static bool is_func(script code, char end) {
    skip(&code);
    trim(&code);
    if (code.p[0] == end && code.p[1] == '>') return true;
    return false;
}
static ast parse_func(script *code) {
    script start = *code;
    code->p ++, trim(code);

    param mem[NOCTER_BUFF], *p = mem;
    size_t prmlen = 0;

    if (*code->p == ')') code->p ++, trim(code);
    else {
        for (;;) {
            if (prmlen == NOCTER_BUFF) syntax_err("too many parameters", &start, code->p);

            param prm;

            prm.is_spread = (code->p[0] == '.' && code->p[1] == '.' && code->p[2] == '.');
            if (prm.is_spread) code->p += 3, trim(code);

            if (!IS_ID(*code->p)) syntax_err("expected identifier", code, code->p);
            if (IS_ANY_KEYS(code->p) || IS_ANY_STAT(code->p)) syntax_err("unexpected use of keyword", code, code->p);

            prm.id = identifier(code);

            if (code->p[0] == ':') {
                code->p ++, trim(code);
                prm.type = astdup(parse_expr(code));
            }
            else prm.type = NULL;

            if (code->p[0] == '=') {
                code->p ++, trim(code);
                prm.assigned = astdup(parse_expr(code));
            }
            else prm.assigned = NULL;

            *p ++ = prm;
            prmlen ++;

            if (*code->p == ',') code->p ++, trim(code);
            else break;
        }
        bracketsend(code, ')');
    }

    code->p += 2, trim(code);
    return (ast){
        .expr_cmd = expr_val,
        .chld.valp = valuedup((value){
            .type = &FUNC_OBJ,
            .funcp = funcdup((func){
                .prm = allocpy(mem, sizeof(param) * prmlen),
                .prmlen = prmlen,
                .expr = parse_expr(code),
                .this = NULL
            })
        })
    };
}

// a(1, 2, 3)  [1, 2, 3]
static ast *parse_args(script *code, char end, char *errmsg) {
    script start = *code;
    code->p ++, trim(code);

    ast mem[NOCTER_BUFF], *lp = mem, *p = mem + 1;
    if (*code->p == end) code->p ++, trim(code);
    else {
        for (;;) {
            if (p - mem == NOCTER_BUFF) syntax_err(errmsg, &start, code->p);
            *p ++ = parse_spread(code);
            if (*code->p == ',') code->p ++, trim(code);
            else break;
        }
        bracketsend(code, end);
    }
    lp->len = p - mem - 1;

    return allocpy(mem, sizeof(ast) * (p - mem));
}

// let a = b  let a() => b  .a = b  .a() => b
static idast parse_dec(script *code) {
    idast res;

    if (!IS_ID(*code->p)) syntax_err("expected identifier", code, code->p);
    if (IS_ANY_KEYS(code->p) || IS_ANY_STAT(code->p)) syntax_err("unexpected use of keyword", code, code->p);

    res.id = identifier(code);

    if (*code->p == '=') {
        code->p ++, trim(code);
        res.expr = parse_expr(code);
    }
    else if (*code->p == '(') {
        if (!is_func(*code, '=')) syntax_err("expected '=' after identifier", code, code->p);
        res.expr = parse_func(code);
    }
    else {
        res.expr = (ast){
            .expr_cmd = expr_val,
            .chld.valp = &NULL_VALUE
        };
    }

    return res;
}

// 1 "abc" true ()
static ast parse_0(script *code) {

    if (IS_ID(*code->p)) {
        if (IS_ANY_STAT(code->p)) syntax_err("unexpected statement", code, code->p);
        
        if (IS_TRUE(code->p)) {
            (code->p) += 4, trim(code);
            return (ast){
                .expr_cmd = expr_val,
                .chld.valp = &TRUE_VALUE
            };
        }
        
        if (IS_FALSE(code->p)) {
            (code->p) += 5, trim(code);
            return (ast){
                .expr_cmd = expr_val,
                .chld.valp = &FALSE_VALUE
            };
        }
        
        if (IS_THIS(code->p)) {
            (code->p) += 4, trim(code);
            return (ast){
                .expr_cmd = expr_this
            };
        }
        
        if (IS_VOID(code->p)) {
            (code->p) += 4, trim(code);
            return (ast){
                .expr_cmd = expr_val,
                .chld.valp = &VOID_VALUE
            };
        }
        
        if (IS_NULL(code->p)) {
            (code->p) += 4, trim(code);
            return (ast){
                .expr_cmd = expr_val,
                .chld.valp = &NULL_VALUE
            };
        }
        
        if (IS_NAN(code->p)) {
            (code->p) += 3, trim(code);
            return (ast){
                .expr_cmd = expr_val,
                .chld.valp = valuedup((value){
                    .type = &FLOAT_OBJ,
                    .db = NAN
                })
            };
        }
        
        if (IS_INF(code->p)) {
            (code->p) += 3, trim(code);
            return (ast){
                .expr_cmd = expr_val,
                .chld.valp = valuedup((value){
                    .type = &FLOAT_OBJ,
                    .db = INFINITY
                })
            };
        }
        
        return (ast){
            .expr_cmd = expr_ident,
            .chld.ptr = identifier(code)
        };
    }

    if (IS_INT(*code->p)) {
        ast res;
        long i = 0L;

        do i = i * 10L + *code->p ++ - '0'; while (IS_INT(*code->p));
        if (*code->p == '.' && IS_INT(code->p[1])) {
            code->p ++;
            double f = i, base = 0.1;
            do {
                f += (*code->p ++ - '0') * base;
                base *= 0.1;
            } while (IS_INT(*code->p));

            res = (ast){
                .expr_cmd = expr_val,
                .chld.valp = valuedup((value){
                    .type = &FLOAT_OBJ,
                    .db = f
                })
            };
        }
        else {
            res = (ast){
                .expr_cmd = expr_val,
                .chld.valp = valuedup((value){
                    .type = &INT_OBJ,
                    .bit = i
                })
            };
        }
        
        if (IS_ID(*code->p)) syntax_err("invalid numeric literal", code, code->p);
        trim(code);
        return res;
    }

    switch (*code->p) {
        case '"': case '\'': {
            script start = *code;
            char mem[NOCTER_BUFF], *p = mem;
            code->p ++;
            while (*code->p != *start.p) {
                if (p - mem == NOCTER_BUFF - 1) syntax_err("string literal too long", &start, code->p);
                switch (*code->p) {
                    case '\\': {
                        code->p ++;
                        switch (*code->p) {
                            case 'n': *p ++ = '\n'; break;
                            case 'r': *p ++ = '\r'; break;
                            case 'a': *p ++ = '\a'; break;
                            case 'b': *p ++ = '\b'; break;
                            case 'f': *p ++ = '\f'; break;
                            case 'v': *p ++ = '\v'; break;
                            case 'e': *p ++ = '\e'; break;
                            case '(': {
                                // code->p ++, trim(code);
                                // // ast tmp = parse_comma(c);
                                // bracketsend(c, ')');
                                // code->p ++;
                            }
                            default: *p ++ = *code->p;
                        }
                        code->p ++;
                        break;
                    }
                    case '\n': case '\0': syntax_err("unterminated string literal", &start, code->p);
                    default: *p ++ = *code->p ++;
                }
            }
            *p = 0, code->p ++, trim(code);

            string *strp = alloc(sizeof(string));
            *strp = (string){
                .len = p - mem,
                .ptr = allocpy(mem, sizeof(char) * (p - mem + 1))
            };
            return (ast){
                .expr_cmd = expr_val,
                .chld.valp = valuedup((value){
                    .type = &STRING_OBJ,
                    .strp = strp
                })
            };
        }

        case '(': {
            if (is_func(*code, '-')) return parse_func(code);
            else {
                code->p ++, trim(code);
                if (*code->p == ')') syntax_err("expected expression before ')'", code, code->p);
                ast res = (ast){
                    .expr_cmd = expr_group,
                    .chld.astp = astdup(parse_comma(code))
                };
                bracketsend(code, ')');
                return res;
            }
        }

        case '{': {
            script start = *code;
            code->p ++, trim(code);

            if (*code->p == '.') {
                idast mem[NOCTER_BUFF], *lp = mem, *p = mem + 1;

                for (;;) {
                    if (p - mem == NOCTER_BUFF) syntax_err("too many fields in object literal", &start, code->p);

                    if (code->p[0] == '.' && code->p[1] == '.' && code->p[2] == '.') {
                        code->p += 3, trim(code);
                        *p ++ = (idast){
                            .id = NULL,
                            .expr = (ast){
                                .expr_cmd = expr_spread,
                                .chld.astp = astdup(parse_expr(code))
                            }
                        };
                    }
                    else {
                        code->p ++, trim(code);
                        *p ++ = parse_dec(code);
                    }

                    if (*code->p == ',') {
                        code->p ++, trim(code);
                        if (*code->p != '.') syntax_err("expected '.field = value' in object literal", code, code->p);
                    }
                    else break;
                }
                bracketsend(code, '}');

                lp->len = p - mem - 1;
                return (ast){
                    .expr_cmd = expr_obj,
                    .chld.idastp = allocpy(mem, sizeof(idast) * (p - mem))
                };
            }

            return (ast){
                .expr_cmd = expr_block,
                .chld.astp = block(code, 1, 0, NULL)
            };
        }

        case '[': {
            return (ast){
                .expr_cmd = expr_arr,
                .chld.astp = parse_args(code, ']', "too many elements in array literal")
            };
        }

        case ')': case '}': case ']': {
            char msg[] = "unmatched ' '";
            msg[11] = *code->p;
            syntax_err(msg, code, code->p);
        }
        case '\0': {
            syntax_err("unexpected termination of the expression", code, code->p);
        }
        default: {
            syntax_err("unexpected token", code, code->p);
            exit(1);
        }
    }
}

static void modifiable(ast *res, script *start, script *code, int len) {
    if (res->expr_cmd == expr_ident) res->expr_cmd = expr_ident_ptr;
    else if (res->expr_cmd == expr_this) res->expr_cmd = expr_this_ptr;
    else if (res->expr_cmd == expr_dot) res->expr_cmd = expr_dot_ptr;
    else if (res->expr_cmd == expr_access) res->expr_cmd = expr_access_ptr;
    else syntax_err("cannot assign to a temporary value", start, code->p - len);

    code->p += len, trim(code);
}

// x()  x[]  x.y
static ast parse_1(script *code) {
    ast res = parse_0(code);

    for (;;) {
        if (*code->p == '(') {
            if (is_func(*code, '=')) res = (ast){
                .expr_cmd = expr_assign,
                .chld.dbp = dbexprdup((dbexpr){
                    .lexpr = res,
                    .rexpr = parse_func(code)
                })
            };
            else res = (ast){
                .expr_cmd = expr_call,
                .chld.callp = calldup((callexpr){
                    .args = parse_args(code, ')', "too many arguments"),
                    .expr = res
                })
            };
            continue;
        }
        if (*code->p == '.') {
            code->p ++, trim(code);

            if (!(IS_ID(*code->p) || (code->p[0] == '#' && IS_ID(code->p[1])))) syntax_err("Expected identifier", code, code->p);
            if (IS_ANY_KEYS(code->p) || IS_ANY_STAT(code->p)) syntax_err("Unexpected use of keyword", code, code->p);

            res = (ast){
                .expr_cmd = expr_dot,
                .chld.idastp = idastdup((idast){
                    .id = identifier(code),
                    .expr = res
                })
            };
            continue;
        }
        if (*code->p == '[') {
            code->p ++, trim(code);
            if (*code->p == ']') syntax_err("expected expression before ']'", code, code->p);
            res = (ast){
                .expr_cmd = expr_access,
                .chld.dbp = dbexprdup((dbexpr){
                    .lexpr = res, 
                    .rexpr = parse_comma(code)
                })
            };
            bracketsend(code, ']');
            continue;
        }
        return res;
    }
}

// x = y  x += y
static ast parse_2(script *code) {
    script start = *code;
    ast res = parse_1(code);

    for (;;) {
        if (*code->p == '=') {
            modifiable(&res, &start, code, 1);
            
            res = (ast){
                .expr_cmd = expr_assign,
                .chld.dbp = dbexprdup((dbexpr){
                    .lexpr = res,
                    .rexpr = parse_expr(code)
                })
            };
        }
        else break;
    }

    return res;
}

// !x  ~x  -x  +x  --x  ++x  typeof x
static ast parse_3(script *code) {
    if (*code->p == '!') {
        code->p ++, trim(code);
        return (ast){
            .expr_cmd = expr_not,
            .chld.astp = astdup(parse_3(code))
        };
    }

    return parse_2(code);
}

// x ** y
static ast parse_4(script *code) {
    ast res = parse_3(code);

    return res;
}

// x * y  x / y  x % y
static ast parse_5(script *code) {
    ast res = parse_4(code);

    for (;;) {
        if (*code->p == '*') {
            code->p ++, trim(code);
            res = (ast){
                .expr_cmd = expr_add,
                .chld.dbp = dbexprdup((dbexpr){
                    .lexpr = res,
                    .rexpr = parse_4(code)
                })
            };
            continue;
        }
        if (*code->p == '/') {
            code->p ++, trim(code);
            res = (ast){
                .expr_cmd = expr_add,
                .chld.dbp = dbexprdup((dbexpr){
                    .lexpr = res,
                    .rexpr = parse_4(code)
                })
            };
            continue;
        }
        if (*code->p == '%') {
            code->p ++, trim(code);
            res = (ast){
                .expr_cmd = expr_add,
                .chld.dbp = dbexprdup((dbexpr){
                    .lexpr = res,
                    .rexpr = parse_4(code)
                })
            };
            continue;
        }
        return res;
    }

    return res;
}

// x + y  x - y
static ast parse_6(script *code) {
    ast res = parse_5(code);

    for (;;) {
        if (*code->p == '+') {
            code->p ++, trim(code);
            res = (ast){
                .expr_cmd = expr_add,
                .chld.dbp = dbexprdup((dbexpr){
                    .lexpr = res,
                    .rexpr = parse_5(code)
                })
            };
            continue;
        }
        if (*code->p == '-') {
            code->p ++, trim(code);
            res = (ast){
                .expr_cmd = expr_subtract,
                .chld.dbp = dbexprdup((dbexpr){
                    .lexpr = res,
                    .rexpr = parse_5(code)
                })
            };
            continue;
        }
        return res;
    }
}

// x << y  x >> y
static ast parse_7(script *code) {
    ast res = parse_6(code);

    return res;
}

// x < y  x > y  x <= y  x >= y  x in y
static ast parse_8(script *code) {
    ast res = parse_7(code);

    return res;
}

// x == y  x != y
static ast parse_9(script *code) {
    ast res = parse_8(code);

    for (;;) {
        if (code->p[0] == '=' && code->p[1] == '=') {
            code->p += 2, trim(code);
            res = (ast){
                .expr_cmd = expr_equal,
                .chld.dbp = dbexprdup((dbexpr){
                    .lexpr = res,
                    .rexpr = parse_8(code)
                })
            };
            continue;
        }
        if (code->p[0] == '!' && code->p[1] == '=') {
            code->p += 2, trim(code);
            res = (ast){
                .expr_cmd = expr_inequal,
                .chld.dbp = dbexprdup((dbexpr){
                    .lexpr = res,
                    .rexpr = parse_8(code)
                })
            };
            continue;
        }
        return res;
    }
}



// a ? b : c
static ast parse_expr(script *code) {
    ast res = parse_9(code);
    return res;
}

// ... a
static ast parse_spread(script *code) {
    if (code->p[0] == '.' && code->p[1] == '.' && code->p[2] == '.') {
        code->p += 3, trim(code);
        return (ast){
            .expr_cmd = expr_spread,
            .chld.astp = astdup(parse_expr(code))
        };
    }
    return parse_expr(code);
}

// a, b, c
static ast parse_comma(script *code) {
    script start = *code;
    ast tmp = parse_expr(code);

    if (*code->p == ',') {
        ast mem[NOCTER_BUFF], *lp = mem, *p = mem + 1;

        *p ++ = tmp;
        do {
            if (p - mem == NOCTER_BUFF) syntax_err("too many expressions in comma sequence", &start, code->p);
            code->p ++, trim(code);
            *p ++ = parse_expr(code);
        }
        while (*code->p == ',');

        lp->len = p - mem - 1;
        return (ast){
            .expr_cmd = expr_comma,
            .chld.astp = allocpy(mem, sizeof(ast) * (p - mem))
        };
    }
    
    return tmp;
}

// a;
static void linebreak(script *code) {
    if (*code->p == ';') code->p ++, trim(code);
    else if (*code->p != 0) {
        script cp = *code;
        cp.p --;
    
        for (;;) {
            if (*cp.p == ' ' || *cp.p == '\r') cp.p --;
            else if (cp.p[-1] == '*' && cp.p[0] == '/') {
                cp.p -= 2;
                while (cp.p[-1] != '/' || cp.p[0] != '*') cp.p --;
                cp.p -= 2;
            }
            else break;
        }
        if (*cp.p != '\n') cp.p ++, syntax_err("expected a line break or ';'", &cp, cp.p);
    }
}

void add_stat(statlist *stat, ast val) {
    size_t statlen = stat->p - stat->head;
    if (statlen == stat->size - 1) {
        stat->size *= 2;
        stat->head = allocs(stat->head, stat->size * sizeof(ast));
        stat->p = stat->head + statlen;
    }
    *++ stat->p = val;
}

bool add_imp(char *fullpath, implist *imp) {
    string path = {
        .ptr = fullpath,
        .len = strlen(fullpath)
    };

    for (string *p = imp->p; p != imp->head; p --) {
        if (string_equal(p, &path)) {
            if (VERBOSE) printf("\e[90mverbose: Skipping already parsed file '%s'...\e[0m\n", fullpath);
            return false;
        }
    }

    size_t implen = imp->p - imp->head;
    if (implen == imp->size - 1) {
        imp->size *= 2;
        imp->head = allocs(imp->head, imp->size * sizeof(string));
        imp->p = imp->head + implen;
    }

    path.ptr = allocpy(path.ptr, path.len);
    *++ imp->p = path;

    if (VERBOSE) printf("\e[90mverbose: Parsing file '%s'...\e[0m\n", fullpath);
    return true;
}

static void parse_stat(script *code, bool ret, bool loop, statlist *stat, implist *imp);

static void parse_file(char *fullpath, bool ret, bool loop, statlist *stat, implist *imp) {
    if (add_imp(fullpath, imp)) {

        FILE *fp = fopen(fullpath, "r");
        if (fp == NULL) {
            printf("\e[1;31merror:\e[0;1m cannot open file '%s'\n", fullpath);
            exit(1);
        }

        fseek(fp, 0, SEEK_END);
        size_t codelen = ftell(fp);
        fseek(fp, 0, SEEK_SET);

        script code;
        code.path = *imp->p;
        code.p = code.hd = alloc(sizeof(char) * (codelen + 1));
        fread(code.p, sizeof(char), codelen, fp), code.p[codelen] = 0;
        fclose(fp);

        trim(&code);
        while (*code.p != 0) parse_stat(&code, ret, loop, stat, imp);
        
        free(code.hd);
    }
}

// {}  if a b;  else a;  return;
static ast parse_single_stat(script *code, bool ret, bool loop, implist *imp) {
    if (*code->p == '{') {
        code->p ++, trim(code);
        return (ast){
            .stat_cmd = stat_block,
            .chld.astp = block(code, ret, loop, imp)
        };
    }

    if (IS_RETURN(code->p)) {
        if (!ret) syntax_err("'return' is only allowed in an evaluated block", code, code->p + 5);
        code->p += 6, trim(code);

        ast res = (ast){
            .stat_cmd = stat_return,
            .chld.astp = astdup((*code->p == ';' || IS_ANY_STAT(code->p)) ? (ast){
                .expr_cmd = expr_val,
                .chld.valp = &VOID_VALUE
            } : parse_comma(code))
        };
        linebreak(code);
        return res;
    }

    if (IS_IF(code->p)) {
        code->p += 2, trim(code);
        ast cond = parse_comma(code);
        ast expr = parse_single_stat(code, ret, loop, imp);
        
        if (IS_ELSE(code->p)) {
            code->p += 4, trim(code);
            return (ast){
                .stat_cmd = stat_if_else,
                .chld.trp = trexprdup((trexpr){
                    .cexpr = cond,
                    .lexpr = expr,
                    .rexpr = parse_single_stat(code, ret, loop, imp)
                })
            };
        }
        else return (ast){
            .stat_cmd = stat_if,
            .chld.dbp = dbexprdup((dbexpr){
                .lexpr = cond,
                .rexpr = expr
            })
        };
    }

    if (IS_WHILE(code->p)) {
        code->p += 5, trim(code);
        return (ast){
            .stat_cmd = stat_while,
            .chld.dbp = dbexprdup((dbexpr){
                .lexpr = parse_comma(code),
                .rexpr = parse_single_stat(code, ret, true, imp)
            })
        };
    }

    if (IS_ELSE(code->p)) {
        syntax_err("'else' without a matching 'if'", code, code->p + 3);
    }

    if (IS_LET(code->p)) {
        syntax_err("'let' must be inside a block", code, code->p + 2);
    }

    if (IS_IMPORT(code->p)) {
        syntax_err("'import' must be inside a block", code, code->p + 5);
    }

    ast res = parse_comma(code);
    linebreak(code);
    return (ast){
        .stat_cmd = stat_expr,
        .chld.astp = astdup(res)
    };
}

// let a;  import a.nct;  +other
static void parse_stat(script *code, bool ret, bool loop, statlist *stat, implist *imp) {
    if (IS_LET(code->p)) {
        code->p += 3, trim(code);

        for (;;) {
            idast res = parse_dec(code);

            add_stat(stat, (ast){
                .stat_cmd = stat_let,
                .chld.idastp = allocpy(&res, sizeof(idast))
            });

            if (*code->p == ',') code->p ++, trim(code);
            else break;
        }
        linebreak(code);
        return;
    }

    if (IS_IMPORT(code->p)) {
        if (imp == NULL) syntax_err("'import' is only allowed in a statement block", code, code->p + 5);
        code->p += 6, trim(code);
        script start = *code;

        char path[NOCTER_PATH_MAX];
        memcpy(path, code->path.ptr, code->path.len);
        char *p = path + code->path.len - 1;
        while (p != path) {
            if (*p == '/') {
                p ++;
                break;
            }
            p --;
        }
        char *fhd = p;
        while (*code->p != '\n' && *code->p != ';' && *code->p != 0) {
            if (p - path == NOCTER_PATH_MAX - 1) syntax_err("path too long", &start, code->p);
            *p ++ = *code->p ++;
        }
        *p = 0;

        char fullpath[NOCTER_PATH_MAX];
        if (get_fullpath(path, fullpath) == NULL) {
            if (!register_lib(fhd, stat, imp)) {
                syntax_err("no such file or directory", &start, code->p - 1);
            }
        }
        else parse_file(fullpath, ret, loop, stat, imp);

        trim(code);
        linebreak(code);
        return;
    }

    add_stat(stat, parse_single_stat(code, ret, loop, imp));
}

// {}
static ast *block(script *code, bool ret, bool loop, implist *imp) {
    // save imports
    size_t implen;
    if (imp != NULL) implen = imp->p - imp->head;

    statlist stat;
    stat.size = 16;
    stat.p = stat.head = alloc(stat.size * sizeof(ast));

    while (*code->p != '}') {
        if (*code->p == '\0') syntax_err("unexpected end of input; missing '}'?", code, code->p);
        parse_stat(code, ret, loop, &stat, imp);
    }
    code->p ++, trim(code);

    // free imports parsed in this dir
    if (imp != NULL) {
        while (implen != imp->p - imp->head) free(imp->p->ptr), imp->p --;
    }

    size_t len = stat.p - stat.head;
    stat.head->len = len;
    return allocs(stat.head, sizeof(ast) * (len + 1));
}


ast *parse(char *file) {
    long start = 0;
    if (VERBOSE) {
        start = get_current_time_ms();
        puts("\e[90mverbose: Starting parsing phase...\e[0m");
    }

    char path[NOCTER_PATH_MAX];
    if (get_fullpath(file, path) == NULL) {
        printf("\e[1;31merror:\e[0;1m No such file or directory '%s'\n", file);
        exit(1);
    }

    statlist stat;
    stat.size = 16;
    stat.p = stat.head = alloc(stat.size * sizeof(ast));

    implist imp;
    imp.size = 16;
    imp.p = imp.head = alloc(imp.size * sizeof(string));

    // parse built-in script
    builtin(&stat);

    // parse main script
    parse_file(path, 0, 0, &stat, &imp);

    // free imports
    while (imp.p != imp.head) free(imp.p->ptr), imp.p --;
    free(imp.head);

    if (VERBOSE) {
        printf("\e[90mverbose: Parsing completed successfully in %ld ms\e[0m\n", get_current_time_ms() - start);
    }

    size_t len = stat.p - stat.head;
    stat.head->len = len;
    return allocs(stat.head, sizeof(ast) * (len + 1));
}

