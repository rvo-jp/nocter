#ifndef FILE_NOCTER_H
#define FILE_NOCTER_H

#include "../nocter.h"

// File.open(path, mode): File
value *file_open(value *tmp, value *this);

// File.close(): void
value *file_close(value *tmp, value *this);

// File.read(): String
value *file_read(value *tmp, value *this);

// File.write(text): void
value *file_write(value *tmp, value *this);

#endif