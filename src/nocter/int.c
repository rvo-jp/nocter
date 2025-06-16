#include "../nocter.h"
#include "../utils/alloc.h"
#include "../builtin.h"
#include "../interpretor.h"

#define IS_INT(ch) (ch >= '0' && ch <= '9')

long conv_int(value val) {
    if (val.type == &INT_OBJ) return val.bit;
    
    if (val.type == &FLOAT_OBJ) return (long)val.db;
    
    if (val.type == &STRING_OBJ) {
        long i = 0L;
        char *p = val.strp->ptr;
        do i = i * 10L + *p ++ - '0'; while (IS_INT(*p));
        return i;
    }
    
    if (val.type == &BOOL_OBJ) return val.bit;
    
    return 0;
}
