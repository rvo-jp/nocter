#ifndef INTERPRETOR_H
#define INTERPRETOR_H

#include "nocter.h"

// "hoge" 123 true
value *expr_val(chp ch, value *tmp, value *this);
// native
value *expr_native(chp ch, value *tmp, value *this);
// {let id; return expr;}
value *expr_block(chp ch, value *tmp, value *this);
// {.id = expr, .id = expr, .id = expr}
value *expr_obj(chp ch, value *tmp, value *this);
// [expr, expr, expr]
value *expr_arr(chp ch, value *tmp, value *this);

// (expr)
value *expr_group(chp ch, value *tmp, value *this);
// expr(expr, expr, expr)
value *expr_call(chp ch, value *tmp, value *this);

/**
 * access
 */

// this
value *expr_this(chp ch, value *tmp, value *this);
value *expr_this_ptr(chp ch, value *tmp, value *this);
// id
value *expr_ident(chp ch, value *tmp, value *this);
value *expr_ident_ptr(chp ch, value *tmp, value *this);
// expr.id
value *expr_dot(chp ch, value *tmp, value *this);
value *expr_dot_ptr(chp ch, value *tmp, value *this);
// expr[expr]
value *expr_access(chp ch, value *tmp, value *this);
value *expr_access_ptr(chp ch, value *tmp, value *this);


// expr + expr
value *expr_add(chp ch, value *tmp, value *this);


// ... expr
value *expr_spread(chp ch, value *tmp, value *this);
// expr, expr, expr
value *expr_comma(chp ch, value *tmp, value *this);

// ptr = expr
value *expr_assign(chp ch, value *tmp, value *this);

/**
 * statements
 */

// expr;
statement stat_expr(chp ch, value *tmp, value *this);
// {let id; return expr;}
statement stat_block(chp ch, value *tmp, value *this);
// let id expr;
statement stat_let(chp ch, value *tmp, value *this); 
// return expr;
statement stat_return(chp ch, value *tmp, value *this);

#endif