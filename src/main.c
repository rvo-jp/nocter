#include "nocter.h"
#include <stdio.h>
#include <stdlib.h>
#include "parser.h"
#include "builtin.h"
#include "utils/alloc.h"
#include "nocter/time.h"

int VERBOSE = 0;
int CHECK = 0;
char **ARGS = NULL;
char *VERSION = "1.2.0";

variable *VAR_P;
variable *VAR_H;
size_t VAR_SIZE;

static void nocter() {
    ast *p = parse(*ARGS);
    if (CHECK) {
        puts("check: no syntax errors detected");
        return;
    }

    VAR_SIZE = 16;
    VAR_H = alloc(sizeof(variable) * VAR_SIZE);
    VAR_P = VAR_H - 1;
    
    long start;
    if (VERBOSE) {
        start = get_current_time_ms();
        puts("\e[90mverbose: Starting execution of the program...\e[0m");
    }

    value tmp;
    statement stat;
    for (size_t i = p->len; i; i --) {
        p ++;
        stat = p->stat_cmd(p->chld, &tmp, NULL);
        if (stat.type == RETURN && stat.valp->type == &ERROR_OBJ) {
            printf("\e[1;31merror:\e[0;1m %s\e[0m\n", stat.valp->strp->ptr);
            exit(1);
        }
    }
    if (VERBOSE) printf("\e[90mverbose: Execution finished successfully in %ld ms\e[0m\n", get_current_time_ms() - start);
}

int main(int argc, char **argv) {

    // repl
    if (argc == 1) {
        printf("Welcome to Nocter %s interactive mode!\nPress Ctrl+D to exit.\n", VERSION);
        return 0;
    }

    while (*++ argv != NULL) {
        // options
        if (**argv == '-') {
            #define OPT_VERSION(s) (((s)[1] == 'v' && (s)[2] == 0) || (((s)[1] == '-' && (s)[2] == 'v' && (s)[3] == 'e' && (s)[4] == 'r' && (s)[5] == 's' && (s)[6] == 'i' && (s)[7] == 'o' && (s)[8] == 'n' && (s)[9] == 0)))
            #define OPT_HELP(s) (((s)[1] == 'h' && (s)[2] == 0) || (((s)[1] == '-' && (s)[2] == 'h' && (s)[3] == 'e' && (s)[4] == 'l' && (s)[5] == 'p' && (s)[6] == 0)))
            #define OPT_CHECK(s) (((s)[1] == '-' && (s)[2] == 'c' && (s)[3] == 'h' && (s)[4] == 'e' && (s)[5] == 'c' && (s)[6] == 'k' && (s)[7] == 0))
            #define OPT_VERBOSE(s) (((s)[1] == '-' && (s)[2] == 'v' && (s)[3] == 'e' && (s)[4] == 'r' && (s)[5] == 'b' && (s)[6] == 'o' && (s)[7] == 's' && (s)[8] == 'e' && (s)[9] == 0))
            #define OPT_CREDITS(s) (((s)[1] == '-' && (s)[2] == 'c' && (s)[3] == 'r' && (s)[4] == 'e' && (s)[5] == 'd' && (s)[6] == 'i' && (s)[7] == 't' && (s)[8] == 's' && (s)[9] == 0))

            if (OPT_VERSION(*argv)) {
                printf("Nocter %s\n", VERSION);
                return 0;
            }
            else if (OPT_HELP(*argv)) {
                puts(
                    "Usage:\n"
                    "  nocter [options] <file> [arguments]\n"
                    "\n"
                    "Options:\n"
                    "  --check               Check for syntax errors only (no execution)\n"
                    "  --verbose             Show detailed output\n"
                    "  --credits             Show license and third-party credits, then exit\n"
                    "  -h, --help            Show this help message and exit\n"
                    "  -v, --version         Show version information and exit\n"
                );
                return 0;
            }
            else if (OPT_CREDITS(*argv)) {
                puts(
                    "Nocter - A lightweight interpreted language\n"
                    "Copyright (c) 2023-2025 Rvo JP <contact@rvo.jp>\n"
                    "Licensed under the MIT License\n"
                    "\n"
                    "Includes:\n"
                    "- fpconv (BSL-1.0 License): https://github.com/night-shift/fpconv\n"
                );
                return 0;
            }
            else if (OPT_CHECK(*argv)) CHECK = 1;
            else if (OPT_VERBOSE(*argv)) VERBOSE = 1;
            else {
                printf("\e[1;31merror:\e[0;1m unknown option '%s'\e[0m\n", *argv);
                return 1;
            }
        }

        // script
        else {
            ARGS = argv;
            nocter();
            return 0;
        }
    }

    puts("\e[1;31merror:\e[0;1m no script file specified\e[0m");
    return 1;
}
