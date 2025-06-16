#ifndef OS_NOCTER_H
#define OS_NOCTER_H

#include "../nocter.h"

#if defined(_WIN32) || defined(_WIN64)
#include <windows.h>
#define NOCTER_PATH_MAX MAX_PATH
#else
#include <limits.h>
#include <unistd.h>
#define NOCTER_PATH_MAX PATH_MAX
#endif

char *get_fullpath(const char *path, char *buf);

#endif