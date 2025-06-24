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

extern string VOID_KIND_NAME;
extern string NULL_KIND_NAME;
extern string INT_KIND_NAME;
extern string FLOAT_KIND_NAME;
extern string STRING_KIND_NAME;
extern string BOOL_KIND_NAME;
extern string ARRAY_KIND_NAME;
extern string FUNCTION_KIND_NAME;
extern string CUSTOM_KIND_NAME;
extern string OBJECT_KIND_NAME;



void builtin(statlist *stat);
bool register_lib(char *id, statlist *stat, implist *imp);

#endif

