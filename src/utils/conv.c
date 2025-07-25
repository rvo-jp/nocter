#include "../nocter.h"
#include "../interpretor.h"
#include "../builtin.h"
#include "conv.h"

#define IS_INT(ch) (ch >= '0' && ch <= '9')



double conv_float(value val) {
    if (val.type == &FLOAT_OBJ) return val.db;
    
    if (val.type == &INT_OBJ) return (double)val.bit;
    
    if (val.type == &STRING_OBJ) {
        double f = 0.0;
        char *p = val.strp->ptr;
        
        do f = f * 10.0 + *p ++ - '0';
        while (IS_INT(*p));
        
        if (*p == '.' && IS_INT(p[1])) {
            p ++;
            double base = 0.1;
            do {
                f += (*p ++ - '0') * base;
                base *= 0.1;
            } while (IS_INT(*p));
        }
        return f;
    }

    if (val.type == &BOOL_OBJ) return (double)val.bit;
    
    return 0.0;
}


