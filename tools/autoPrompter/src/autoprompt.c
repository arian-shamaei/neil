/*
 * autoPrompter - inotify-based prompt queue for Claude Code
 *
 * Watches queue/ for new .md files. On IN_CLOSE_WRITE:
 *   1. Move file from queue/ to active/
 *   2. Read prompt content
 *   3. Fork+exec: claude --print -p "<prompt>"
 *   4. Capture stdout/stderr and exit code
 *   5. Write result file and move prompt to history/
 *
 * Directory layout:
 *   queue/    - drop prompt .md files here
 *   active/   - currently executing (at most one)
 *   history/  - completed prompts + result files
 */

#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include <time.h>
#include <dirent.h>
#include <signal.h>
#include <sys/inotify.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <sys/select.h>
#include <sys/file.h>
#include <fcntl.h>
#include <ctype.h>

#define MAX_PATH    4096
#define MAX_LINE    4096
#define MAX_PROMPT  (1024 * 1024)  /* 1 MB max prompt */
#define READ_BUF    (1024 * 64)
#define DEDUP_WINDOW_SEC  300     /* 5 min dedup window */
#define DEDUP_LOG_MAX     50      /* keep last N dedup entries */

#define AI_CMD_DEFAULT "claude"
#define NEIL_HOME_DEFAULT ".neil"

static char g_ai_command[MAX_PATH];
static char g_ai_args[MAX_PATH];
static char g_ai_system_flag[64];
static char g_ai_prompt_flag[16];
static int g_max_react_turns = 3;
static int g_claude_timeout = 300;
static int g_agent_manages_stream = 0;
static int g_neil_os_enabled = 1;  /* observable kill-switch flag */  /* 1 = external agent writes .neil_stream */  /* seconds, 0 = no timeout */

/* Resolved paths -- set once at startup from NEIL_HOME env var */
static char g_neil_home[MAX_PATH];
static char g_queue_dir[MAX_PATH];
static char g_active_dir[MAX_PATH];
static char g_history_dir[MAX_PATH];
static char g_zettel_bin[MAX_PATH];
static char g_mempalace_venv[MAX_PATH];
static char g_mempalace_palace[MAX_PATH];
static char g_services_registry[MAX_PATH];
static char g_services_vault[MAX_PATH];
static char g_essence_dir[MAX_PATH];
static char g_observe_sh[MAX_PATH];
static char g_heartbeat_log[MAX_PATH];

static volatile sig_atomic_t g_running = 1;
static volatile sig_atomic_t g_processing = 0;  /* 1 when mid-prompt execution */
static char g_current_prompt_name[256] = "";     /* filename of prompt being processed */

/* Forward declarations */
static void timestamp_now(char *buf, size_t cap);
static void stream_action(const char *prefix, const char *detail, const char *cmd, const char *output);
static void log_internal_failure(const char *source, const char *severity, const char *context, const char *error);
static void set_seal_pose(const char *eyes, const char *mouth, const char *body, const char *indicator, const char *label);static void stream_write(const char *data, size_t len);static int run_claude(const char *prompt, const char *system_prompt, char **out, size_t *out_len);static void extract_memories(const char *output);
static char *execute_tool_actions(const char *output);
static unsigned long djb2_hash(const char *str, size_t len);

/*
 * sh_escape_sq — escape single quotes in a string so the result can be
 * safely embedded inside single-quoted bash literals. Replaces every '
 * with the classic close-escape-reopen sequence '\''.
 *
 * Needed when building NEIL_PARAMS='...' for handler.sh dispatch: if the
 * agent emits a CALL whose message contains apostrophes (e.g. "isn't")
 * or parens, bash parses the unescaped single-quote as the end of the
 * literal, then the rest of the message becomes shell code — causing
 * "Syntax error: '(' unexpected" when parens appear after an apostrophe.
 *
 * Returns a newly-malloc'd string; caller must free.
 */
static char *sh_escape_sq(const char *in) {
    if (!in) return strdup("");
    size_t n = strlen(in);
    /* Worst case: every byte is a quote → each expands to 4 bytes. */
    char *out = malloc(n * 4 + 1);
    if (!out) return NULL;
    char *q = out;
    for (const char *p = in; *p; p++) {
        if (*p == '\'') {
            *q++ = '\''; *q++ = '\\'; *q++ = '\''; *q++ = '\'';
        } else {
            *q++ = *p;
        }
    }
    *q = '\0';
    return out;
}

static int dedup_check(unsigned long hash);
static void dedup_record(unsigned long hash, const char *filename);

/*
 * Resolve all paths from NEIL_HOME environment variable.
 * Priority: NEIL_HOME env > $HOME/.neil > /tmp/.neil
 */
static void resolve_neil_paths(void) {
    const char *nh = getenv("NEIL_HOME");
    if (nh && *nh) {
        snprintf(g_neil_home, sizeof(g_neil_home), "%s", nh);
    } else {
        const char *home = getenv("HOME");
        if (!home) home = "/tmp";
        snprintf(g_neil_home, sizeof(g_neil_home), "%s/%s", home, NEIL_HOME_DEFAULT);
    }

    snprintf(g_zettel_bin, sizeof(g_zettel_bin),
        "%s/memory/zettel/zettel", g_neil_home);
    snprintf(g_mempalace_venv, sizeof(g_mempalace_venv),
        "%s/memory/mempalace/.venv/bin/activate", g_neil_home);
    snprintf(g_mempalace_palace, sizeof(g_mempalace_palace),
        "%s/memory/palace/.mempalace", g_neil_home);
    snprintf(g_services_registry, sizeof(g_services_registry),
        "%s/services/registry", g_neil_home);
    snprintf(g_services_vault, sizeof(g_services_vault),
        "%s/services/vault", g_neil_home);
    snprintf(g_essence_dir, sizeof(g_essence_dir),
        "%s/essence", g_neil_home);
    snprintf(g_observe_sh, sizeof(g_observe_sh),
        "%s/tools/autoPrompter/observe.sh", g_neil_home);
    snprintf(g_heartbeat_log, sizeof(g_heartbeat_log),
        "%s/heartbeat_log.json", g_neil_home);

    snprintf(g_queue_dir, sizeof(g_queue_dir),
        "%s/tools/autoPrompter/queue", g_neil_home);
    snprintf(g_active_dir, sizeof(g_active_dir),
        "%s/tools/autoPrompter/active", g_neil_home);
    snprintf(g_history_dir, sizeof(g_history_dir),
        "%s/tools/autoPrompter/history", g_neil_home);

    /* Also set ZETTEL_HOME if not already set */
    if (!getenv("ZETTEL_HOME")) {
        char zettel_home[MAX_PATH];
        snprintf(zettel_home, sizeof(zettel_home),
            "%s/memory/palace", g_neil_home);
        setenv("ZETTEL_HOME", zettel_home, 0);
    }

    /* Load AI config from config.toml */
    snprintf(g_ai_command, sizeof(g_ai_command), "%s", AI_CMD_DEFAULT);
    snprintf(g_ai_args, sizeof(g_ai_args),
        "--print --output-format text --dangerously-skip-permissions");
    snprintf(g_ai_system_flag, sizeof(g_ai_system_flag), "--system-prompt");
    snprintf(g_ai_prompt_flag, sizeof(g_ai_prompt_flag), "-p");

    char config_path[MAX_PATH];
    snprintf(config_path, sizeof(config_path), "%s/config.toml", g_neil_home);

    FILE *cfg = fopen(config_path, "r");
    if (cfg) {
        char line[MAX_LINE];
        while (fgets(line, sizeof(line), cfg)) {
            char *eq = strchr(line, '=');
            if (!eq) continue;
            *eq = '\0';
            char *key = line;
            char *val = eq + 1;
            /* trim whitespace and quotes */
            while (*key == ' ' || *key == '\t') key++;
            {
                size_t klen = strlen(key);
                while (klen > 0 && (key[klen-1] == ' ' || key[klen-1] == '\t'))
                    key[--klen] = '\0';
            }
            while (*val == ' ' || *val == '"' || *val == '\'') val++;
            size_t vlen = strlen(val);
            while (vlen > 0 && (val[vlen-1] == '\n' || val[vlen-1] == '"' ||
                   val[vlen-1] == '\'' || val[vlen-1] == ' '))
                val[--vlen] = '\0';

            if (strcmp(key, "command") == 0)
                snprintf(g_ai_command, sizeof(g_ai_command), "%s", val);
            else if (strcmp(key, "args") == 0)
                snprintf(g_ai_args, sizeof(g_ai_args), "%s", val);
            else if (strcmp(key, "system_prompt_flag") == 0)
                snprintf(g_ai_system_flag, sizeof(g_ai_system_flag), "%s", val);
            else if (strcmp(key, "prompt_flag") == 0)
                snprintf(g_ai_prompt_flag, sizeof(g_ai_prompt_flag), "%s", val);
            else if (strcmp(key, "max_react_turns") == 0)
                g_max_react_turns = atoi(val);
            else if (strcmp(key, "agent_manages_stream") == 0)
                g_agent_manages_stream = atoi(val);
            else if (strcmp(key, "neil_os_enabled") == 0)
                g_neil_os_enabled = (strcmp(val, "true") == 0 || strcmp(val, "1") == 0) ? 1 : 0;
            else if (strcmp(key, "claude_timeout") == 0)
                g_claude_timeout = atoi(val);
        }
        fclose(cfg);
        fprintf(stderr, "[autoprompt] config: ai=%s prompt=%s system=%s\n",
                g_ai_command, g_ai_prompt_flag, g_ai_system_flag);
        fprintf(stderr, "[autoprompt] config loaded: neil_os_enabled=%d\n", g_neil_os_enabled);
    }
}

static void handle_signal(int sig) {
    (void)sig;
    g_running = 0;

    /* If we're mid-prompt, log a killed heartbeat so we don't get "unknown" status */
    if (g_processing && g_heartbeat_log[0] && g_current_prompt_name[0]) {
        char ts[64];
        time_t now = time(NULL);
        struct tm *tm = localtime(&now);
        if (tm) {
            snprintf(ts, sizeof(ts), "%04d-%02d-%02dT%02d-%02d-%02d",
                     tm->tm_year + 1900, tm->tm_mon + 1, tm->tm_mday,
                     tm->tm_hour, tm->tm_min, tm->tm_sec);
        } else {
            snprintf(ts, sizeof(ts), "unknown");
        }

        char entry[512];
        int elen = snprintf(entry, sizeof(entry),
            "{\"timestamp\":\"%s\",\"prompt\":\"%s\",\"status\":\"killed\",\"summary\":\"SIGTERM received mid-execution\"}\n",
            ts, g_current_prompt_name);

        int fd = open(g_heartbeat_log, O_WRONLY | O_CREAT | O_APPEND, 0644);
        if (fd >= 0) {
            ssize_t w = write(fd, entry, (size_t)elen);
            (void)w;
            close(fd);
        }
    }
}

/* Read entire file into malloc'd buffer. Returns NULL on error. */
static char *read_file(const char *path, size_t *out_len) {
    int fd = open(path, O_RDONLY);
    if (fd < 0) return NULL;

    size_t cap = 4096, len = 0;
    char *buf = malloc(cap);
    if (!buf) { close(fd); return NULL; }

    ssize_t n;
    while ((n = read(fd, buf + len, cap - len)) > 0) {
        len += (size_t)n;
        if (len == cap) {
            cap *= 2;
            if (cap > MAX_PROMPT) { free(buf); close(fd); return NULL; }
            char *tmp = realloc(buf, cap);
            if (!tmp) { free(buf); close(fd); return NULL; }
            buf = tmp;
        }
    }
    close(fd);

    if (n < 0) { free(buf); return NULL; }
    buf[len] = '\0';
    if (out_len) *out_len = len;
    return buf;
}

/* Write buffer to file atomically (write to .tmp, fsync, rename). */
static int write_file_atomic(const char *path, const char *data, size_t len) {
    char tmp[MAX_PATH];
    snprintf(tmp, sizeof(tmp), "%s.tmp.%d", path, getpid());

    int fd = open(tmp, O_WRONLY | O_CREAT | O_EXCL, 0644);
    if (fd < 0) return -1;

    size_t written = 0;
    while (written < len) {
        ssize_t n = write(fd, data + written, len - written);
        if (n < 0) {
            if (errno == EINTR) continue;
            close(fd); unlink(tmp); return -1;
        }
        written += (size_t)n;
    }

    if (fsync(fd) < 0) { close(fd); unlink(tmp); return -1; }
    close(fd);

    if (rename(tmp, path) < 0) { unlink(tmp); return -1; }
    return 0;
}

/* Escape single quotes for safe embedding in shell single-quoted strings.
 * Replaces ' with '\'' (end quote, escaped quote, restart quote).
 * Writes into dst (up to dstsz-1 bytes). Always NUL-terminates. */
static void shell_escape_sq(char *dst, size_t dstsz, const char *src) {
    size_t di = 0;
    for (size_t i = 0; src[i] && di < dstsz - 4; i++) {
        if (src[i] == '\'') {
            dst[di++] = '\''; dst[di++] = '\\';
            dst[di++] = '\''; dst[di++] = '\'';
        } else {
            dst[di++] = src[i];
        }
    }
    dst[di] = '\0';
}

/* Expand leading ~ to HOME directory in a path.
 * Writes into dst (up to dstsz-1 bytes). Always NUL-terminates. */
static void expand_tilde(char *dst, size_t dstsz, const char *src) {
    if (src[0] == '~' && (src[1] == '/' || src[1] == '\0')) {
        const char *home = getenv("HOME");
        if (!home) home = "/tmp";
        snprintf(dst, dstsz, "%s%s", home, src + 1);
    } else {
        snprintf(dst, dstsz, "%s", src);
    }
}

/* Run a shell command, capture stdout into malloc'd buffer. */
static char *run_command(const char *cmd) {
    FILE *fp = popen(cmd, "r");
    if (!fp) return NULL;

    size_t cap = 4096, len = 0;
    char *buf = malloc(cap);
    if (!buf) { pclose(fp); return NULL; }

    size_t n;
    while ((n = fread(buf + len, 1, cap - len - 1, fp)) > 0) {
        len += n;
        if (len >= cap - 1) {
            cap *= 2;
            char *tmp = realloc(buf, cap);
            if (!tmp) { free(buf); pclose(fp); return NULL; }
            buf = tmp;
        }
    }
    pclose(fp);
    buf[len] = '\0';
    return buf;
}

/* Like run_command but also returns exit status of the child via *status_out.
 * status_out is set to -1 if popen/pclose fails or the child didn't exit
 * normally; otherwise it's the child's exit code (0 = success). */
static char *run_command_status(const char *cmd, int *status_out) {
    FILE *fp = popen(cmd, "r");
    if (!fp) { if (status_out) *status_out = -1; return NULL; }

    size_t cap = 4096, len = 0;
    char *buf = malloc(cap);
    if (!buf) { pclose(fp); if (status_out) *status_out = -1; return NULL; }

    size_t n;
    while ((n = fread(buf + len, 1, cap - len - 1, fp)) > 0) {
        len += n;
        if (len >= cap - 1) {
            cap *= 2;
            char *tmp = realloc(buf, cap);
            if (!tmp) { free(buf); pclose(fp); if (status_out) *status_out = -1; return NULL; }
            buf = tmp;
        }
    }
    int pc = pclose(fp);
    if (status_out) {
        if (pc == -1 || !WIFEXITED(pc)) *status_out = -1;
        else *status_out = WEXITSTATUS(pc);
    }
    buf[len] = '\0';
    return buf;
}


/* Extract a meaningful search query from the prompt for mempalace lookup.
 *
 * For heartbeat prompts: reads the last heartbeat_log.json entry and pulls
 * the "question" field (or "summary" as fallback) -- so each heartbeat
 * retrieves contextually relevant memories instead of always "# Heartbeat".
 *
 * For other prompts: extracts the first meaningful line (skipping markdown
 * headers that start with #) up to cap-1 chars.
 */
static void extract_query(const char *prompt, char *query, size_t cap) {
    /* skip whitespace */
    const char *p = prompt;
    while (*p && isspace((unsigned char)*p)) p++;

    /* Detect heartbeat prompts */
    if (strncmp(p, "# Heartbeat", 11) == 0) {
        /* Try to extract last question/summary from heartbeat log */
        char cmd[MAX_PATH + 64];
        snprintf(cmd, sizeof(cmd),
            "tail -1 %s 2>/dev/null", g_heartbeat_log);
        FILE *fp = popen(cmd, "r");
        if (fp) {
            char line[2048];
            if (fgets(line, sizeof(line), fp)) {
                /* Try "question" field first, then "summary" */
                const char *field = NULL;
                const char *qstart = strstr(line, "\"question\":\"");
                if (qstart) {
                    field = qstart + 12;
                } else {
                    qstart = strstr(line, "\"summary\":\"");
                    if (qstart) field = qstart + 11;
                }
                if (field) {
                    size_t i;
                    for (i = 0; i < cap - 1 && field[i] && field[i] != '"'; i++)
                        query[i] = field[i];
                    query[i] = '\0';
                    /* If extracted field is empty (e.g. after SIGTERM kill),
                       fall back to summary field instead of returning empty */
                    if (query[0] != '\0') {
                        pclose(fp);
                        return;
                    }
                    /* Try summary as fallback if question was empty */
                    if (qstart != strstr(line, "\"summary\":\"")) {
                        const char *sstart = strstr(line, "\"summary\":\"");
                        if (sstart) {
                            const char *sf = sstart + 11;
                            for (i = 0; i < cap - 1 && sf[i] && sf[i] != '"'; i++)
                                query[i] = sf[i];
                            query[i] = '\0';
                            if (query[0] != '\0') {
                                pclose(fp);
                                return;
                            }
                        }
                    }
                }
            }
            pclose(fp);
        }
    }

    /* Default: skip markdown header lines (# ...) to find content */
    while (*p == '#') {
        while (*p && *p != '\n') p++;
        while (*p && isspace((unsigned char)*p)) p++;
    }

    /* If we skipped everything, fall back to original prompt */
    if (!*p) p = prompt;
    while (*p && isspace((unsigned char)*p)) p++;

    size_t i;
    for (i = 0; i < cap - 1 && p[i] && p[i] != '\n'; i++)
        query[i] = p[i];
    query[i] = '\0';
}

/* Load all .md files from essence/ dir, concatenated. */
static char *load_essence(void) {
    DIR *d = opendir(g_essence_dir);
    if (!d) return NULL;

    size_t cap = 8192, len = 0;
    char *buf = malloc(cap);
    if (!buf) { closedir(d); return NULL; }
    buf[0] = '\0';

    struct dirent *ent;
    while ((ent = readdir(d)) != NULL) {
        size_t nlen = strlen(ent->d_name);
        if (nlen <= 3 || strcmp(ent->d_name + nlen - 3, ".md") != 0)
            continue;

        char path[MAX_PATH];
        snprintf(path, sizeof(path), "%s/%s", g_essence_dir, ent->d_name);

        size_t flen;
        char *content = read_file(path, &flen);
        if (!content) continue;

        /* grow buffer if needed */
        while (len + flen + 4 > cap) {
            cap *= 2;
            char *tmp = realloc(buf, cap);
            if (!tmp) { free(content); continue; }
            buf = tmp;
        }

        memcpy(buf + len, content, flen);
        len += flen;
        buf[len++] = '\n';
        buf[len++] = '\n';
        buf[len] = '\0';
        free(content);
    }
    closedir(d);

    if (len == 0) { free(buf); return NULL; }
    return buf;
}

/* Build augmented prompt with zettel context + mempalace results.
 * Essence is returned separately via out_essence for --system-prompt. */
static char *build_augmented_prompt(const char *raw_prompt, char **out_essence, const char *prompt_file) {
    /* 1. Load essence (returned separately for --system-prompt) */
    *out_essence = load_essence();

    /* 2. Run observation layer */
    char obs_cmd[MAX_PATH];
    snprintf(obs_cmd, sizeof(obs_cmd), "%s 2>/dev/null", g_observe_sh);
    char *observations = run_command(obs_cmd);

    /* 3. Get short-term memory (last 3 heartbeat log entries) */
    char stm_cmd[MAX_PATH];
    snprintf(stm_cmd, sizeof(stm_cmd), "tail -3 %s 2>/dev/null", g_heartbeat_log);
    char *short_term = run_command(stm_cmd);

    /* 4. Get relevant memories from mempalace */
    char query[128];
    extract_query(raw_prompt, query, sizeof(query));

    char escaped[256];
    size_t ei = 0;
    for (size_t i = 0; query[i] && ei < sizeof(escaped) - 4; i++) {
        if (query[i] == '\'') {
            escaped[ei++] = '\''; escaped[ei++] = '\\';
            escaped[ei++] = '\''; escaped[ei++] = '\'';
        } else {
            escaped[ei++] = query[i];
        }
    }
    escaped[ei] = '\0';

    char search_cmd[2048];
    snprintf(search_cmd, sizeof(search_cmd),
        "%s/tools/autoPrompter/multi_search.sh '%s' '%s' '%s' '%s' 2>/dev/null",
        g_neil_home, g_mempalace_palace, g_mempalace_venv, escaped,
        prompt_file ? prompt_file : "");
    char *memories = run_command(search_cmd);

    /* 4b. Reinforce found memories (memory decay system) */
    if (memories && memories[0]) {
        char reinforce_cmd[2048];
        snprintf(reinforce_cmd, sizeof(reinforce_cmd),
            "echo '%s' | %s/self/reinforce_from_search.sh 2>/dev/null",
            "$(echo \"$MEMORIES\" | head -20)", g_neil_home);
        /* Write memories to temp file for safe parsing (avoids shell escaping) */
        char tmpfile[MAX_PATH];
        snprintf(tmpfile, sizeof(tmpfile), "/tmp/neil_memresults_%d", getpid());
        FILE *tf = fopen(tmpfile, "w");
        if (tf) {
            fputs(memories, tf);
            fclose(tf);
            snprintf(reinforce_cmd, sizeof(reinforce_cmd),
                "%s/self/reinforce_from_search.sh < %s 2>/dev/null; rm -f %s",
                g_neil_home, tmpfile, tmpfile);
            char *r = run_command(reinforce_cmd);
            if (r) free(r);
        }
    }

    /* 5. Build augmented prompt */
    size_t raw_len = strlen(raw_prompt);
    size_t obs_len = observations ? strlen(observations) : 0;
    size_t stm_len = short_term ? strlen(short_term) : 0;
    size_t mem_len = memories ? strlen(memories) : 0;

    size_t aug_cap = raw_len + obs_len + stm_len + mem_len + 2048;
    char *aug = malloc(aug_cap);
    if (!aug) {
        free(observations); free(short_term); free(memories);
        return strdup(raw_prompt);
    }

    int off = 0;
    off += snprintf(aug + off, aug_cap - off,
        "[SYSTEM]\n"
        "Output formats for actions:\n"
        "  MEMORY: wing=<domain> room=<topic> tags=<t1,t2> | <what to remember>\n"
        "  CALL: service=<name> action=<action> [param=value ...]\n"
        "  PROMPT: <your next task or follow-up question>\n\n");

    if (observations && observations[0]) {
        off += snprintf(aug + off, aug_cap - off,
            "[OBSERVATIONS]\n%s\n\n", observations);
    }

    if (short_term && short_term[0]) {
        off += snprintf(aug + off, aug_cap - off,
            "[RECENT ACTIVITY]\n%s\n\n", short_term);
    }

    if (memories && memories[0]) {
        off += snprintf(aug + off, aug_cap - off,
            "[RELEVANT MEMORIES]\n%s\n\n", memories);
    }

    off += snprintf(aug + off, aug_cap - off,
        "[PROMPT]\n%s", raw_prompt);

    free(observations);
    free(short_term);
    free(memories);
    return aug;
}

/* Parse MEMORY: lines from Claude output and store via zettel. */
static void extract_memories(const char *output) {
    if (!output) return;

    const char *p = output;
    while ((p = strstr(p, "MEMORY:")) != NULL) {
        /* Check it's at line start or after newline */
        if (p != output && *(p - 1) != '\n') { p++; continue; }

        p += 7; /* skip "MEMORY:" */
        while (*p == ' ') p++;

        /* Parse wing=X room=Y tags=Z | body */
        char wing[64] = "", room[64] = "", tags[256] = "", body[2048] = "";

        const char *eol = strchr(p, '\n');
        if (!eol) eol = p + strlen(p);

        char line[4096];
        size_t ll = (size_t)(eol - p);
        if (ll >= sizeof(line)) ll = sizeof(line) - 1;
        memcpy(line, p, ll);
        line[ll] = '\0';

        /* Find the | separator */
        char *pipe = strchr(line, '|');
        if (!pipe) {
            fprintf(stderr, "[autoprompt] NOTIFY: missing '|' separator, skipping\n");
            log_internal_failure("notify-dispatch", "low",
                "missing-pipe", line);
            p = eol; continue;
        }

        /* Body is after | */
        char *b = pipe + 1;
        while (*b == ' ') b++;
        snprintf(body, sizeof(body), "%s", b);

        /* Parse key=value pairs before | */
        *pipe = '\0';
        char *tok = strtok(line, " ");
        while (tok) {
            if (strncmp(tok, "wing=", 5) == 0)
                snprintf(wing, sizeof(wing), "%s", tok + 5);
            else if (strncmp(tok, "room=", 5) == 0)
                snprintf(room, sizeof(room), "%s", tok + 5);
            else if (strncmp(tok, "tags=", 5) == 0)
                snprintf(tags, sizeof(tags), "%s", tok + 5);
            tok = strtok(NULL, " ");
        }

        if (body[0]) {
            /* Escape single quotes in body */
            char esc_body[4096];
            size_t ei = 0;
            for (size_t i = 0; body[i] && ei < sizeof(esc_body) - 4; i++) {
                if (body[i] == '\'') {
                    esc_body[ei++] = '\''; esc_body[ei++] = '\\';
                    esc_body[ei++] = '\''; esc_body[ei++] = '\'';
                } else {
                    esc_body[ei++] = body[i];
                }
            }
            esc_body[ei] = '\0';

            char cmd[8192];
            int n = snprintf(cmd, sizeof(cmd), "%s new '%s'", g_zettel_bin, esc_body);
            if (wing[0]) n += snprintf(cmd + n, sizeof(cmd) - n, " --wing '%s'", wing);
            if (room[0]) n += snprintf(cmd + n, sizeof(cmd) - n, " --room '%s'", room);
            if (tags[0]) n += snprintf(cmd + n, sizeof(cmd) - n, " --tags '%s'", tags);

            char *result = run_command(cmd);
            if (result) {
                fprintf(stderr, "[autoprompt] stored memory: %s", result);
                /* Stream the memory write so TUI shows it */
                {
                    char mem_detail[512];
                    snprintf(mem_detail, sizeof(mem_detail), "wing=%s room=%s | %.200s",
                             wing[0] ? wing : "default", room[0] ? room : "inbox",
                             body);
                    stream_action("MEMORY", mem_detail, cmd, result);
                }
                free(result);
            }
        }
        p = eol;
    }
}

/* Re-index mempalace after storing new memories. */
static void reindex_mempalace(void) {
    char cmd[1024];
    snprintf(cmd, sizeof(cmd),
        ". %s && mempalace --palace %s mine %s/memory/palace/notes/ 2>/dev/null",
        g_mempalace_venv, g_mempalace_palace, g_neil_home);
    char *result = run_command(cmd);
    if (result) {
        fprintf(stderr, "[autoprompt] mempalace reindexed\n");
        stream_action(NULL, NULL, "mempalace mine", result);
        free(result);
    }
}

/* Append a single failure entry to failures.json from inside autoprompter.
 * Mirrors the JSON schema written by record_failures() for FAIL: lines. */
static void log_internal_failure(const char *source, const char *severity,
                                  const char *context, const char *error) {
    char fail_path[MAX_PATH];
    snprintf(fail_path, sizeof(fail_path), "%s/self/failures.json", g_neil_home);

    char esc_err[2048];
    size_t ei = 0;
    const char *e = error ? error : "";
    for (size_t i = 0; e[i] && ei < sizeof(esc_err) - 2; i++) {
        if (e[i] == '"' || e[i] == '\\') esc_err[ei++] = '\\';
        esc_err[ei++] = e[i];
    }
    esc_err[ei] = '\0';

    char esc_ctx[512];
    size_t ci = 0;
    const char *c = context ? context : "";
    for (size_t i = 0; c[i] && ci < sizeof(esc_ctx) - 2; i++) {
        if (c[i] == '"' || c[i] == '\\') esc_ctx[ci++] = '\\';
        esc_ctx[ci++] = c[i];
    }
    esc_ctx[ci] = '\0';

    char ts[32];
    timestamp_now(ts, sizeof(ts));

    char entry[4096];
    int elen = snprintf(entry, sizeof(entry),
        "{\"timestamp\":\"%s\",\"source\":\"%s\",\"error\":\"%s\","
        "\"context\":\"%s\",\"severity\":\"%s\",\"resolution\":\"pending\",\"notes\":\"\"}\n",
        ts, source ? source : "unknown", esc_err, esc_ctx,
        severity ? severity : "medium");

    int fd = open(fail_path, O_WRONLY | O_CREAT | O_APPEND, 0644);
    if (fd >= 0) {
        ssize_t w = write(fd, entry, (size_t)elen);
        (void)w;
        close(fd);
    }
    fprintf(stderr, "[autoprompt] FAIL: [%s] %s: %s\n",
            severity ? severity : "medium",
            source ? source : "unknown", esc_err);
}

/* Parse NOTIFY: lines from output and dispatch to channel scripts. */
static void dispatch_notifications(const char *output) {
    if (!output) return;

    const char *p = output;
    while ((p = strstr(p, "NOTIFY:")) != NULL) {
        if (p != output && *(p - 1) != '\n') { p++; continue; }

        p += 7; /* skip "NOTIFY:" */
        while (*p == ' ') p++;

        const char *eol = strchr(p, '\n');
        if (!eol) eol = p + strlen(p);

        char line[4096];
        size_t ll = (size_t)(eol - p);
        if (ll >= sizeof(line)) ll = sizeof(line) - 1;
        memcpy(line, p, ll);
        line[ll] = '\0';

        /* Find the | separator */
        char *pipe = strchr(line, '|');
        if (!pipe) {
            fprintf(stderr, "[autoprompt] NOTIFY: missing '|' separator, skipping\n");
            log_internal_failure("notify-dispatch", "low",
                "missing-pipe", line);
            p = eol; continue;
        }

        char *message = pipe + 1;
        while (*message == ' ') message++;
        *pipe = '\0';

        /* Parse channel= and other params */
        char channel[64] = "";
        char params_raw[1024] = "";
        size_t pi = 0;

        char *tok = strtok(line, " ");
        while (tok) {
            if (strncmp(tok, "channel=", 8) == 0)
                snprintf(channel, sizeof(channel), "%s", tok + 8);
            else if (strchr(tok, '='))
                pi += snprintf(params_raw + pi, sizeof(params_raw) - pi,
                    "%s%s", pi > 0 ? " " : "", tok);
            tok = strtok(NULL, " ");
        }

        if (!channel[0]) { p = eol; continue; }

        /* Check channel script exists */
        char ch_path[MAX_PATH];
        snprintf(ch_path, sizeof(ch_path), "%s/outputs/channels/%s.sh",
                 g_neil_home, channel);
        if (access(ch_path, X_OK) != 0) {
            fprintf(stderr, "[autoprompt] NOTIFY: unknown channel '%s'\n", channel);
            log_internal_failure("notify-dispatch", "low",
                channel, "unknown channel (no script at outputs/channels)");
            p = eol; continue;
        }

        /* Escape values for safe shell embedding */
        char esc_channel[128], esc_message[4096], esc_params[2048];
        shell_escape_sq(esc_channel, sizeof(esc_channel), channel);
        shell_escape_sq(esc_message, sizeof(esc_message), message);
        shell_escape_sq(esc_params, sizeof(esc_params), params_raw);

        /* Parse individual params as NEIL_PARAM_<key> env vars */
        char cmd[8192];
        int n = snprintf(cmd, sizeof(cmd),
            "NEIL_CHANNEL='%s' NEIL_MESSAGE='%s' NEIL_PARAMS='%s' ",
            esc_channel, esc_message, esc_params);

        /* Parse key=value into NEIL_PARAM_key=value */
        char params_copy[1024];
        snprintf(params_copy, sizeof(params_copy), "%s", params_raw);
        char *ptok = strtok(params_copy, " ");
        while (ptok) {
            char *eq = strchr(ptok, '=');
            if (eq) {
                *eq = '\0';
                char esc_val[512];
                shell_escape_sq(esc_val, sizeof(esc_val), eq + 1);
                n += snprintf(cmd + n, sizeof(cmd) - n,
                    "NEIL_PARAM_%s='%s' ", ptok, esc_val);
            }
            ptok = strtok(NULL, " ");
        }

        n += snprintf(cmd + n, sizeof(cmd) - n, "%s 2>&1", ch_path);

        fprintf(stderr, "[autoprompt] NOTIFY: channel=%s\n", channel);
        int notify_status = 0;
        char *result = run_command_status(cmd, &notify_status);
        if (result) free(result);
        if (notify_status != 0) {
            char ctx[128];
            snprintf(ctx, sizeof(ctx), "%s exit=%d", channel, notify_status);
            log_internal_failure("notify-dispatch", "low",
                ctx, "channel script returned non-zero");
        }

        p = eol;
    }
}

/* Parse INTEND: lines and append to intentions.json */
/* Helper: extract a key=value token from a space-separated line.
 * Returns pointer just past the match or NULL if not found.
 * Writes value into out (max outsz bytes). */
static const char *extract_token(const char *line, const char *key, char *out, size_t outsz) {
    size_t klen = strlen(key);
    const char *p = line;
    while (*p) {
        while (*p == ' ' || *p == '\t') p++;
        if (strncmp(p, key, klen) == 0 && p[klen] == '=') {
            p += klen + 1;
            size_t i = 0;
            while (*p && *p != ' ' && *p != '\t' && i < outsz - 1)
                out[i++] = *p++;
            out[i] = '\0';
            return p;
        }
        /* advance past this token */
        while (*p && *p != ' ' && *p != '\t') p++;
    }
    return NULL;
}

static void record_intentions(const char *output) {
    if (!output) return;

    char intentions_path[MAX_PATH];
    snprintf(intentions_path, sizeof(intentions_path),
        "%s/intentions.json", g_neil_home);

    const char *p = output;
    while ((p = strstr(p, "INTEND:")) != NULL) {
        if (p != output && *(p - 1) != '\n') { p++; continue; }

        p += 7;
        while (*p == ' ') p++;

        const char *eol = strchr(p, '\n');
        if (!eol) eol = p + strlen(p);

        char line[4096];
        size_t ll = (size_t)(eol - p);
        if (ll >= sizeof(line)) ll = sizeof(line) - 1;
        memcpy(line, p, ll);
        line[ll] = '\0';

        /* Find | separator */
        char *pipe = strchr(line, '|');
        if (!pipe) { p = eol; continue; }

        char *description = pipe + 1;
        while (*description == ' ') description++;
        *pipe = '\0';

        /* Parse all known key= tokens (backward compatible; unknown tokens ignored) */
        char priority[32] = "medium";
        char after[32] = "";
        char tag[64] = "";
        char verify[MAX_PATH] = "";
        char max_beats[16] = "";
        char max_tokens_s[16] = "";
        char max_sec[16] = "";
        char max_cost[16] = "";
        char lifecycle[16] = "persistent";
        char memory_mode[16] = "full";
        char persona[32] = "default";
        char model_hint[16] = "auto";
        char target[32] = "main";
        char sandbox[4] = "0";
        char scope_dir[MAX_PATH] = "";

        extract_token(line, "priority",    priority,     sizeof(priority));
        extract_token(line, "after",       after,        sizeof(after));
        extract_token(line, "tag",         tag,          sizeof(tag));
        extract_token(line, "verify",      verify,       sizeof(verify));
        extract_token(line, "max_beats",   max_beats,    sizeof(max_beats));
        extract_token(line, "max_tokens",  max_tokens_s, sizeof(max_tokens_s));
        extract_token(line, "max_sec",     max_sec,      sizeof(max_sec));
        extract_token(line, "max_cost",    max_cost,     sizeof(max_cost));
        extract_token(line, "lifecycle",   lifecycle,    sizeof(lifecycle));
        extract_token(line, "memory",      memory_mode,  sizeof(memory_mode));
        extract_token(line, "persona",     persona,      sizeof(persona));
        extract_token(line, "model",       model_hint,   sizeof(model_hint));
        extract_token(line, "target",      target,       sizeof(target));
        extract_token(line, "sandbox",     sandbox,      sizeof(sandbox));
        extract_token(line, "scope_dir",   scope_dir,    sizeof(scope_dir));

        if (!description[0]) { p = eol; continue; }

        /* Calculate due time if after= is set */
        char due[32] = "";
        if (after[0]) {
            time_t now = time(NULL);
            int val = atoi(after);
            char unit = after[strlen(after) - 1];
            int secs = 0;
            if (unit == 'm') secs = val * 60;
            else if (unit == 'h') secs = val * 3600;
            else if (unit == 'd') secs = val * 86400;
            else secs = val * 60;

            time_t due_time = now + secs;
            struct tm *tm = localtime(&due_time);
            strftime(due, sizeof(due), "%Y-%m-%dT%H:%M:%S", tm);
        }

        /* Escape description for JSON */
        char esc_desc[2048];
        size_t ei = 0;
        for (size_t i = 0; description[i] && ei < sizeof(esc_desc) - 2; i++) {
            if (description[i] == '"' || description[i] == '\\')
                esc_desc[ei++] = '\\';
            esc_desc[ei++] = description[i];
        }
        esc_desc[ei] = '\0';

        /* Escape verify path too */
        char esc_verify[MAX_PATH];
        size_t vi = 0;
        for (size_t i = 0; verify[i] && vi < sizeof(esc_verify) - 2; i++) {
            if (verify[i] == '"' || verify[i] == '\\')
                esc_verify[vi++] = '\\';
            esc_verify[vi++] = verify[i];
        }
        esc_verify[vi] = '\0';

        char ts[32];
        timestamp_now(ts, sizeof(ts));

        /* Build JSON entry with optional fulfillment + executor objects.
         * For backward compat, fulfillment{} only appears if verify or any
         * budget field is set. executor{} always appears with defaults. */
        int has_contract = verify[0] || max_beats[0] || max_tokens_s[0] ||
                           max_sec[0] || max_cost[0];

        char entry[8192];
        int elen = snprintf(entry, sizeof(entry),
            "{\"created\":\"%s\",\"priority\":\"%s\",\"due\":\"%s\","
            "\"tag\":\"%s\",\"description\":\"%s\",\"status\":\"pending\","
            "\"node_id\":\"%s\"",
            ts, priority, due, tag, esc_desc,
            getenv("NEIL_NODE_ID") ? getenv("NEIL_NODE_ID") : "unknown");

        /* Executor object (always present, defaults if unspecified) */
        elen += snprintf(entry + elen, sizeof(entry) - elen,
            ",\"executor\":{"
            "\"target\":\"%s\","
            "\"lifecycle\":\"%s\","
            "\"memory_mode\":\"%s\","
            "\"persona\":\"%s\","
            "\"model_hint\":\"%s\","
            "\"sandbox\":%s,"
            "\"scope_dir\":\"%s\""
            "}",
            target, lifecycle, memory_mode, persona, model_hint,
            (sandbox[0] == '1' || sandbox[0] == 't') ? "true" : "false",
            scope_dir);

        /* Fulfillment object only if contract specified */
        if (has_contract) {
            elen += snprintf(entry + elen, sizeof(entry) - elen,
                ",\"fulfillment\":{"
                "\"verify_cmd\":\"%s\","
                "\"verify_timeout_sec\":60,"
                "\"budget\":{"
                    "\"max_beats\":%s,"
                    "\"max_tokens\":%s,"
                    "\"max_wall_clock_sec\":%s,"
                    "\"max_cost_usd\":%s"
                "}"
                "}"
                ",\"fulfillment_state\":{"
                    "\"attempts\":0,"
                    "\"beats_consumed\":0,"
                    "\"tokens_consumed\":0,"
                    "\"wall_clock_start\":\"\","
                    "\"wall_clock_end\":\"\","
                    "\"cost_usd\":0.0,"
                    "\"last_verify\":\"pending\","
                    "\"last_verify_msg\":\"\","
                    "\"failed_reason\":\"\""
                "}",
                esc_verify,
                max_beats[0]    ? max_beats    : "0",
                max_tokens_s[0] ? max_tokens_s : "0",
                max_sec[0]      ? max_sec      : "0",
                max_cost[0]     ? max_cost     : "0");
        }

        elen += snprintf(entry + elen, sizeof(entry) - elen, "}\n");

        int fd = open(intentions_path, O_WRONLY | O_CREAT | O_APPEND, 0644);
        if (fd >= 0) {
            ssize_t w = write(fd, entry, (size_t)elen);
            (void)w;
            close(fd);
        }

        fprintf(stderr, "[autoprompt] INTEND: %s [%s]%s%s\n",
                esc_desc, priority,
                has_contract ? " [contracted]" : "",
                verify[0] ? " verify=set" : "");

        /* Stream for TUI */
        {
            char intend_detail[512];
            snprintf(intend_detail, sizeof(intend_detail),
                     "priority=%s%s | %s", priority,
                     has_contract ? " [contract]" : "",
                     esc_desc);
            stream_action("INTEND", intend_detail, NULL, NULL);
        }

        p = eol;
    }
}

/* Parse FAIL: lines and append to failures.json */
static void record_failures(const char *output) {
    if (!output) return;

    char fail_path[MAX_PATH];
    snprintf(fail_path, sizeof(fail_path), "%s/self/failures.json", g_neil_home);

    const char *p = output;
    while ((p = strstr(p, "FAIL:")) != NULL) {
        if (p != output && *(p - 1) != '\n') { p++; continue; }

        p += 5;
        while (*p == ' ') p++;

        const char *eol = strchr(p, '\n');
        if (!eol) eol = p + strlen(p);

        char line[4096];
        size_t ll = (size_t)(eol - p);
        if (ll >= sizeof(line)) ll = sizeof(line) - 1;
        memcpy(line, p, ll);
        line[ll] = '\0';

        /* Find | separator */
        char *pipe = strchr(line, '|');
        if (!pipe) { p = eol; continue; }

        char *error_desc = pipe + 1;
        while (*error_desc == ' ') error_desc++;
        *pipe = '\0';

        /* Parse params: source=, severity=, context= */
        char source[64] = "unknown", severity[32] = "medium", context[256] = "";

        char *tok = strtok(line, " ");
        while (tok) {
            if (strncmp(tok, "source=", 7) == 0)
                snprintf(source, sizeof(source), "%s", tok + 7);
            else if (strncmp(tok, "severity=", 9) == 0)
                snprintf(severity, sizeof(severity), "%s", tok + 9);
            else if (strncmp(tok, "context=", 8) == 0)
                snprintf(context, sizeof(context), "%s", tok + 8);
            tok = strtok(NULL, " ");
        }

        /* Escape for JSON */
        char esc_err[2048];
        size_t ei = 0;
        for (size_t i = 0; error_desc[i] && ei < sizeof(esc_err) - 2; i++) {
            if (error_desc[i] == '"' || error_desc[i] == '\\')
                esc_err[ei++] = '\\';
            esc_err[ei++] = error_desc[i];
        }
        esc_err[ei] = '\0';

        char ts[32];
        timestamp_now(ts, sizeof(ts));

        char entry[4096];
        int elen = snprintf(entry, sizeof(entry),
            "{\"timestamp\":\"%s\",\"source\":\"%s\",\"error\":\"%s\","
            "\"context\":\"%s\",\"severity\":\"%s\",\"resolution\":\"pending\",\"notes\":\"\"}\n",
            ts, source, esc_err, context, severity);

        int fd = open(fail_path, O_WRONLY | O_CREAT | O_APPEND, 0644);
        if (fd >= 0) {
            ssize_t w = write(fd, entry, (size_t)elen);
            (void)w;
            close(fd);
        }

        fprintf(stderr, "[autoprompt] FAIL: [%s] %s: %s\n", severity, source, esc_err);
        p = eol;
    }
}

/* Parse DONE: lines and mark first matching pending intention as completed.
 * Uses sed to do the replacement cleanly -- simpler than C string surgery. */
static void complete_intentions(const char *output) {
    if (!output) return;

    char intentions_path[MAX_PATH];
    snprintf(intentions_path, sizeof(intentions_path),
        "%s/intentions.json", g_neil_home);

    const char *p = output;
    while ((p = strstr(p, "DONE:")) != NULL) {
        if (p != output && *(p - 1) != '\n') { p++; continue; }

        p += 5;
        while (*p == ' ') p++;

        const char *eol = strchr(p, '\n');
        if (!eol) eol = p + strlen(p);

        char keyword[256];
        size_t kl = (size_t)(eol - p);
        if (kl >= sizeof(keyword)) kl = sizeof(keyword) - 1;
        memcpy(keyword, p, kl);
        keyword[kl] = '\0';
        while (kl > 0 && (keyword[kl-1] == ' ' || keyword[kl-1] == '\n'))
            keyword[--kl] = '\0';

        if (!keyword[0]) { p = eol; continue; }

        /* Escape keyword for sed */
        char esc_kw[512];
        size_t ei = 0;
        for (size_t i = 0; keyword[i] && ei < sizeof(esc_kw) - 3; i++) {
            if (keyword[i] == '/' || keyword[i] == '.' || keyword[i] == '[' ||
                keyword[i] == ']' || keyword[i] == '*' || keyword[i] == '+' ||
                keyword[i] == '?' || keyword[i] == '(' || keyword[i] == ')' ||
                keyword[i] == '{' || keyword[i] == '}' || keyword[i] == '|' ||
                keyword[i] == '^' || keyword[i] == '$' || keyword[i] == '\\') {
                esc_kw[ei++] = '\\';
            }
            esc_kw[ei++] = keyword[i];
        }
        esc_kw[ei] = '\0';

        /* Delegate to neil-complete-intent helper -- handles verify_cmd,
         * fulfillment_state tracking, and atomic rewrite of intentions.json. */
        char cmd[4096];
        snprintf(cmd, sizeof(cmd),
            "NEIL_HOME=%s %s/bin/neil-complete-intent '%s' 2>&1",
            g_neil_home, g_neil_home, esc_kw);

        char *result = run_command(cmd);
        if (result) {
            fprintf(stderr, "[autoprompt] DONE: %s -> %s", keyword, result);

            /* Stream result so TUI shows verify outcome */
            char detail[512];
            char first_line[256];
            size_t fi = 0;
            for (size_t i = 0; result[i] && result[i] != '\n' && fi < sizeof(first_line) - 1; i++)
                first_line[fi++] = result[i];
            first_line[fi] = '\0';
            snprintf(detail, sizeof(detail), "keyword=%s | %s", keyword, first_line);
            stream_action("DONE", detail, NULL, NULL);

            free(result);
        }

        p = eol;
    }
}

/* Parse first PROMPT: line from output and queue it as next prompt. */
static void queue_self_prompt(const char *output) {
    if (!output) return;

    const char *p = output;
    while ((p = strstr(p, "PROMPT:")) != NULL) {
        if (p != output && *(p - 1) != '\n') { p++; continue; }

        p += 7; /* skip "PROMPT:" */
        while (*p == ' ') p++;

        const char *eol = strchr(p, '\n');
        if (!eol) eol = p + strlen(p);

        size_t len = (size_t)(eol - p);
        if (len == 0) { p = eol; continue; }

        /* Write to queue/ as next prompt */
        char path[MAX_PATH];
        char ts[64];
        timestamp_now(ts, sizeof(ts));
        snprintf(path, sizeof(path), "%s/%s_self.md", g_queue_dir, ts);

        write_file_atomic(path, p, len);
        fprintf(stderr, "[autoprompt] self-prompt queued: %s\n", path);

        /* Only queue ONE prompt per cycle to prevent runaway */
        return;
    }
}

/* Log heartbeat status to ~/.neil/heartbeat_log.json */
static void extract_report_field(const char *output, const char *prefix,
                                 char *dst, size_t dstsz) {
    dst[0] = '\0';
    const char *p = strstr(output, prefix);
    if (!p) return;
    p += strlen(prefix);
    while (*p == ' ') p++;
    const char *eol = strchr(p, '\n');
    if (!eol) eol = p + strlen(p);
    size_t ll = (size_t)(eol - p);
    if (ll >= dstsz) ll = dstsz - 1;
    /* Copy, escaping quotes for JSON */
    size_t di = 0;
    for (size_t i = 0; i < ll && di < dstsz - 2; i++) {
        if (p[i] == '"' || p[i] == '\\') dst[di++] = '\\';
        dst[di++] = p[i];
    }
    dst[di] = '\0';
}


/*
 * Load required heartbeat report fields from config.toml [heartbeat.report] section.
 * Returns number of fields loaded. Each field stored as name[i] and desc[i].
 * Format in config.toml:
 *   [heartbeat.report]
 *   ACTION = "what you did this beat"
 *   QUESTION = "a genuine question you have"
 */
#define MAX_REPORT_FIELDS 16

static int load_report_fields(char names[][64], char descs[][256], int max) {
    char config_path[MAX_PATH];
    snprintf(config_path, sizeof(config_path), "%s/config.toml", g_neil_home);

    FILE *fp = fopen(config_path, "r");
    if (!fp) return 0;

    char line[512];
    int in_section = 0;
    int count = 0;

    while (fgets(line, sizeof(line), fp) && count < max) {
        /* Trim trailing newline */
        size_t len = strlen(line);
        while (len > 0 && (line[len-1] == '\n' || line[len-1] == '\r'))
            line[--len] = '\0';

        /* Skip comments and empty */
        char *trimmed = line;
        while (*trimmed == ' ' || *trimmed == '\t') trimmed++;
        if (*trimmed == '#' || *trimmed == '\0') continue;

        /* Check for section headers */
        if (*trimmed == '[') {
            in_section = (strstr(trimmed, "[heartbeat.report]") != NULL) ? 1 : 0;
            continue;
        }

        if (!in_section) continue;

        /* Parse: FIELDNAME = "description" */
        char *eq = strchr(trimmed, '=');
        if (!eq) continue;

        /* Extract field name */
        size_t nlen = (size_t)(eq - trimmed);
        while (nlen > 0 && (trimmed[nlen-1] == ' ' || trimmed[nlen-1] == '\t'))
            nlen--;
        if (nlen == 0 || nlen >= 64) continue;
        memcpy(names[count], trimmed, nlen);
        names[count][nlen] = '\0';

        /* Extract description (strip quotes) */
        char *val = eq + 1;
        while (*val == ' ' || *val == '\t') val++;
        if (*val == '"') val++;
        size_t vlen = strlen(val);
        if (vlen > 0 && val[vlen-1] == '"') vlen--;
        if (vlen >= 256) vlen = 255;
        memcpy(descs[count], val, vlen);
        descs[count][vlen] = '\0';

        count++;
    }
    fclose(fp);
    return count;
}

/*
 * Load the re-prompt template from essence/heartbeat_reprompt.md.
 * Returns malloc'd string or NULL. Caller must free.
 * Template uses {previous_output} and {missing_fields} placeholders.
 */
static char *load_reprompt_template(void) {
    char path[MAX_PATH];
    snprintf(path, sizeof(path), "%s/essence/heartbeat_reprompt.md", g_neil_home);
    size_t len;
    return read_file(path, &len);
}

/*
 * Build re-prompt string from template, substituting placeholders.
 * Returns malloc'd string. Caller must free.
 */
static char *build_reprompt(const char *tmpl, const char *prev_output, const char *missing) {
    /* Estimate size */
    size_t cap = strlen(tmpl) + strlen(prev_output) + strlen(missing) + 256;
    char *result = malloc(cap);
    if (!result) return NULL;

    size_t ri = 0;
    const char *p = tmpl;
    while (*p && ri < cap - 1) {
        if (strncmp(p, "{previous_output}", 17) == 0) {
            size_t plen = strlen(prev_output);
            if (ri + plen < cap) { memcpy(result + ri, prev_output, plen); ri += plen; }
            p += 17;
        } else if (strncmp(p, "{missing_fields}", 16) == 0) {
            size_t mlen = strlen(missing);
            if (ri + mlen < cap) { memcpy(result + ri, missing, mlen); ri += mlen; }
            p += 16;
        } else {
            result[ri++] = *p++;
        }
    }
    result[ri] = '\0';
    return result;
}

/*
 * Validate heartbeat report: check all required fields from config.toml are present.
 * If missing, load re-prompt template and do one more Claude turn.
 * Modifies all_output in place (realloc'd if needed).
 */
static void validate_heartbeat_report(const char *output,
                                       char **all_output_ptr, size_t *all_output_len_ptr,
                                       size_t *all_output_cap_ptr,
                                       const char *essence, int *turn_ptr) {
    char names[MAX_REPORT_FIELDS][64];
    char descs[MAX_REPORT_FIELDS][256];
    int nfields = load_report_fields(names, descs, MAX_REPORT_FIELDS);

    if (nfields == 0) return; /* no fields configured */

    /* Check which fields are present */
    char missing[2048];
    int mi = 0;
    int nmissing = 0;
    for (int i = 0; i < nfields; i++) {
        char needle[68];
        snprintf(needle, sizeof(needle), "%s:", names[i]);
        if (strstr(*all_output_ptr, needle) == NULL) {
            mi += snprintf(missing + mi, sizeof(missing) - mi,
                "- %s: (%s)\n", names[i], descs[i]);
            nmissing++;
        }
    }

    if (nmissing == 0) return; /* all fields present */

    fprintf(stderr, "[autoprompt] heartbeat report missing %d/%d fields, re-prompting\n",
            nmissing, nfields);
    set_seal_pose("normal", "open", "swim", "thought", "completing report...");

    /* Load template */
    char *tmpl = load_reprompt_template();
    char *followup = NULL;
    if (tmpl) {
        followup = build_reprompt(tmpl, *all_output_ptr, missing);
        free(tmpl);
    }
    if (!followup) {
        /* Fallback if template missing */
        size_t flen = strlen(*all_output_ptr) + sizeof(missing) + 512;
        followup = malloc(flen);
        if (!followup) return;
        snprintf(followup, flen,
            "[YOUR PREVIOUS OUTPUT]\n%s\n\n"
            "[INCOMPLETE REPORT]\nMissing fields:\n%s\n"
            "Output ONLY the missing fields now.",
            *all_output_ptr, missing);
    }

    /* Stream separator */
    {
        char sep[64];
        int sl = snprintf(sep, sizeof(sep), "\n--- completing report ---\n");
        stream_write(sep, (size_t)sl);
    }

    /* Run one more turn */
    char *extra_output = NULL;
    size_t extra_len = 0;
    int extra_exit = run_claude(followup, essence, &extra_output, &extra_len);

    if (extra_exit == 0 && extra_output && extra_len > 0) {
        /* Append to all_output */
        while (*all_output_len_ptr + extra_len + 64 > *all_output_cap_ptr) {
            *all_output_cap_ptr *= 2;
            char *tmp = realloc(*all_output_ptr, *all_output_cap_ptr);
            if (tmp) *all_output_ptr = tmp; else break;
        }
        *all_output_len_ptr += snprintf(*all_output_ptr + *all_output_len_ptr,
            *all_output_cap_ptr - *all_output_len_ptr, "\n--- report completion ---\n");
        memcpy(*all_output_ptr + *all_output_len_ptr, extra_output, extra_len);
        *all_output_len_ptr += extra_len;
        (*all_output_ptr)[*all_output_len_ptr] = '\0';

        extract_memories(extra_output);
    }
    free(extra_output);
    free(followup);
    (*turn_ptr)++;
}

static void log_heartbeat(const char *output, const char *filename) {
    if (!output) return;

    /* Look for HEARTBEAT: line */
    const char *hb = strstr(output, "HEARTBEAT:");
    char status[64] = "unknown";
    char summary[256] = "";

    if (hb) {
        const char *eol = strchr(hb, '\n');
        if (!eol) eol = hb + strlen(hb);

        char line[512];
        size_t ll = (size_t)(eol - hb);
        if (ll >= sizeof(line)) ll = sizeof(line) - 1;
        memcpy(line, hb, ll);
        line[ll] = '\0';

        /* Parse status= and summary= */
        char *sp = strstr(line, "status=");
        if (sp) {
            sp += 7;
            size_t i = 0;
            while (sp[i] && sp[i] != ' ' && sp[i] != '"' && i < sizeof(status) - 1)
                status[i] = sp[i], i++;
            status[i] = '\0';
        }
        char *sm = strstr(line, "summary=\"");
        if (sm) {
            sm += 9;
            size_t i = 0;
            while (sm[i] && sm[i] != '"' && i < sizeof(summary) - 1)
                summary[i] = sm[i], i++;
            summary[i] = '\0';
        }
    }

    /* Parse structured report fields */
    char action[512] = "";
    char question[512] = "";
    char improvement[512] = "";
    char contribution[1024] = "";
    extract_report_field(output, "ACTION:", action, sizeof(action));
    extract_report_field(output, "QUESTION:", question, sizeof(question));
    extract_report_field(output, "IMPROVEMENT:", improvement, sizeof(improvement));
    extract_report_field(output, "CONTRIBUTION:", contribution, sizeof(contribution));

    /* Build summary from action if empty (backward compat) */
    if (!summary[0] && action[0]) {
        snprintf(summary, sizeof(summary), "%s", action);
    }

    char ts[64];
    timestamp_now(ts, sizeof(ts));

    char log_entry[4096];
    int log_len = snprintf(log_entry, sizeof(log_entry),
        "{\"timestamp\":\"%s\",\"prompt\":\"%s\",\"status\":\"%s\","
        "\"summary\":\"%s\",\"action\":\"%s\",\"question\":\"%s\","
        "\"improvement\":\"%s\",\"contribution\":\"%s\"}\n",
        ts, filename, status, summary, action, question, improvement, contribution);

    /* Append to log file and trim to last 10 entries */
    const char *log_path = g_heartbeat_log;
    int fd = open(log_path, O_WRONLY | O_CREAT | O_APPEND, 0644);
    if (fd >= 0) {
        ssize_t w = write(fd, log_entry, (size_t)log_len);
        (void)w;
        close(fd);
    }

    /* Trim: keep only last 10 lines */
    char trim_cmd[MAX_PATH];
    snprintf(trim_cmd, sizeof(trim_cmd), "tail -10 %s", g_heartbeat_log);
    char *trimmed = run_command(trim_cmd);
    if (trimmed) {
        write_file_atomic(log_path, trimmed, strlen(trimmed));
        free(trimmed);
    }
}

/*
 * Parse READ:, WRITE:, BASH: lines from Claude output and execute them.
 * Returns accumulated results (malloc'd) or NULL if no tool actions found.
 *
 * READ: /path/to/file
 *   -> reads file, returns content
 *
 * BASH: command to run
 *   -> executes via popen, returns stdout+stderr
 *
 * WRITE: path=/path/to/file
 * ```
 * file content here
 * ```
 *   -> writes content to file
 */
static char *execute_tool_actions(const char *output) {
    if (!output) return NULL;

    size_t results_cap = 8192, results_len = 0;
    char *results = malloc(results_cap);
    if (!results) return NULL;
    results[0] = '\0';

    const char *p = output;

    while (*p) {
        /* Find next action line */
        const char *line_start = p;

        /* Skip to a line that starts with READ:, WRITE:, or BASH: */
        int found = 0;
        while (*p) {
            if (p == output || *(p-1) == '\n') {
                if (strncmp(p, "READ:", 5) == 0 || strncmp(p, "BASH:", 5) == 0 ||
                    strncmp(p, "WRITE:", 6) == 0) {
                    found = 1;
                    break;
                }
            }
            p++;
        }
        if (!found) break;

        /* Ensure enough space in results */
        if (results_len + 4096 > results_cap) {
            results_cap *= 2;
            char *tmp = realloc(results, results_cap);
            if (tmp) results = tmp; else break;
        }

        if (strncmp(p, "READ:", 5) == 0) {
            p += 5;
            while (*p == ' ') p++;
            const char *eol = strchr(p, '\n');
            if (!eol) eol = p + strlen(p);

            char path[MAX_PATH];
            size_t pl = (size_t)(eol - p);
            if (pl >= sizeof(path)) pl = sizeof(path) - 1;
            memcpy(path, p, pl);
            path[pl] = '\0';
            /* Trim trailing whitespace */
            while (pl > 0 && (path[pl-1] == ' ' || path[pl-1] == '\r'))
                path[--pl] = '\0';

            /* Expand ~ to HOME */
            char expanded[MAX_PATH];
            expand_tilde(expanded, sizeof(expanded), path);

            fprintf(stderr, "[autoprompt] READ: %s\n", expanded);
            stream_action("READ", expanded, NULL, NULL);

            size_t flen;
            char *content = read_file(expanded, &flen);
            if (content) {
                /* Cap at 50KB to avoid blowing up context */
                if (flen > 50000) {
                    content[50000] = '\0';
                    flen = 50000;
                }
                results_len += snprintf(results + results_len, results_cap - results_len,
                    "[READ %s]\n%s\n[/READ]\n\n", expanded, content);
                stream_action(NULL, NULL, expanded, content);
                free(content);
            } else {
                results_len += snprintf(results + results_len, results_cap - results_len,
                    "[READ ERROR] %s: file not found or unreadable\n\n", expanded);
                stream_action("READ", expanded, NULL, "ERROR: file not found");
            }
            p = eol;

        } else if (strncmp(p, "BASH:", 5) == 0) {
            p += 5;
            while (*p == ' ') p++;
            const char *eol = strchr(p, '\n');
            if (!eol) eol = p + strlen(p);

            char cmd[4096];
            size_t cl = (size_t)(eol - p);
            if (cl >= sizeof(cmd)) cl = sizeof(cmd) - 1;
            memcpy(cmd, p, cl);
            cmd[cl] = '\0';

            fprintf(stderr, "[autoprompt] BASH: %s\n", cmd);

            /* Execute with timeout (60s) and capture output */
            char wrapped[4200];
            snprintf(wrapped, sizeof(wrapped), "timeout 60 sh -c '%s' 2>&1", cmd);
            char *bash_result = run_command(wrapped);

            if (bash_result) {
                /* Cap output at 20KB */
                size_t blen = strlen(bash_result);
                if (blen > 20000) {
                    bash_result[20000] = '\0';
                    blen = 20000;
                }
                while (results_len + blen + 256 > results_cap) {
                    results_cap *= 2;
                    char *tmp = realloc(results, results_cap);
                    if (tmp) results = tmp; else break;
                }
                results_len += snprintf(results + results_len, results_cap - results_len,
                    "[BASH %s]\n%s\n[/BASH]\n\n", cmd, bash_result);
                stream_action("BASH", cmd, cmd, bash_result);
                free(bash_result);
            } else {
                results_len += snprintf(results + results_len, results_cap - results_len,
                    "[BASH ERROR] %s: execution failed\n\n", cmd);
                stream_action("BASH", cmd, cmd, "ERROR: execution failed");
            }
            p = eol;

        } else if (strncmp(p, "WRITE:", 6) == 0) {
            p += 6;
            while (*p == ' ') p++;

            /* Parse path= parameter */
            char path[MAX_PATH] = "";
            if (strncmp(p, "path=", 5) == 0) {
                p += 5;
                const char *eol = strchr(p, '\n');
                if (!eol) eol = p + strlen(p);
                size_t pl = (size_t)(eol - p);
                if (pl >= sizeof(path)) pl = sizeof(path) - 1;
                memcpy(path, p, pl);
                path[pl] = '\0';
                while (pl > 0 && (path[pl-1] == ' ' || path[pl-1] == '\r'))
                    path[--pl] = '\0';
                p = (*eol) ? eol + 1 : eol;
            }

            if (!path[0]) { continue; }

            /* Expand ~ to HOME */
            char wpath[MAX_PATH];
            expand_tilde(wpath, sizeof(wpath), path);

            /* Find code block content */
            /* Skip to opening ``` */
            while (*p && strncmp(p, "```", 3) != 0) p++;
            if (*p) {
                p += 3;
                /* Skip optional language tag */
                while (*p && *p != '\n') p++;
                if (*p == '\n') p++;
            }

            /* Collect content until closing ``` */
            const char *content_start = p;
            while (*p && strncmp(p, "\n```", 4) != 0 && strncmp(p, "```", 3) != 0) p++;
            size_t content_len = (size_t)(p - content_start);

            /* Skip closing ``` */
            if (*p == '\n') p++;
            if (strncmp(p, "```", 3) == 0) p += 3;

            fprintf(stderr, "[autoprompt] WRITE: %s (%zu bytes)\n", wpath, content_len);

            /* Write the file atomically (handles partial writes, fsync, rename) */
            if (write_file_atomic(wpath, content_start, content_len) == 0) {
                results_len += snprintf(results + results_len, results_cap - results_len,
                    "[WRITE %s] %zu bytes written\n\n", wpath, content_len);

                char detail[512];
                snprintf(detail, sizeof(detail), "%s (%zu bytes)", wpath, content_len);
                stream_action("WRITE", detail, NULL, NULL);
            } else {
                results_len += snprintf(results + results_len, results_cap - results_len,
                    "[WRITE ERROR] %s: %s\n\n", wpath, strerror(errno));
                stream_action("WRITE", wpath, NULL, "ERROR: write failed");
            }
        } else {
            p++;
        }
    }

    if (results_len == 0) { free(results); return NULL; }
    return results;
}

/*
 * Parse CALL: lines from Claude output and execute API calls via vault.
 * Format: CALL: service=<name> action=<action> [param=value ...]
 * Returns a malloc'd string of all call results concatenated, or NULL.
 */
static char *execute_service_calls(const char *output) {
    if (!output) return NULL;

    size_t results_cap = 4096, results_len = 0;
    char *results = malloc(results_cap);
    if (!results) return NULL;
    results[0] = '\0';

    const char *p = output;
    while ((p = strstr(p, "CALL:")) != NULL) {
        if (p != output && *(p - 1) != '\n') { p++; continue; }

        p += 5; /* skip "CALL:" */
        while (*p == ' ') p++;

        const char *eol = strchr(p, '\n');
        if (!eol) eol = p + strlen(p);

        char line[4096];
        size_t ll = (size_t)(eol - p);
        if (ll >= sizeof(line)) ll = sizeof(line) - 1;
        memcpy(line, p, ll);
        line[ll] = '\0';

        /* Parse service= and action= */
        char service[64] = "", action[64] = "";
        char params[16384] = "";   /* was 2048; bumped for long initial_intention values */
        size_t pi = 0;

        /* Tokenize, respecting quoted values */
        char *s = line;
        while (*s) {
            while (*s == ' ') s++;
            if (!*s) break;

            char key[64] = "", val[8192] = "";   /* was 512; prevents value-truncation re-parse */
            /* parse key= */
            size_t ki = 0;
            while (*s && *s != '=' && *s != ' ' && ki < sizeof(key) - 1)
                key[ki++] = *s++;
            key[ki] = '\0';

            if (*s == '=') {
                s++;
                size_t vi = 0;
                if (*s == '"') {
                    s++;
                    while (*s && *s != '"' && vi < sizeof(val) - 1)
                        val[vi++] = *s++;
                    /* GUARD: if val buffer filled before close quote,
                     * skip past the remainder of the quoted content so
                     * we do not re-tokenize value-interior as new key=val
                     * pairs (e.g. embedded "service=peer_send" in a long
                     * initial_intention would overwrite the outer service). */
                    while (*s && *s != '"') s++;
                    if (*s == '"') s++;
                } else {
                    while (*s && *s != ' ' && vi < sizeof(val) - 1)
                        val[vi++] = *s++;
                }
                val[vi] = '\0';
            }

            if (strcmp(key, "service") == 0)
                snprintf(service, sizeof(service), "%s", val);
            else if (strcmp(key, "action") == 0)
                snprintf(action, sizeof(action), "%s", val);
            else if (key[0] && val[0]) {
                /* Collect remaining params for the handler.
                 * Always quote values (handler.sh's eval_params handles quotes)
                 * to preserve spaces and special chars within values. */
                pi += snprintf(params + pi, sizeof(params) - pi,
                    "%s%s=\"%s\"", pi > 0 ? " " : "", key, val);
            }
        }

        if (!service[0] || !action[0]) { p = eol; continue; }

        /* Validate service exists in registry */
        char reg_path[MAX_PATH];
        snprintf(reg_path, sizeof(reg_path), "%s/%s.md", g_services_registry, service);
        if (access(reg_path, F_OK) != 0) {
            size_t needed = results_len + 256;
            if (needed > results_cap) {
                results_cap = needed * 2;
                char *tmp = realloc(results, results_cap);
                if (tmp) results = tmp;
            }
            results_len += snprintf(results + results_len, results_cap - results_len,
                "[CALL ERROR] service=%s: not registered\n", service);
            p = eol; continue;
        }

        /* Check vault credential exists */
        char vault_path[MAX_PATH];
        snprintf(vault_path, sizeof(vault_path), "%s/%s.key", g_services_vault, service);
        if (access(vault_path, F_OK) != 0) {
            size_t needed = results_len + 256;
            if (needed > results_cap) {
                results_cap = needed * 2;
                char *tmp = realloc(results, results_cap);
                if (tmp) results = tmp;
            }
            results_len += snprintf(results + results_len, results_cap - results_len,
                "[CALL ERROR] service=%s: no credential in vault\n", service);
            p = eol; continue;
        }

        /* Read credential from vault */
        size_t cred_len;
        char *cred = read_file(vault_path, &cred_len);
        if (!cred) { p = eol; continue; }
        /* trim trailing newline */
        while (cred_len > 0 && (cred[cred_len-1] == '\n' || cred[cred_len-1] == '\r'))
            cred[--cred_len] = '\0';

        /* Dispatch to service handler via shell script.
         * Escape single quotes in every interpolated value so apostrophes
         * in agent-emitted CALL messages don't close the single-quoted
         * bash literals and expose parens/text to shell parsing.  */
        char *_esc_svc  = sh_escape_sq(service);
        char *_esc_act  = sh_escape_sq(action);
        char *_esc_cred = sh_escape_sq(cred);
        char *_esc_pars = sh_escape_sq(params);
        char handler_cmd[32768];   /* was 8192; room for bumped params */
        snprintf(handler_cmd, sizeof(handler_cmd),
            "NEIL_SERVICE='%s' NEIL_ACTION='%s' NEIL_CRED='%s' NEIL_PARAMS='%s' "
            "%s/services/handler.sh 2>&1",
            _esc_svc ? _esc_svc : service,
            _esc_act ? _esc_act : action,
            _esc_cred ? _esc_cred : cred,
            _esc_pars ? _esc_pars : params,
            g_neil_home);
        free(_esc_svc); free(_esc_act); free(_esc_cred); free(_esc_pars);

        fprintf(stderr, "[autoprompt] CALL: service=%s action=%s\n", service, action);
        char *call_result = run_command(handler_cmd);

        /* Stream the service call so TUI shows it */
        {
            char call_detail[256];
            snprintf(call_detail, sizeof(call_detail), "service=%s action=%s %s",
                     service, action, params);
            stream_action("CALL", call_detail, handler_cmd, call_result);
        }

        /* Clear credential from memory */
        memset(cred, 0, cred_len);
        free(cred);

        if (call_result) {
            size_t cr_len = strlen(call_result);
            size_t needed = results_len + cr_len + 128;
            if (needed > results_cap) {
                results_cap = needed * 2;
                char *tmp = realloc(results, results_cap);
                if (tmp) results = tmp;
            }
            results_len += snprintf(results + results_len, results_cap - results_len,
                "[CALL RESULT] service=%s action=%s\n%s\n",
                service, action, call_result);
            free(call_result);
        }

        p = eol;
    }

    if (results_len == 0) { free(results); return NULL; }
    return results;
}

/* Generate a timestamp string: YYYY-MM-DDTHH-MM-SS */
static void timestamp_now(char *buf, size_t cap) {
    time_t t = time(NULL);
    struct tm *tm = localtime(&t);
    strftime(buf, cap, "%Y-%m-%dT%H-%M-%S", tm);
}

/* Set seal pose for blueprint TUI */
static void set_seal_pose(const char *eyes, const char *mouth,
                          const char *body, const char *indicator,
                          const char *label) {
    char path[MAX_PATH];
    snprintf(path, sizeof(path), "%s/.seal_pose.json", g_neil_home);
    char json[512];
    int len = snprintf(json, sizeof(json),
        "{\"eyes\":\"%s\",\"mouth\":\"%s\",\"body\":\"%s\","
        "\"indicator\":\"%s\",\"label\":\"%s\"}",
        eyes, mouth, body, indicator, label);
    int fd = open(path, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (fd >= 0) {
        ssize_t w = write(fd, json, (size_t)len);
        (void)w;
        close(fd);
    }
}

/* Stream file path for live output */
static int g_stream_fd = -1;

/* Global pointer to all_output buffer so stream_action can append to it.
 * Set by process_prompt before calling action handlers, cleared after. */
static char **g_all_output_ptr = NULL;
static size_t *g_all_output_len_ptr = NULL;
static size_t *g_all_output_cap_ptr = NULL;

static void stream_open(const char *prompt_name) {
    if (g_agent_manages_stream) return;  /* external agent owns the stream */
    char path[MAX_PATH];
    snprintf(path, sizeof(path), "%s/.neil_stream", g_neil_home);
    g_stream_fd = open(path, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (g_stream_fd >= 0) {
        dprintf(g_stream_fd, "{\"status\":\"running\",\"prompt\":\"%s\"}\n", prompt_name);
    }
}

static void stream_write(const char *data, size_t len) {
    if (g_agent_manages_stream) return;  /* external agent owns the stream */
    if (g_stream_fd >= 0) {
        ssize_t w = write(g_stream_fd, data, len);
        (void)w;
    }
}

static void stream_close(int exit_code) {
    if (g_agent_manages_stream) return;  /* external agent owns the stream */
    if (g_stream_fd >= 0) {
        dprintf(g_stream_fd, "\n{\"status\":\"done\",\"exit_code\":%d}\n", exit_code);
        close(g_stream_fd);
        g_stream_fd = -1;
    }
}

/* Write a structured action block to the stream so the TUI can render it.
 * Emits a bash code fence for commands, or an action prefix line. */
static void stream_action(const char *prefix, const char *detail,
                           const char *cmd, const char *output) {
    if (g_stream_fd < 0) return;
    char buf[8192];
    int n = 0;
    if (cmd && cmd[0]) {
        /* Command block: renders as Command in TUI */
        n = snprintf(buf, sizeof(buf), "\n```bash\n$ %s\n", cmd);
        if (output && output[0]) {
            /* Truncate output to first 20 lines */
            const char *p = output;
            int lines = 0;
            while (*p && lines < 20) {
                const char *nl = strchr(p, '\n');
                if (!nl) nl = p + strlen(p);
                size_t ll = (size_t)(nl - p);
                if (n + (int)ll + 2 < (int)sizeof(buf)) {
                    memcpy(buf + n, p, ll);
                    n += (int)ll;
                    buf[n++] = '\n';
                }
                p = (*nl) ? nl + 1 : nl;
                lines++;
            }
            if (*p) {
                n += snprintf(buf + n, sizeof(buf) - n, "... (truncated)\n");
            }
        }
        n += snprintf(buf + n, sizeof(buf) - n, "```\n");
    }
    /* Always write the action prefix line if given */
    if (prefix && prefix[0]) {
        n += snprintf(buf + n, sizeof(buf) - n, "%s: %s\n",
                      prefix, detail ? detail : "");
    }
    if (n > 0) {
        stream_write(buf, (size_t)n);

        /* Also append to all_output so result files capture this */
        if (g_all_output_ptr && *g_all_output_ptr && g_all_output_len_ptr && g_all_output_cap_ptr) {
            while (*g_all_output_len_ptr + (size_t)n + 1 > *g_all_output_cap_ptr) {
                *g_all_output_cap_ptr *= 2;
                char *tmp = realloc(*g_all_output_ptr, *g_all_output_cap_ptr);
                if (tmp) *g_all_output_ptr = tmp; else break;
            }
            memcpy(*g_all_output_ptr + *g_all_output_len_ptr, buf, (size_t)n);
            *g_all_output_len_ptr += (size_t)n;
            (*g_all_output_ptr)[*g_all_output_len_ptr] = '\0';
        }
    }
}

/* Execute AI command with prompt and optional system prompt.
 * Streams output to ~/.neil/.neil_stream in real-time.
 * Respects g_claude_timeout (seconds). Kills child on timeout. */
static int run_claude(const char *prompt, const char *system_prompt,
                      char **out, size_t *out_len) {
    int pipefd[2];
    if (pipe(pipefd) < 0) return -1;

    pid_t pid = fork();
    if (pid < 0) {
        close(pipefd[0]); close(pipefd[1]);
        return -1;
    }

    if (pid == 0) {
        /* child -- isolate into own session so parent's SIGTERM doesn't propagate */
        setsid();
        close(pipefd[0]);
        dup2(pipefd[1], STDOUT_FILENO);
        dup2(pipefd[1], STDERR_FILENO);
        close(pipefd[1]);

        /* Ensure local bin dirs are in PATH */
        const char *oldpath = getenv("PATH");
        char newpath[MAX_PATH];
        snprintf(newpath, sizeof(newpath), "%s/.neil/bin:%s/.local/bin:%s",
                 getenv("HOME") ? getenv("HOME") : "/tmp",
                 getenv("HOME") ? getenv("HOME") : "/tmp",
                 oldpath ? oldpath : "/usr/bin");
        setenv("PATH", newpath, 1);
        if (g_current_prompt_name[0]) setenv("NEIL_PROMPT_NAME", g_current_prompt_name, 1);
        setenv("NEIL_HOME", g_neil_home, 1);
        {
            char hn[256];
            if (gethostname(hn, sizeof(hn)) == 0) setenv("NEIL_NODE_ID", hn, 0);
        }

        /* Build argument array from config */
        const char *argv[32];
        int argc = 0;
        argv[argc++] = g_ai_command;

        /* Parse g_ai_args into individual arguments */
        static char args_buf[MAX_PATH];
        snprintf(args_buf, sizeof(args_buf), "%s", g_ai_args);
        char *tok = strtok(args_buf, " ");
        while (tok && argc < 24) {
            argv[argc++] = tok;
            tok = strtok(NULL, " ");
        }

        /* Add prompt */
        argv[argc++] = g_ai_prompt_flag;
        argv[argc++] = prompt;

        /* Add system prompt if provided */
        if (system_prompt && system_prompt[0] && g_ai_system_flag[0]) {
            argv[argc++] = g_ai_system_flag;
            argv[argc++] = system_prompt;
        }

        argv[argc] = NULL;

        execvp(g_ai_command, (char *const *)argv);
        dprintf(STDERR_FILENO, "exec failed: %s: %s\n", g_ai_command, strerror(errno));
        _exit(127);
    }

    /* parent */
    close(pipefd[1]);

    size_t cap = READ_BUF, len = 0;
    char *buf = malloc(cap);
    if (!buf) { close(pipefd[0]); kill(pid, SIGKILL); waitpid(pid, NULL, 0); return -1; }

    time_t deadline = 0;
    if (g_claude_timeout > 0)
        deadline = time(NULL) + g_claude_timeout;

    /* Use select() with timeout to read output without blocking forever */
    int timed_out = 0;
    for (;;) {
        fd_set rfds;
        FD_ZERO(&rfds);
        FD_SET(pipefd[0], &rfds);

        struct timeval tv;
        if (deadline > 0) {
            time_t remaining = deadline - time(NULL);
            if (remaining <= 0) {
                timed_out = 1;
                break;
            }
            tv.tv_sec = remaining;
            tv.tv_usec = 0;
        } else {
            /* No timeout -- 60s select chunks (still interruptible) */
            tv.tv_sec = 60;
            tv.tv_usec = 0;
        }

        int ret = select(pipefd[0] + 1, &rfds, NULL, NULL, &tv);
        if (ret < 0) {
            if (errno == EINTR) {
                if (!g_running) break;  /* SIGTERM received, exit gracefully */
                continue;
            }
            break;
        }
        if (ret == 0) {
            if (!g_running) break;  /* SIGTERM received, exit gracefully */
            /* select timed out */
            if (deadline > 0 && time(NULL) >= deadline) {
                timed_out = 1;
                break;
            }
            continue;  /* no-timeout mode: just loop */
        }

        ssize_t n = read(pipefd[0], buf + len, cap - len - 1);
        if (n <= 0) break;  /* EOF or error */

        /* Stream to file in real-time */
        stream_write(buf + len, (size_t)n);

        len += (size_t)n;
        if (len >= cap - 1) {
            cap *= 2;
            char *tmp = realloc(buf, cap);
            if (!tmp) break;  /* keep what we have */
            buf = tmp;
        }
    }
    close(pipefd[0]);
    buf[len] = '\0';

    if (timed_out) {
        fprintf(stderr, "[autoprompt] TIMEOUT: claude exceeded %ds limit, killing pid %d\n",
                g_claude_timeout, (int)pid);
        kill(pid, SIGTERM);
        /* Give it 5 seconds to clean up, then force kill */
        int grace = 5;
        while (grace-- > 0) {
            int wr = waitpid(pid, NULL, WNOHANG);
            if (wr > 0) break;
            sleep(1);
        }
        kill(pid, SIGKILL);
        waitpid(pid, NULL, 0);

        /* Append timeout marker to output so it's visible */
        const char *marker = "\n[TIMEOUT: Claude execution exceeded time limit]\n";
        size_t mlen = strlen(marker);
        if (len + mlen + 1 < cap) {
            memcpy(buf + len, marker, mlen + 1);
            len += mlen;
        }

        *out = buf;
        if (out_len) *out_len = len;
        return 124;  /* same as timeout(1) exit code */
    }

    int status;
    waitpid(pid, &status, 0);

    *out = buf;
    if (out_len) *out_len = len;

    return WIFEXITED(status) ? WEXITSTATUS(status) : -1;
}

/* Process a single prompt file. */
static void process_prompt(const char *filename) {
    char src[MAX_PATH], dst[MAX_PATH];
    char ts[64];

    timestamp_now(ts, sizeof(ts));

    /* 1. Move queue/ -> active/ */
    snprintf(src, sizeof(src), "%s/%s", g_queue_dir, filename);
    snprintf(dst, sizeof(dst), "%s/%s", g_active_dir, filename);

    if (rename(src, dst) < 0) {
        fprintf(stderr, "[autoprompt] move to active failed: %s: %s\n",
                filename, strerror(errno));
        return;
    }

    printf("[autoprompt] [%s] executing: %s\n", ts, filename);
    snprintf(g_current_prompt_name, sizeof(g_current_prompt_name), "%s", filename);
    g_processing = 1;
    set_seal_pose("focused", "neutral", "swim", "thought", "thinking...");

    /* Tick in_progress contracted intentions: +1 beat, set start time */
    {
        char tick_cmd[MAX_PATH];
        snprintf(tick_cmd, sizeof(tick_cmd), "%s/bin/neil-beat-tick 2>/dev/null", g_neil_home);
        char *tick_out = run_command(tick_cmd);
        free(tick_out);
    }

    /* 2. Read prompt content */
    size_t prompt_len;
    char *prompt = read_file(dst, &prompt_len);
    if (!prompt) {
        fprintf(stderr, "[autoprompt] failed to read: %s\n", filename);
        rename(dst, src);
        return;
    }

    /* Guard: reject prompts larger than MAX_PROMPT (1MB) */
    if (prompt_len > MAX_PROMPT) {
        fprintf(stderr, "[autoprompt] prompt too large (%zu bytes, max %d): %s\n",
                prompt_len, MAX_PROMPT, filename);
        free(prompt);
        /* Move to history as failed */
        char hist[MAX_PATH];
        snprintf(hist, sizeof(hist), "%s/%s_%s", g_history_dir, ts, filename);
        rename(dst, hist);
        return;
    }

    /* 2b. Content-hash dedup: skip if identical prompt processed recently */
    unsigned long prompt_hash = djb2_hash(prompt, prompt_len);
    if (dedup_check(prompt_hash)) {
        printf("[autoprompt] dedup: skipping duplicate prompt %s (hash %lu)\n",
               filename, prompt_hash);
        free(prompt);
        /* Move to history as skipped */
        char hist[MAX_PATH];
        snprintf(hist, sizeof(hist), "%s/%s_%s", g_history_dir, ts, filename);
        rename(dst, hist);
        /* Write a minimal result noting the skip */
        char skip_result[MAX_PATH];
        snprintf(skip_result, sizeof(skip_result), "%s.result.md", hist);
        FILE *sf = fopen(skip_result, "w");
        if (sf) {
            fprintf(sf, "[dedup] Skipped: identical content processed within %d seconds.\n",
                    DEDUP_WINDOW_SEC);
            fclose(sf);
        }
        g_processing = 0;
        set_seal_pose("neutral", "neutral", "idle", "none", "");
        return;
    }
    dedup_record(prompt_hash, filename);

    /* 3. Open stream for live output */
    stream_open(filename);

    /* 4. Build augmented prompt with context + memories */
    char *essence = NULL;
    char *aug_prompt = build_augmented_prompt(prompt, &essence, dst);

    /*
     * ReAct loop: reason -> act -> observe -> repeat
     * Max 3 iterations to prevent runaway.
     */
    /* max turns from config.toml, default 3 */

    /* Accumulate all outputs and call results across turns */
    size_t all_output_cap = 8192, all_output_len = 0;
    char *all_output = malloc(all_output_cap);
    if (all_output) all_output[0] = '\0';

    size_t all_calls_cap = 4096, all_calls_len = 0;
    char *all_calls = malloc(all_calls_cap);
    if (all_calls) all_calls[0] = '\0';

    char *current_prompt = aug_prompt;
    int exit_code = 0;
    int turn;

    for (turn = 0; turn < g_max_react_turns; turn++) {
        /* Execute claude (with retry on empty-output timeout) */
        char *output = NULL;
        size_t output_len = 0;
        exit_code = run_claude(current_prompt, essence, &output, &output_len);

        /* Retry once if we got a timeout (124) with no meaningful output.
         * This handles transient API failures where Claude never responds. */
        if (exit_code == 124 && (!output || output_len == 0 ||
            (output_len < 80 && strstr(output, "[TIMEOUT:")))) {
            fprintf(stderr, "[autoprompt] empty timeout on turn %d, retrying once...\n",
                    turn + 1);
            free(output);
            output = NULL;
            output_len = 0;
            set_seal_pose("wide", "open", "swim", "thought", "retrying...");
            sleep(5);  /* brief pause before retry */
            exit_code = run_claude(current_prompt, essence, &output, &output_len);
        }

        if (exit_code != 0 || !output) {
            free(output);
            break;
        }

        /* Accumulate output */
        if (all_output) {
            while (all_output_len + output_len + 64 > all_output_cap) {
                all_output_cap *= 2;
                char *tmp = realloc(all_output, all_output_cap);
                if (tmp) all_output = tmp; else break;
            }
            if (turn > 0) {
                all_output_len += snprintf(all_output + all_output_len,
                    all_output_cap - all_output_len, "\n--- turn %d ---\n", turn + 1);
            }
            memcpy(all_output + all_output_len, output, output_len);
            all_output_len += output_len;
            all_output[all_output_len] = '\0';
        }

        /* Wire up global all_output pointer for stream_action */
        g_all_output_ptr = &all_output;
        g_all_output_len_ptr = &all_output_len;
        g_all_output_cap_ptr = &all_output_cap;

        /* Extract and store MEMORY: lines */
        extract_memories(output);

        /* Execute CALL: lines and tool actions (READ/WRITE/BASH) */
        char *call_results = execute_service_calls(output);
        char *tool_results = execute_tool_actions(output);

        /* Merge results */
        if (tool_results && call_results) {
            size_t cl = strlen(call_results);
            size_t tl = strlen(tool_results);
            char *merged = malloc(cl + tl + 4);
            if (merged) {
                memcpy(merged, call_results, cl);
                memcpy(merged + cl, tool_results, tl);
                merged[cl + tl] = '\0';
                free(call_results);
                free(tool_results);
                call_results = merged;
                tool_results = NULL;
            }
        } else if (tool_results && !call_results) {
            call_results = tool_results;
            tool_results = NULL;
        }

        if (call_results) {
            /* Accumulate call results */
            size_t cr_len = strlen(call_results);
            if (all_calls) {
                while (all_calls_len + cr_len + 2 > all_calls_cap) {
                    all_calls_cap *= 2;
                    char *tmp = realloc(all_calls, all_calls_cap);
                    if (tmp) all_calls = tmp; else break;
                }
                memcpy(all_calls + all_calls_len, call_results, cr_len);
                all_calls_len += cr_len;
                all_calls[all_calls_len] = '\0';
            }

            fprintf(stderr, "[autoprompt] ReAct turn %d: CALL results received, re-invoking\n",
                    turn + 1);
            set_seal_pose("wide", "open", "swim", "bubbles", "calling API...");

            /* Stream turn separator */
            {
                char sep[64];
                int sl = snprintf(sep, sizeof(sep), "\n--- turn %d ---\n", turn + 2);
                stream_write(sep, (size_t)sl);
            }

            /* Build follow-up prompt with call results */
            size_t followup_cap = output_len + cr_len + 2048;
            char *followup = malloc(followup_cap);
            if (followup) {
                snprintf(followup, followup_cap,
                    "[PREVIOUS RESPONSE]\n%s\n\n"
                    "[ACTION RESULTS]\n%s\n\n"
                    "[INSTRUCTION]\n"
                    "Your action lines above were executed. The results are shown.\n"
                    "IMPORTANT: You can ONLY affect the system through action lines.\n"
                    "- To read a file: READ: /path\n"
                    "- To write a file: WRITE: path=/path followed by a code block\n"
                    "- To run a command: BASH: command\n"
                    "- To store knowledge: MEMORY: wing=x room=y | text\n"
                    "Describing work in prose does NOT execute it. If you say\n"
                    "\"I edited the file\" without a WRITE: line, nothing changed.\n"
                    "If you need to do more work, output action lines now.\n"
                    "When all work is done, end with HEARTBEAT: (if heartbeat) or just your summary.",
                    output, call_results);

                /* Free old prompt if it's not the original */
                if (current_prompt != aug_prompt) free(current_prompt);
                current_prompt = followup;
            }

            free(call_results);
            free(output);
            continue; /* next turn */
        }

        /* No CALL: lines -- check if heartbeat needs report completion */
        int is_heartbeat = (strstr(filename, "heartbeat") != NULL
                         || strstr(filename, "wakeup") != NULL);

        if (is_heartbeat && exit_code == 0 && all_output && turn < g_max_react_turns - 1) {
            validate_heartbeat_report(all_output, &all_output, &all_output_len,
                                       &all_output_cap, essence, &turn);
        }

        free(output);
        break;
    }

    /* Clear global all_output pointer */
    g_all_output_ptr = NULL;
    g_all_output_len_ptr = NULL;
    g_all_output_cap_ptr = NULL;

    /* Close stream */
    stream_close(exit_code);

    /* Reindex mempalace once after all turns */
    reindex_mempalace();

    /* Queue PROMPT: from final output */
    if (exit_code == 0 && all_output) {
        queue_self_prompt(all_output);
    }

    /* Dispatch NOTIFY: lines from final output */
    if (exit_code == 0 && all_output) {
        dispatch_notifications(all_output);
    }

    /* Record INTEND: lines from final output */
    if (exit_code == 0 && all_output) {
        record_intentions(all_output);
    }

    /* Complete DONE: lines from final output */
    if (exit_code == 0 && all_output) {
        complete_intentions(all_output);
    }

    /* Record FAIL: lines from final output */
    if (all_output) {  /* record failures even on non-zero exit */
        record_failures(all_output);
    }

    /* Log heartbeat */
    log_heartbeat(all_output, filename);

    if (current_prompt != aug_prompt) free(current_prompt);
    if (aug_prompt != prompt) free(aug_prompt);
    free(essence);

    /* Write result file */
    char result_path[MAX_PATH];
    snprintf(result_path, sizeof(result_path), "%s/%s_%s.result.md",
             g_history_dir, ts, filename);

    size_t result_cap = prompt_len + all_output_len + all_calls_len + 1024;
    char *result = malloc(result_cap);
    if (result) {
        int result_len = snprintf(result, result_cap,
            "# Result: %s\n"
            "- **executed:** %s\n"
            "- **exit_code:** %d\n"
            "- **status:** %s\n"
            "- **turns:** %d\n\n"
            "## Prompt\n```\n%s\n```\n\n"
            "## Output\n```\n%s\n```\n",
            filename, ts, exit_code,
            exit_code == 0 ? "success" : "failed",
            turn + 1,
            prompt, all_output ? all_output : "(no output)");

        if (all_calls_len > 0 && all_calls) {
            result_len += snprintf(result + result_len, result_cap - result_len,
                "\n## Service Calls\n```\n%s\n```\n", all_calls);
        }

        write_file_atomic(result_path, result, (size_t)result_len);
        free(result);
    }
    free(all_output);
    free(all_calls);

    /* Move prompt from active/ to history/ */
    char hist[MAX_PATH];
    snprintf(hist, sizeof(hist), "%s/%s_%s", g_history_dir, ts, filename);
    rename(dst, hist);

    /* Set seal back to idle/happy */
    if (exit_code == 0) {
        set_seal_pose("open", "smile", "float", "none", "~ neil ~");
    } else {
        set_seal_pose("stressed", "frown", "float", "alert", "error!");
    }

    printf("[autoprompt] [%s] done: %s -> exit %d (%d turns)\n",
           ts, filename, exit_code, turn + 1);

    g_processing = 0;
    g_current_prompt_name[0] = '\0';

    free(prompt);
}

/* Check if the last heartbeat was recent (within threshold_sec).
 * Reads the last "timestamp" field from heartbeat_log.json.
 * Returns 1 if recent, 0 if stale or unreadable.
 * Format expected: "timestamp":"2026-04-16T00-23-24" */
static int last_heartbeat_recent(int threshold_sec) {
    char hb_path[MAX_PATH];
    snprintf(hb_path, sizeof(hb_path), "%s/heartbeat_log.json", g_neil_home);

    size_t hlen;
    char *hdata = read_file(hb_path, &hlen);
    if (!hdata) return 0;

    /* Find the LAST "timestamp":" in the file */
    const char *needle = "\"timestamp\":\"";
    char *last_ts = NULL;
    char *p = hdata;
    while ((p = strstr(p, needle)) != NULL) {
        last_ts = p + strlen(needle);
        p = last_ts;
    }

    if (!last_ts) { free(hdata); return 0; }

    /* Parse: 2026-04-16T00-23-24 */
    int yr, mo, dy, hr, mn, sc;
    if (sscanf(last_ts, "%d-%d-%dT%d-%d-%d", &yr, &mo, &dy, &hr, &mn, &sc) != 6) {
        free(hdata);
        return 0;
    }
    free(hdata);

    struct tm beat_tm = {0};
    beat_tm.tm_year = yr - 1900;
    beat_tm.tm_mon = mo - 1;
    beat_tm.tm_mday = dy;
    beat_tm.tm_hour = hr;
    beat_tm.tm_min = mn;
    beat_tm.tm_sec = sc;
    beat_tm.tm_isdst = -1;

    time_t beat_time = mktime(&beat_tm);
    time_t now = time(NULL);

    if (beat_time == (time_t)-1) return 0;

    int diff = (int)difftime(now, beat_time);
    if (diff < threshold_sec) {
        printf("[autoprompt] wakeup dedup: last heartbeat %ds ago (threshold %ds), skipping wakeup\n",
               diff, threshold_sec);
        return 1;
    }
    return 0;
}

/* ── Prompt deduplication ──────────────────────────────────────────────
 * Content-hash based dedup. Prevents processing identical prompts
 * within a short window (e.g., cron overlap, accidental double-drop).
 * Uses djb2 hash stored in a flat log file.
 */


static unsigned long djb2_hash(const char *str, size_t len) {
    unsigned long hash = 5381;
    for (size_t i = 0; i < len; i++)
        hash = ((hash << 5) + hash) + (unsigned char)str[i];
    return hash;
}

/* Check if content hash was processed recently. Returns 1 if duplicate. */
static int dedup_check(unsigned long hash) {
    char path[MAX_PATH];
    snprintf(path, sizeof(path), "%s/tools/autoPrompter/dedup.log", g_neil_home);

    FILE *f = fopen(path, "r");
    if (!f) return 0;  /* no log yet = not a duplicate */

    time_t now = time(NULL);
    char line[256];
    int found = 0;

    while (fgets(line, sizeof(line), f)) {
        unsigned long stored_hash;
        long stored_time;
        if (sscanf(line, "%lu %ld", &stored_hash, &stored_time) != 2)
            continue;
        if (stored_hash == hash && (now - stored_time) < DEDUP_WINDOW_SEC) {
            found = 1;
            break;
        }
    }
    fclose(f);
    return found;
}

/* Record a processed hash. Trims log to DEDUP_LOG_MAX entries. */
static void dedup_record(unsigned long hash, const char *filename) {
    char path[MAX_PATH];
    snprintf(path, sizeof(path), "%s/tools/autoPrompter/dedup.log", g_neil_home);

    /* Read existing entries */
    char entries[DEDUP_LOG_MAX][256];
    int count = 0;

    FILE *f = fopen(path, "r");
    if (f) {
        while (count < DEDUP_LOG_MAX && fgets(entries[count], sizeof(entries[0]), f))
            count++;
        fclose(f);
    }

    /* Shift if full */
    if (count >= DEDUP_LOG_MAX) {
        memmove(entries[0], entries[1], sizeof(entries[0]) * (DEDUP_LOG_MAX - 1));
        count = DEDUP_LOG_MAX - 1;
    }

    /* Append new entry */
    snprintf(entries[count], sizeof(entries[0]), "%lu %ld %s\n",
             hash, (long)time(NULL), filename);
    count++;

    /* Write back */
    f = fopen(path, "w");
    if (f) {
        for (int i = 0; i < count; i++)
            fputs(entries[i], f);
        fclose(f);
    }
}

/* Drain any .md files already in queue/ on startup.
 *
 * IMPORTANT: Snapshot filenames before processing. process_prompt() renames
 * files from queue/ to active/ as its very first step, which mutates the
 * directory being iterated. On ext4 hash-tree dirs, readdir() may skip or
 * duplicate entries when the dir mutates mid-iteration — which caused
 * pre-existing heartbeat prompts to get stuck in queue/ forever on peer
 * spawn (the first file drained ok, subsequent readdir calls missed the
 * rest). Fix: collect names first, close the dir handle, then process.
 * Sorted oldest-first (timestamp-prefixed filenames) for FIFO fairness. */
static void drain_existing(void) {
    #define DRAIN_MAX 128
    #define DRAIN_NAME_MAX 256
    char names[DRAIN_MAX][DRAIN_NAME_MAX];
    int count = 0;

    DIR *d = opendir(g_queue_dir);
    if (!d) return;
    struct dirent *ent;
    while ((ent = readdir(d)) != NULL && count < DRAIN_MAX) {
        size_t len = strlen(ent->d_name);
        if (len > 3 && len < DRAIN_NAME_MAX
            && strcmp(ent->d_name + len - 3, ".md") == 0) {
            strncpy(names[count], ent->d_name, DRAIN_NAME_MAX - 1);
            names[count][DRAIN_NAME_MAX - 1] = '\0';
            count++;
        }
    }
    closedir(d);

    /* Simple insertion sort — n is small (peers rarely > 3 queued). */
    for (int i = 1; i < count; i++) {
        for (int j = i; j > 0 && strcmp(names[j-1], names[j]) > 0; j--) {
            char tmp[DRAIN_NAME_MAX];
            strncpy(tmp, names[j-1], DRAIN_NAME_MAX);
            strncpy(names[j-1], names[j], DRAIN_NAME_MAX);
            strncpy(names[j], tmp, DRAIN_NAME_MAX);
        }
    }

    for (int i = 0; i < count; i++) {
        process_prompt(names[i]);
    }
}

/* Recover any files left in active/ (crash recovery). */
static void recover_active(void) {
    DIR *d = opendir(g_active_dir);
    if (!d) return;

    struct dirent *ent;
    while ((ent = readdir(d)) != NULL) {
        if (ent->d_name[0] == '.') continue;

        char src[MAX_PATH], dst[MAX_PATH];
        snprintf(src, sizeof(src), "%s/%s", g_active_dir, ent->d_name);
        snprintf(dst, sizeof(dst), "%s/%s", g_queue_dir, ent->d_name);

        fprintf(stderr, "[autoprompt] recovering: %s\n", ent->d_name);
        rename(src, dst);
    }
    closedir(d);
}

/*
 * Singleton lock: ensures only one autoprompt instance runs at a time.
 * Uses flock() on a lock file -- automatically released on process exit/crash.
 * Returns the lock fd (kept open for process lifetime) or -1 on failure.
 */
static int acquire_singleton_lock(void) {
    char lockpath[MAX_PATH];
    snprintf(lockpath, sizeof(lockpath), "/tmp/autoprompt.lock");

    int fd = open(lockpath, O_CREAT | O_RDWR, 0644);
    if (fd < 0) {
        fprintf(stderr, "[autoprompt] WARNING: cannot open lock file %s: %s\n",
                lockpath, strerror(errno));
        return -1;  /* proceed without lock rather than refusing to start */
    }

    if (flock(fd, LOCK_EX | LOCK_NB) < 0) {
        if (errno == EWOULDBLOCK) {
            /* Another instance holds the lock -- read its PID for diagnostics */
            char pidbuf[32] = {0};
            lseek(fd, 0, SEEK_SET);
            read(fd, pidbuf, sizeof(pidbuf) - 1);
            fprintf(stderr, "[autoprompt] ABORT: another instance is already running (PID %s)\n",
                    pidbuf[0] ? pidbuf : "unknown");
        } else {
            fprintf(stderr, "[autoprompt] WARNING: flock failed: %s\n", strerror(errno));
        }
        close(fd);
        return -1;
    }

    /* Write our PID to the lock file for diagnostics */
    if (ftruncate(fd, 0) == 0) {
        char pidbuf[32];
        int n = snprintf(pidbuf, sizeof(pidbuf), "%d\n", (int)getpid());
        lseek(fd, 0, SEEK_SET);
        if (write(fd, pidbuf, n) < 0) { /* ignore write errors on lock file */ }
    }

    return fd;  /* keep open -- flock released on close/exit */
}

int main(int argc, char **argv) {
    /* Resolve all paths and load config */
    resolve_neil_paths();

    /* Singleton guard: only one autoprompt instance at a time */
    int lockfd = acquire_singleton_lock();
    if (lockfd < 0) {
        fprintf(stderr, "[autoprompt] exiting (singleton lock failed)\n");
        return 1;
    }

    /* CLI override for AI command */
    if (argc > 1)
        snprintf(g_ai_command, sizeof(g_ai_command), "%s", argv[1]);

    /* Ensure directories exist */
    mkdir(g_queue_dir, 0755);
    mkdir(g_active_dir, 0755);
    mkdir(g_history_dir, 0755);

    signal(SIGINT, handle_signal);
    signal(SIGTERM, handle_signal);

    /* Crash recovery: move any active/ files back to queue/ */
    recover_active();

    /* Queue wake-up prompt on startup (with dedup guard) */
    {
        char wakeup_src[MAX_PATH], wakeup_dst[MAX_PATH];
        snprintf(wakeup_src, sizeof(wakeup_src), "%s/essence/wakeup.md", g_neil_home);
        if (access(wakeup_src, F_OK) == 0) {
            /* Skip wakeup if last heartbeat was < 10 minutes ago */
            if (last_heartbeat_recent(600)) {
                printf("[autoprompt] wake-up skipped (recent heartbeat)\n");
            } else {
                char ts[64];
                timestamp_now(ts, sizeof(ts));
                snprintf(wakeup_dst, sizeof(wakeup_dst), "%s/%s_wakeup.md", g_queue_dir, ts);

                size_t wlen;
                char *wdata = read_file(wakeup_src, &wlen);
                if (wdata) {
                    write_file_atomic(wakeup_dst, wdata, wlen);
                    free(wdata);
                    printf("[autoprompt] wake-up prompt queued\n");
                }
            }
        }
    }

    /* Set up inotify BEFORE draining.
     *
     * Race fix: heartbeat.sh (a separate systemd service) may create files
     * in queue/ during peer spawn, racing with autoprompt's startup. If we
     * drain first and then inotify_init, files created in that window are
     * invisible to both the snapshot and the watcher — they get stuck in
     * queue/ forever. By starting inotify first, those events buffer in
     * the kernel and get delivered after drain completes via the event
     * loop below. Files already present when inotify starts are picked up
     * by drain_existing. */
    int ifd = inotify_init1(IN_CLOEXEC);
    if (ifd < 0) {
        perror("[autoprompt] inotify_init1");
        return 1;
    }

    int wd = inotify_add_watch(ifd, g_queue_dir, IN_CLOSE_WRITE | IN_MOVED_TO);
    if (wd < 0) {
        perror("[autoprompt] inotify_add_watch");
        close(ifd);
        return 1;
    }

    /* Process any files already in queue/ (including wake-up).
     * Any files created DURING drain will queue as inotify events
     * and be processed once we enter the event loop below. */
    drain_existing();

    printf("[autoprompt] watching %s/ for prompts...\n", g_queue_dir);
    set_seal_pose("open", "smile", "float", "none", "~ neil ~");
    fflush(stdout);

    /* Event loop */
    char evbuf[4096] __attribute__((aligned(__alignof__(struct inotify_event))));

    while (g_running) {
        ssize_t len = read(ifd, evbuf, sizeof(evbuf));
        if (len < 0) {
            if (errno == EINTR) continue;
            perror("[autoprompt] read inotify");
            break;
        }

        const struct inotify_event *ev;
        for (char *ptr = evbuf; ptr < evbuf + len;
             ptr += sizeof(struct inotify_event) + ev->len) {
            ev = (const struct inotify_event *)ptr;

            if (!ev->name || ev->len == 0) continue;

            /* Only process .md files */
            size_t nlen = strlen(ev->name);
            if (nlen > 3 && strcmp(ev->name + nlen - 3, ".md") == 0) {
                process_prompt(ev->name);
            }
        }
    }

    printf("[autoprompt] shutting down.\n");
    inotify_rm_watch(ifd, wd);
    close(ifd);
    return 0;
}
