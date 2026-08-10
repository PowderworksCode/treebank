/* treebank C validity oracle — libclang, parse-only, category-based.
 *
 * What it asserts: "this file contains no C SYNTAX error, judged by clang's
 * parser, in the dialect and include environment the caller supplied."
 * It says nothing about whether the file compiles. See ORACLE.md.
 *
 * Protocol. stdin: one request per line, tab-separated —
 *
 *     <path>[\t<clang arg>]*
 *
 * so the dialect and the -I list live in the caller (and thus in the
 * ledger), not in here. stdout: one JSON object per line, same order.
 *
 * The verdict is computed from clang's own diagnostic categories and
 * nothing else — no message text is ever matched. CXTranslationUnit_KeepGoing
 * is what makes this possible: a missing #include stays non-fatal, so the
 * rest of the file is still parsed and still judged.
 */
#include <clang-c/Index.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* clang's category names, from its own diagnostic tables. */
#define CAT_PARSE "Parse Issue"
#define CAT_SEMA  "Semantic Issue"
#define CAT_LEXPP "Lexical or Preprocessor Issue"
/* #error / #warning. An #error IS the author telling us the configuration is
   unsupported, so it counts as missing context, never as bad syntax. */
#define CAT_USER  "User-Defined Issue"
/* Two further non-syntax categories, both found by the `other` tripwire on a
   real Debian sweep rather than predicted: "incompatible pointer types" and
   friends, and inline-asm constraint errors. Neither is a syntax error, so
   both are counted with the semantic family. */
#define CAT_VALUE "Value Conversion Issue"
#define CAT_ASM   "Inline Assembly Issue"

struct counts {
    unsigned parse, sema, lexpp, userdef, other;
};

static void put_json_string(FILE *out, const char *s) {
    fputc('"', out);
    for (; *s; s++) {
        unsigned char c = (unsigned char)*s;
        switch (c) {
        case '"':  fputs("\\\"", out); break;
        case '\\': fputs("\\\\", out); break;
        case '\n': fputs("\\n", out); break;
        case '\r': fputs("\\r", out); break;
        case '\t': fputs("\\t", out); break;
        default:
            if (c < 0x20) fprintf(out, "\\u%04x", c);
            else fputc(c, out);
        }
    }
    fputc('"', out);
}

/* parse>0 and nothing else wrong => invalid. parse==0 => valid.
   anything else => we cannot tell. */
static const char *verdict_of(const struct counts *c) {
    if (c->parse == 0) return "valid";
    if (c->sema == 0 && c->lexpp == 0 && c->userdef == 0 && c->other == 0)
        return "invalid";
    return "indeterminate";
}

int main(void) {
    CXIndex index = clang_createIndex(/*excludeDeclsFromPCH=*/0,
                                     /*displayDiagnostics=*/0);
    if (!index) {
        fprintf(stderr, "c-oracle: clang_createIndex failed\n");
        return 1;
    }

    char *line = NULL;
    size_t cap = 0;
    ssize_t len;
    while ((len = getline(&line, &cap, stdin)) > 0) {
        while (len > 0 && (line[len - 1] == '\n' || line[len - 1] == '\r'))
            line[--len] = '\0';
        if (len == 0) continue;

        /* Split on tabs: field 0 is the path, the rest are clang args.
           Sized from the line, never fixed: a big package supplies one
           include flag per header-bearing directory and glibc alone has
           498 of them. An earlier fixed cap of 128 silently dropped the
           rest, which quietly under-resolved the three largest packages
           in the corpus. */
        size_t ntabs = 0;
        for (const char *q = line; *q; q++)
            if (*q == '\t') ntabs++;
        char **fields = calloc(ntabs + 2, sizeof *fields);
        const char **argv = calloc(ntabs + 2, sizeof *argv);
        if (!fields || !argv) {
            fprintf(stderr, "c-oracle: out of memory\n");
            free(fields);
            free((void *)argv);
            return 1;
        }
        int nfields = 0;
        for (char *p = line; p;) {
            char *tab = strchr(p, '\t');
            if (tab) *tab = '\0';
            fields[nfields++] = p;
            p = tab ? tab + 1 : NULL;
        }
        if (!fields[0] || !*fields[0]) {
            free(fields);
            free((void *)argv);
            continue;
        }
        const char *path = fields[0];
        int argc = 0;
        for (int i = 1; i < nfields; i++)
            if (*fields[i]) argv[argc++] = fields[i];

        CXTranslationUnit tu = NULL;
        enum CXErrorCode rc = clang_parseTranslationUnit2(
            index, path, argv, argc, NULL, 0,
            CXTranslationUnit_KeepGoing, &tu);
        if (rc != CXError_Success || !tu) {
            fputs("{\"path\":", stdout);
            put_json_string(stdout, path);
            printf(",\"verdict\":\"error\",\"detail\":\"libclang rc=%d\"}\n", rc);
            fflush(stdout);
            if (tu) clang_disposeTranslationUnit(tu);
            free(fields);
            free((void *)argv);
            continue;
        }

        struct counts n = {0, 0, 0, 0, 0};
        char unknown_cat[128] = {0};
        char first_parse[512] = {0};
        unsigned first_parse_line = 0;

        unsigned ndiag = clang_getNumDiagnostics(tu);
        for (unsigned i = 0; i < ndiag; i++) {
            CXDiagnostic d = clang_getDiagnostic(tu, i);
            if (clang_getDiagnosticSeverity(d) < CXDiagnostic_Error) {
                clang_disposeDiagnostic(d);
                continue;
            }
            CXString cat_s = clang_getDiagnosticCategoryText(d);
            const char *cat = clang_getCString(cat_s);
            if (strcmp(cat, CAT_PARSE) == 0) {
                n.parse++;
                if (!first_parse[0]) {
                    CXString msg_s = clang_getDiagnosticSpelling(d);
                    snprintf(first_parse, sizeof first_parse, "%s",
                             clang_getCString(msg_s));
                    clang_disposeString(msg_s);
                    clang_getSpellingLocation(clang_getDiagnosticLocation(d),
                                              NULL, &first_parse_line, NULL, NULL);
                }
            } else if (strcmp(cat, CAT_SEMA) == 0 ||
                       strcmp(cat, CAT_VALUE) == 0 ||
                       strcmp(cat, CAT_ASM) == 0) {
                n.sema++;
            } else if (strcmp(cat, CAT_LEXPP) == 0) {
                n.lexpp++;
            } else if (strcmp(cat, CAT_USER) == 0) {
                n.userdef++;
            } else {
                n.other++;
                if (!unknown_cat[0])
                    snprintf(unknown_cat, sizeof unknown_cat, "%s", cat);
            }
            clang_disposeString(cat_s);
            clang_disposeDiagnostic(d);
        }
        clang_disposeTranslationUnit(tu);

        fputs("{\"path\":", stdout);
        put_json_string(stdout, path);
        fputs(",\"verdict\":\"", stdout);
        fputs(verdict_of(&n), stdout);
        printf("\",\"parse\":%u,\"semantic\":%u,\"lexpp\":%u,"
               "\"userdef\":%u,\"other\":%u",
               n.parse, n.sema, n.lexpp, n.userdef, n.other);
        if (first_parse[0]) {
            fputs(",\"first_parse\":", stdout);
            put_json_string(stdout, first_parse);
            printf(",\"first_parse_line\":%u", first_parse_line);
        }
        if (unknown_cat[0]) {
            fputs(",\"unknown_category\":", stdout);
            put_json_string(stdout, unknown_cat);
        }
        fputs("}\n", stdout);
        fflush(stdout);
        free(fields);
        free((void *)argv);
    }
    free(line);
    clang_disposeIndex(index);
    return 0;
}
