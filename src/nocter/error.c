#include "../nocter.h"
#include "../utils/alloc.h"
#include "../builtin.h"

value *err_this(value *tmp) {
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
