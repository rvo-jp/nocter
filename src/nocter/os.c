#include "../nocter.h"
#include "../utils/alloc.h"
#include "os.h"

char *get_fullpath(const char *path, char *buf) {
    #if defined(_WIN32) || defined(_WIN64)
    if (_fullpath(buf, path, NOCTER_PATH_MAX) == NULL) {
        return NULL;
    }
    #else
    if (realpath(path, buf) == NULL) {
        return NULL;
    }
    #endif

    return buf;
}

