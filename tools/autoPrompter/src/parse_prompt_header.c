/* parse_prompt_header.c -- implementation of openclaw prompt header grammar.
 * See parse_prompt_header.h for contract. */

#include "parse_prompt_header.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>

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
    else fprintf(stderr, "parse_prompt_header: warn: unknown flag '%s'\n", name);
}

static void set_kv(prompt_header_t *h, const char *k, const char *v) {
    if      (!strcasecmp(k, "priority")) {
        strncpy(h->priority, v, sizeof(h->priority)-1);
        h->priority[sizeof(h->priority)-1] = 0;
    } else if (!strcasecmp(k, "timeout"))  {
        int n = atoi(v);
        if (strchr(v, 'm')) n *= 60;
        h->timeout_sec = n > 0 ? n : 60;
    } else if (!strcasecmp(k, "tag")) {
        strncpy(h->tag, v, sizeof(h->tag)-1);
        h->tag[sizeof(h->tag)-1] = 0;
    } else if (!strcasecmp(k, "reply-to")) {
        strncpy(h->reply_to, v, sizeof(h->reply_to)-1);
        h->reply_to[sizeof(h->reply_to)-1] = 0;
    } else if (!strcasecmp(k, "parent")) {
        strncpy(h->parent, v, sizeof(h->parent)-1);
        h->parent[sizeof(h->parent)-1] = 0;
    } else {
        fprintf(stderr, "parse_prompt_header: warn: unknown key '%s'\n", k);
    }
}

/* parse one whitespace-delimited token: either flag=csv or key=value */
static void parse_token(prompt_header_t *h, char *tok) {
    char *eq = strchr(tok, '=');
    if (!eq) {
        fprintf(stderr, "parse_prompt_header: warn: bare token '%s'\n", tok);
        return;
    }
    *eq = 0;
    char *key = tok, *val = eq+1;
    if (!strcasecmp(key, "flags")) {
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

    if (!buf || buflen == 0) return 0;

    /* find first non-blank line */
    size_t i = 0, line_start = 0;
    while (i < buflen) {
        line_start = i;
        while (i < buflen && buf[i] != '\n') i++;
        size_t line_len = i - line_start;
        int blank = 1;
        for (size_t j = line_start; j < i; j++)
            if (!isspace((unsigned char)buf[j])) { blank = 0; break; }
        if (i < buflen) i++; /* step past \n */
        if (!blank) {
            size_t plen = strlen(OPENCLAW_HDR_PREFIX);
            if (line_len < plen) return 0;
            if (strncasecmp(buf + line_start, OPENCLAW_HDR_PREFIX, plen) != 0)
                return 0;
            char line[1024];
            size_t copy = line_len > sizeof(line)-1 ? sizeof(line)-1 : line_len;
            memcpy(line, buf + line_start, copy);
            line[copy] = 0;
            char *rest = line + plen;
            strtrim(rest);
            char *save, *t = strtok_r(rest, " \t", &save);
            while (t) { parse_token(h, t); t = strtok_r(NULL, " \t", &save); }
            h->present = 1;
            h->body_offset = i;
            return 1;
        }
    }
    return 0;
}

const char *prompt_header_classification(const prompt_header_t *h) {
    if (!h || !h->present) return "normal";
    if (h->f_context) return "context";
    if (h->f_test)    return "test";
    if (h->f_dry_run) return "dry-run";
    if (h->f_urgent)  return "urgent";
    return "normal";
}
