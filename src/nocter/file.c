#include "../nocter.h"
#include "../utils/alloc.h"
#include "../builtin.h"
#include "../interpretor.h"

// File.open(path, mode): File
value *file_open(value *tmp, value *this) {
    FILE_OBJ.refcount ++;
    *tmp = (value){
        .type = &FILE_OBJ,
        .filep = fopen(VAR_P[-1].val.strp->ptr, VAR_P[0].val.strp->ptr)
    };
    return tmp;
}

// File.close(): void
value *file_close(value *tmp, value *this) {
    fclose(this->filep);
    return &VOID_VALUE;
}

static inline size_t get_file_size(FILE *fp) {
    fseek(fp, 0, SEEK_END);
    size_t len = ftell(fp);
    fseek(fp, 0, SEEK_SET);
    return len;
}

// File.read(): String
value *file_read(value *tmp, value *this) {
    FILE *fp = this->filep;

    size_t len = get_file_size(fp);
    char *buf = alloc(len + 1);
    fread(buf, 1, len, fp);
    buf[len] = '\0';

    *tmp = (value){
        .type = &STRING_OBJ,
        .strp = stringdup((string){
            .ptr = buf,
            .len = len
        })
    };
    return tmp;
}

// File.write(text): void
value *file_write(value *tmp, value *this) {
    FILE *fp = this->filep;

    string text = *VAR_P[0].val.strp;
    fwrite(text.ptr, 1, text.len, fp);

    return &VOID_VALUE;
}

