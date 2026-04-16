/* prototype: parse openclaw prompt header
 * spec: see ~/.neil/tools/autoPrompter/PROMPT_GRAMMAR.md
 *
 * demonstrates:
 *   - header detection on first non-blank line
 *   - flag CSV parse
 *   - key=value parse
 *   - body offset return for downstream pipeline
 *
 * build: cc -O2 -Wall -o /tmp/parse_header /tmp/parse_header.c
 * test:  /tmp/parse_header
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>

#define HDR_PREFIX "#!openclaw:"

typedef struct {
    /* classification flags (mutually exclusive by priority) */
    int f_context, f_test, f_dry_run, f_urgent;
    /* orthogonal flags */
    int f_force, f_quiet, f_no_memory, f_observe_only;
    /* key=value */
    char priority[16];
    int  timeout_sec;
    char tag[64];
    char reply_to[256];
    char parent[64];
    /* output: byte offset of body (first char after header line) */
    size_t body_offset;
    /* output: 1 if header present, 0 if absent/malformed */
    int present;
} prompt_header_t;

static void strtrim(char *s) {
    char *p = s; while (*p && isspace((unsigned char)*p)) p++;
    if (p != s) memmove(s, p, strlen(p)+1);
    size_t n = strlen(s);
    while (n && isspace((unsigned char)s[n-1])) s[--n] = 0;
}

static void set_flag(prompt_header_t *h, const char *name) {
    if      (!strcasecmp(name, "test"))         h->f_test = 1;
    else if (!strcasecmp(name, "force"))        h->f_force = 1;
    else if (!strcasecmp(name, "urgent"))       h->f_urgent = 1;
    else if (!strcasecmp(name, "quiet"))        h->f_quiet = 1;
    else if (!strcasecmp(name, "dry-run"))      h->f_dry_run = 1;
    else if (!strcasecmp(name, "context"))      h->f_context = 1;
    else if (!strcasecmp(name, "observe-only")) h->f_observe_only = 1;
    else if (!strcasecmp(name, "no-memory"))    h->f_no_memory = 1;
    else fprintf(stderr, "warn: unknown flag '%s'\n", name);
}

static void set_kv(prompt_header_t *h, const char *k, const char *v) {
    if      (!strcasecmp(k, "priority")) { strncpy(h->priority, v, 15); }
    else if (!strcasecmp(k, "timeout"))  {
        int n = atoi(v);
        if (strchr(v, 'm')) n *= 60;
        h->timeout_sec = n;
    }
    else if (!strcasecmp(k, "tag"))      { strncpy(h->tag, v, 63); }
    else if (!strcasecmp(k, "reply-to")) { strncpy(h->reply_to, v, 255); }
    else if (!strcasecmp(k, "parent"))   { strncpy(h->parent, v, 63); }
    else fprintf(stderr, "warn: unknown key '%s'\n", k);
}

/* parse one whitespace-delimited token of form flag=csv or key=value */
static void parse_token(prompt_header_t *h, char *tok) {
    char *eq = strchr(tok, '=');
    if (!eq) { fprintf(stderr, "warn: bare token '%s'\n", tok); return; }
    *eq = 0;
    char *key = tok, *val = eq+1;
    if (!strcasecmp(key, "flags")) {
        /* flags=a,b,c */
        char *save, *f = strtok_r(val, ",", &save);
        while (f) { set_flag(h, f); f = strtok_r(NULL, ",", &save); }
    } else {
        set_kv(h, key, val);
    }
}

int parse_prompt_header(const char *buf, size_t buflen, prompt_header_t *h) {
    memset(h, 0, sizeof(*h));
    strcpy(h->priority, "medium");
    h->timeout_sec = 60;

    /* skip leading blank lines to find first content line */
    size_t i = 0, line_start = 0;
    while (i < buflen) {
        line_start = i;
        while (i < buflen && buf[i] != '\n') i++;
        size_t line_len = i - line_start;
        /* check if line is blank (all whitespace) */
        int blank = 1;
        for (size_t j = line_start; j < i; j++)
            if (!isspace((unsigned char)buf[j])) { blank = 0; break; }
        if (i < buflen) i++; /* step past \n */
        if (!blank) {
            /* found first non-blank line -- check for header */
            if (line_len < strlen(HDR_PREFIX)) return 0;
            if (strncasecmp(buf + line_start, HDR_PREFIX, strlen(HDR_PREFIX)) != 0)
                return 0;
            /* extract header content after prefix */
            char line[1024];
            size_t copy = line_len > 1023 ? 1023 : line_len;
            memcpy(line, buf + line_start, copy);
            line[copy] = 0;
            char *rest = line + strlen(HDR_PREFIX);
            strtrim(rest);
            /* tokenize by whitespace */
            char *save, *t = strtok_r(rest, " \t", &save);
            while (t) { parse_token(h, t); t = strtok_r(NULL, " \t", &save); }
            h->present = 1;
            h->body_offset = i;
            return 1;
        }
    }
    return 0;
}

static void print_header(const prompt_header_t *h) {
    printf("present=%d body_offset=%zu\n", h->present, h->body_offset);
    printf("  classification: test=%d context=%d dry_run=%d urgent=%d\n",
           h->f_test, h->f_context, h->f_dry_run, h->f_urgent);
    printf("  orthogonal:     force=%d quiet=%d no_memory=%d observe_only=%d\n",
           h->f_force, h->f_quiet, h->f_no_memory, h->f_observe_only);
    printf("  kv: priority=%s timeout=%d tag=%s reply-to=%s parent=%s\n",
           h->priority, h->timeout_sec, h->tag, h->reply_to, h->parent);
}

static void test_case(const char *label, const char *input) {
    printf("\n--- %s ---\n", label);
    printf("input: %.60s%s\n", input, strlen(input) > 60 ? "..." : "");
    prompt_header_t h;
    parse_prompt_header(input, strlen(input), &h);
    print_header(&h);
}

int main(void) {
    test_case("no header", "just a plain prompt body\nmore text\n");
    test_case("test flag",
        "#!openclaw: flags=test\nverify mcp bash\n");
    test_case("multi-flag",
        "#!openclaw: flags=test,quiet,force tag=mcp-verify\nsmoke test\n");
    test_case("urgent kv",
        "#!openclaw: flags=urgent priority=critical timeout=30s\ndisk full\n");
    test_case("context",
        "#!openclaw: flags=context\noperator prefers terser output\n");
    test_case("leading blank lines",
        "\n\n   \n#!openclaw: flags=dry-run\nplan a refactor\n");
    test_case("malformed (falls back)",
        "#!openclaw garbled flags missing equals\nbody\n");
    test_case("unknown flag (warned, accepted)",
        "#!openclaw: flags=test,nonsense\nbody\n");
    return 0;
}
