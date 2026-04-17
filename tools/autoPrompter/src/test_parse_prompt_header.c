/* test_parse_prompt_header.c -- standalone test harness.
 * build: see Makefile target test-parser
 * run:   ./test_parse_prompt_header
 * exits 0 on all-pass, 1 on any failure (assertable by CI / snapshot scripts).
 */

#include "parse_prompt_header.h"

#include <stdio.h>
#include <string.h>
#include <assert.h>

static int failures = 0;

#define EXPECT(cond, msg) do { \
    if (!(cond)) { \
        fprintf(stderr, "FAIL: %s -- %s\n", __func__, msg); \
        failures++; \
    } \
} while (0)

static void t_no_header(void) {
    prompt_header_t h;
    const char *in = "plain body\nmore\n";
    int rc = parse_prompt_header(in, strlen(in), &h);
    EXPECT(rc == 0, "no header should return 0");
    EXPECT(h.present == 0, "present flag clear");
    EXPECT(h.body_offset == 0, "body_offset zero when absent");
    EXPECT(!strcmp(h.priority, "medium"), "priority default");
    EXPECT(h.timeout_sec == 60, "timeout default");
}

static void t_test_flag(void) {
    prompt_header_t h;
    const char *in = "#!openclaw: flags=test\nverify\n";
    int rc = parse_prompt_header(in, strlen(in), &h);
    EXPECT(rc == 1, "header detected");
    EXPECT(h.present == 1, "present set");
    EXPECT(h.f_test == 1, "test flag set");
    EXPECT(h.f_force == 0, "force not set");
    EXPECT(h.body_offset > 0, "body_offset advances");
    EXPECT(!strcmp(in + h.body_offset, "verify\n"), "body extracted correctly");
    EXPECT(!strcmp(prompt_header_classification(&h), "test"), "classified as test");
}

static void t_multi_flag_and_kv(void) {
    prompt_header_t h;
    const char *in = "#!openclaw: flags=test,quiet,force tag=mcp-verify timeout=30s\nsmoke\n";
    parse_prompt_header(in, strlen(in), &h);
    EXPECT(h.f_test && h.f_quiet && h.f_force, "three flags set");
    EXPECT(!strcmp(h.tag, "mcp-verify"), "tag parsed");
    EXPECT(h.timeout_sec == 30, "timeout 30s parsed (numeric prefix)");
}

static void t_timeout_minutes(void) {
    prompt_header_t h;
    const char *in = "#!openclaw: timeout=5m\nlong job\n";
    parse_prompt_header(in, strlen(in), &h);
    EXPECT(h.timeout_sec == 300, "5m -> 300 seconds");
}

static void t_urgent_priority(void) {
    prompt_header_t h;
    const char *in = "#!openclaw: flags=urgent priority=critical\ndisk full\n";
    parse_prompt_header(in, strlen(in), &h);
    EXPECT(h.f_urgent == 1, "urgent flag");
    EXPECT(!strcmp(h.priority, "critical"), "priority=critical");
    EXPECT(!strcmp(prompt_header_classification(&h), "urgent"), "classified urgent");
}

static void t_context_classification(void) {
    prompt_header_t h;
    /* context has higher priority than test in classification */
    const char *in = "#!openclaw: flags=context,test\nnote\n";
    parse_prompt_header(in, strlen(in), &h);
    EXPECT(h.f_context == 1 && h.f_test == 1, "both flags set");
    EXPECT(!strcmp(prompt_header_classification(&h), "context"),
           "context wins over test");
}

static void t_leading_blank_lines(void) {
    prompt_header_t h;
    const char *in = "\n\n   \n#!openclaw: flags=dry-run\nbody\n";
    int rc = parse_prompt_header(in, strlen(in), &h);
    EXPECT(rc == 1, "header found after blank lines");
    EXPECT(h.f_dry_run == 1, "dry-run flag");
}

static void t_malformed_falls_back(void) {
    prompt_header_t h;
    const char *in = "#!openclaw garbled flags missing equals\nbody\n";
    int rc = parse_prompt_header(in, strlen(in), &h);
    /* The colon after openclaw is missing -- prefix check fails */
    EXPECT(rc == 0, "malformed prefix rejected");
    EXPECT(h.present == 0, "not marked present");
}

static void t_empty_buffer(void) {
    prompt_header_t h;
    int rc = parse_prompt_header(NULL, 0, &h);
    EXPECT(rc == 0, "NULL buf safe");
    rc = parse_prompt_header("", 0, &h);
    EXPECT(rc == 0, "empty buf safe");
}

static void t_case_insensitive(void) {
    prompt_header_t h;
    const char *in = "#!OPENCLAW: FLAGS=Test,Force TAG=Mixed\nbody\n";
    int rc = parse_prompt_header(in, strlen(in), &h);
    EXPECT(rc == 1, "uppercase prefix accepted");
    EXPECT(h.f_test && h.f_force, "flags case-insensitive");
    EXPECT(!strcmp(h.tag, "Mixed"), "values case-sensitive (preserved)");
}

static void t_classification_default(void) {
    prompt_header_t h;
    memset(&h, 0, sizeof(h));
    EXPECT(!strcmp(prompt_header_classification(&h), "normal"),
           "no header -> normal");
    EXPECT(!strcmp(prompt_header_classification(NULL), "normal"),
           "NULL safe -> normal");
}

int main(void) {
    t_no_header();
    t_test_flag();
    t_multi_flag_and_kv();
    t_timeout_minutes();
    t_urgent_priority();
    t_context_classification();
    t_leading_blank_lines();
    t_malformed_falls_back();
    t_empty_buffer();
    t_case_insensitive();
    t_classification_default();

    if (failures == 0) {
        printf("all tests passed\n");
        return 0;
    } else {
        fprintf(stderr, "%d failure(s)\n", failures);
        return 1;
    }
}
