#ifndef BUILTIN_H
#define BUILTIN_H

#include "nocter.h"
#include "parser.h"



extern object OBJECT_OBJ;
extern object STRING_OBJ;
extern object INT_OBJ;
extern object BOOL_OBJ;
extern object FLOAT_OBJ;
extern object FUNC_OBJ;
extern object ARRAY_OBJ;
extern object ERROR_OBJ;
extern object FILE_OBJ;

extern value VOID_VALUE;
extern value NULL_VALUE;
extern value TRUE_VALUE;
extern value FALSE_VALUE;



void builtin(statlist *stat);
bool register_lib(char *id, statlist *stat, implist *imp);

#endif

