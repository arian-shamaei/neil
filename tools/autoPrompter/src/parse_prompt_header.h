/* parse_prompt_header.h -- openclaw prompt header grammar (v0)
 *
 * Header format (first non-blank line of prompt file, optional):
 *   #!openclaw: flags=<csv> [key=value ...]
 *
 * See ~/.neil/tools/autoPrompter/PROMPT_GRAMMAR.md for the full spec.
 *
 * Contract:
 *   - parse_prompt_header() is pure: no I/O, no allocation.
 *   - On success, h->present=1 and h->body_offset points to the first byte
 *     after the header line's trailing \n.
 *   - On absence or malformation, h->present=0 and h->body_offset=0;
 *     defaults are applied to key=value fields (priority="medium", timeout=60).
 *   - Unknown flags/keys produce a stderr warning but do not fail the parse.
 *
 * Memory model:
 *   - Caller owns the input buffer. No pointers into buf are retained.
 *   - All string fields in prompt_header_t are fixed-size and NUL-terminated.
 *
 * Backward compatibility: absent header => today's autoprompt behavior.
 */

#ifndef PARSE_PROMPT_HEADER_H
#define PARSE_PROMPT_HEADER_H

#include <stddef.h>

#define OPENCLAW_HDR_PREFIX "#!openclaw:"

typedef struct {
    /* classification flags (mutually exclusive by priority) */
    int f_context, f_test, f_dry_run, f_urgent;
    /* orthogonal flags (compose freely) */
    int f_force, f_quiet, f_no_memory, f_observe_only;
    /* key=value options */
    char priority[16];     /* low|medium|high|critical, default "medium" */
    int  timeout_sec;      /* default 60 */
    char tag[64];
    char reply_to[256];
    char parent[64];
    /* output */
    size_t body_offset;    /* byte index of first char after header line */
    int    present;        /* 1 if header parsed, 0 if absent/malformed */
} prompt_header_t;

/* Parse header from in-memory buffer. Returns 1 if header present, 0 otherwise.
 * Always zero-initializes h and applies defaults regardless of return value. */
int parse_prompt_header(const char *buf, size_t buflen, prompt_header_t *h);

/* Return a stable classification label for routing/logging.
 * Priority: context > test > dry-run > urgent > "normal". */
const char *prompt_header_classification(const prompt_header_t *h);

#endif /* PARSE_PROMPT_HEADER_H */
