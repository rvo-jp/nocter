#ifndef TIME_NOCTER_H
#define TIME_NOCTER_H

#include "../nocter.h"

long get_current_time_ms();

// Time.now(): Int
value *time_now(value *tmp, value *this);

// Time.sleep(ms): void
value *time_sleep(value *tmp, value *this);

#endif