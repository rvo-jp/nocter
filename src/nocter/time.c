#include "../nocter.h"
#include "../utils/alloc.h"
#include "../utils/conv.h"
#include "../builtin.h"

// Get current time in milliseconds
long get_current_time_ms() {
#ifdef _WIN32
    FILETIME ft;
    GetSystemTimeAsFileTime(&ft);
    ULARGE_INTEGER li = { .LowPart = ft.dwLowDateTime, .HighPart = ft.dwHighDateTime };
    // Windows FILETIME is 100-ns intervals since 1601-01-01
    return (long)((li.QuadPart - 11644473600000ULL) / 10000);
#else
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return tv.tv_sec * 1000L + tv.tv_usec / 1000L;
#endif
}


// Time.now(): Int
value *time_now(value *tmp, value *this) {
    *tmp = (value){
        .type = &INT_OBJ, 
        .bit = get_current_time_ms()
    };
    return tmp;
}

// Time.sleep(ms: Int): void
value *time_sleep(value *tmp, value *this) {
    long ms = VAR_P[0].val.bit;
    if (ms < 0) ms = 0;

#ifdef _WIN32
    Sleep(ms);
#else
    struct timespec ts;
    ts.tv_sec = ms / 1000;
    ts.tv_nsec = (ms % 1000) * 1000000;
    nanosleep(&ts, NULL);
#endif

    return &VOID_VALUE;
}
