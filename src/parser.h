#ifndef PARSER_H
#define PARSER_H

#include "nocter.h"
#include <stdlib.h>

typedef struct implist {
    string *p;
    string *head;
    size_t size;
} implist;

typedef struct statlist {
    ast *p;
    ast *head;
    size_t size;
} statlist;

void add_stat(statlist *stat, ast val);
bool add_imp(char *fullpath, implist *imp);

// parse
ast *parse(char *file);

#endif