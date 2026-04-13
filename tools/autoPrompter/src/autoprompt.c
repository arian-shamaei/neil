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
#include <fcntl.h>
#include <ctype.h>

#define QUEUE_DIR   "queue"
#define ACTIVE_DIR  "active"
#define HISTORY_DIR "history"

#define MAX_PATH    4096
#define MAX_PROMPT  (1024 * 1024)  /* 1 MB max prompt */
#define READ_BUF    (1024 * 64)

#define CLAUDE_BIN_DEFAULT "claude"
#define NEIL_HOME_DEFAULT ".neil"

static const char *claude_bin = CLAUDE_BIN_DEFAULT;

/* Resolved paths -- set once at startup from NEIL_HOME env var */
static char g_neil_home[MAX_PATH];
static char g_zettel_bin[MAX_PATH];
static char g_mempalace_venv[MAX_PATH];
static char g_mempalace_palace[MAX_PATH];
static char g_services_registry[MAX_PATH];
static char g_services_vault[MAX_PATH];
static char g_essence_dir[MAX_PATH];
static char g_observe_sh[MAX_PATH];
static char g_heartbeat_log[MAX_PATH];

static volatile sig_atomic_t g_running = 1;

/* Forward declarations */
static void timestamp_now(char *buf, size_t cap);

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

    /* Also set ZETTEL_HOME if not already set */
    if (!getenv("ZETTEL_HOME")) {
        char zettel_home[MAX_PATH];
        snprintf(zettel_home, sizeof(zettel_home),
            "%s/memory/palace", g_neil_home);
        setenv("ZETTEL_HOME", zettel_home, 0);
    }
}

static void handle_signal(int sig) {
    (void)sig;
    g_running = 0;
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

/* Extract first 80 chars of the prompt body for use as search query. */
static void extract_query(const char *prompt, char *query, size_t cap) {
    /* skip whitespace */
    while (*prompt && isspace((unsigned char)*prompt)) prompt++;
    size_t i;
    for (i = 0; i < cap - 1 && prompt[i] && prompt[i] != '\n'; i++)
        query[i] = prompt[i];
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
static char *build_augmented_prompt(const char *raw_prompt, char **out_essence) {
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
        ". %s && mempalace --palace %s search '%s' --results 3 2>/dev/null",
        g_mempalace_venv, g_mempalace_palace, escaped);
    char *memories = run_command(search_cmd);

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
        if (!pipe) { p = eol; continue; }

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
        free(result);
    }
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
        if (!pipe) { p = eol; continue; }

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
            p = eol; continue;
        }

        /* Parse individual params as NEIL_PARAM_<key> env vars */
        char cmd[8192];
        int n = snprintf(cmd, sizeof(cmd),
            "NEIL_CHANNEL='%s' NEIL_MESSAGE='%s' NEIL_PARAMS='%s' ",
            channel, message, params_raw);

        /* Parse key=value into NEIL_PARAM_key=value */
        char params_copy[1024];
        snprintf(params_copy, sizeof(params_copy), "%s", params_raw);
        char *ptok = strtok(params_copy, " ");
        while (ptok) {
            char *eq = strchr(ptok, '=');
            if (eq) {
                *eq = '\0';
                n += snprintf(cmd + n, sizeof(cmd) - n,
                    "NEIL_PARAM_%s='%s' ", ptok, eq + 1);
            }
            ptok = strtok(NULL, " ");
        }

        n += snprintf(cmd + n, sizeof(cmd) - n, "%s 2>&1", ch_path);

        fprintf(stderr, "[autoprompt] NOTIFY: channel=%s\n", channel);
        char *result = run_command(cmd);
        if (result) {
            free(result);
        }

        p = eol;
    }
}

/* Parse INTEND: lines and append to intentions.json */
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

        /* Parse params: priority=, after=, tag= */
        char priority[32] = "medium";
        char after[32] = "";
        char tag[64] = "";

        char *tok = strtok(line, " ");
        while (tok) {
            if (strncmp(tok, "priority=", 9) == 0)
                snprintf(priority, sizeof(priority), "%s", tok + 9);
            else if (strncmp(tok, "after=", 6) == 0)
                snprintf(after, sizeof(after), "%s", tok + 6);
            else if (strncmp(tok, "tag=", 4) == 0)
                snprintf(tag, sizeof(tag), "%s", tok + 4);
            tok = strtok(NULL, " ");
        }

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
            else secs = val * 60; /* default minutes */

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

        char ts[32];
        timestamp_now(ts, sizeof(ts));

        char entry[4096];
        int elen = snprintf(entry, sizeof(entry),
            "{\"created\":\"%s\",\"priority\":\"%s\",\"due\":\"%s\","
            "\"tag\":\"%s\",\"description\":\"%s\",\"status\":\"pending\"}\n",
            ts, priority, due, tag, esc_desc);

        int fd = open(intentions_path, O_WRONLY | O_CREAT | O_APPEND, 0644);
        if (fd >= 0) {
            ssize_t w = write(fd, entry, (size_t)elen);
            (void)w;
            close(fd);
        }

        fprintf(stderr, "[autoprompt] INTEND: %s [%s] due:%s\n",
                esc_desc, priority, due[0] ? due : "now");

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
                keyword[i] == ']' || keyword[i] == '*') {
                esc_kw[ei++] = '\\';
            }
            esc_kw[ei++] = keyword[i];
        }
        esc_kw[ei] = '\0';

        /* Use sed to replace first matching pending line */
        char cmd[2048];
        snprintf(cmd, sizeof(cmd),
            "sed -i '0,/%s.*\"pending\"/{s/\"status\":\"pending\"/\"status\":\"completed\"/}' %s",
            esc_kw, intentions_path);

        char *result = run_command(cmd);
        free(result);
        fprintf(stderr, "[autoprompt] DONE: %s\n", keyword);

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
        snprintf(path, sizeof(path), "%s/%s_self.md", QUEUE_DIR, ts);

        write_file_atomic(path, p, len);
        fprintf(stderr, "[autoprompt] self-prompt queued: %s\n", path);

        /* Only queue ONE prompt per cycle to prevent runaway */
        return;
    }
}

/* Log heartbeat status to ~/.neil/heartbeat_log.json */
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

    char ts[64];
    timestamp_now(ts, sizeof(ts));

    char log_entry[1024];
    int log_len = snprintf(log_entry, sizeof(log_entry),
        "{\"timestamp\":\"%s\",\"prompt\":\"%s\",\"status\":\"%s\",\"summary\":\"%s\"}\n",
        ts, filename, status, summary);

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
        char params[2048] = "";
        size_t pi = 0;

        /* Tokenize, respecting quoted values */
        char *s = line;
        while (*s) {
            while (*s == ' ') s++;
            if (!*s) break;

            char key[64] = "", val[512] = "";
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
                /* collect remaining params for the handler */
                pi += snprintf(params + pi, sizeof(params) - pi,
                    "%s%s=%s", pi > 0 ? " " : "", key, val);
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

        /* Dispatch to service handler via shell script */
        char handler_cmd[8192];
        snprintf(handler_cmd, sizeof(handler_cmd),
            "NEIL_SERVICE='%s' NEIL_ACTION='%s' NEIL_CRED='%s' NEIL_PARAMS='%s' "
            "%s/services/handler.sh 2>&1",
            service, action, cred, params, g_neil_home);

        fprintf(stderr, "[autoprompt] CALL: service=%s action=%s\n", service, action);
        char *call_result = run_command(handler_cmd);

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

/* Execute claude --print -p <prompt> --system-prompt <identity>, capture output. */
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
        /* child */
        close(pipefd[0]);
        dup2(pipefd[1], STDOUT_FILENO);
        dup2(pipefd[1], STDERR_FILENO);
        close(pipefd[1]);

        /* Ensure claude's dir is in PATH */
        const char *oldpath = getenv("PATH");
        char newpath[MAX_PATH];
        snprintf(newpath, sizeof(newpath), "%s/.local/bin:%s",
                 getenv("HOME") ? getenv("HOME") : "/tmp",
                 oldpath ? oldpath : "/usr/bin");
        setenv("PATH", newpath, 1);

        if (system_prompt && system_prompt[0]) {
            execlp(claude_bin, claude_bin,
                   "--print",
                   "-p", prompt,
                   "--system-prompt", system_prompt,
                   "--output-format", "text",
                   "--dangerously-skip-permissions",
                   (char *)NULL);
        } else {
            execlp(claude_bin, claude_bin,
                   "--print",
                   "-p", prompt,
                   "--output-format", "text",
                   "--dangerously-skip-permissions",
                   (char *)NULL);
        }
        /* If execl fails, write the error so we can see it */
        dprintf(STDERR_FILENO, "execl failed: %s: %s\n", claude_bin, strerror(errno));
        _exit(127);
    }

    /* parent */
    close(pipefd[1]);

    size_t cap = READ_BUF, len = 0;
    char *buf = malloc(cap);
    if (!buf) { close(pipefd[0]); return -1; }

    ssize_t n;
    while ((n = read(pipefd[0], buf + len, cap - len - 1)) > 0) {
        len += (size_t)n;
        if (len >= cap - 1) {
            cap *= 2;
            char *tmp = realloc(buf, cap);
            if (!tmp) { free(buf); close(pipefd[0]); return -1; }
            buf = tmp;
        }
    }
    close(pipefd[0]);
    buf[len] = '\0';

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
    snprintf(src, sizeof(src), "%s/%s", QUEUE_DIR, filename);
    snprintf(dst, sizeof(dst), "%s/%s", ACTIVE_DIR, filename);

    if (rename(src, dst) < 0) {
        fprintf(stderr, "[autoprompt] move to active failed: %s: %s\n",
                filename, strerror(errno));
        return;
    }

    printf("[autoprompt] [%s] executing: %s\n", ts, filename);

    /* 2. Read prompt content */
    size_t prompt_len;
    char *prompt = read_file(dst, &prompt_len);
    if (!prompt) {
        fprintf(stderr, "[autoprompt] failed to read: %s\n", filename);
        /* move back to queue so it can be retried */
        rename(dst, src);
        return;
    }

    /* 3. Build augmented prompt with context + memories */
    char *essence = NULL;
    char *aug_prompt = build_augmented_prompt(prompt, &essence);

    /*
     * ReAct loop: reason -> act -> observe -> repeat
     * Max 3 iterations to prevent runaway.
     */
    #define MAX_REACT_TURNS 3

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

    for (turn = 0; turn < MAX_REACT_TURNS; turn++) {
        /* Execute claude */
        char *output = NULL;
        size_t output_len = 0;
        exit_code = run_claude(current_prompt, essence, &output, &output_len);

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

        /* Extract and store MEMORY: lines */
        extract_memories(output);

        /* Execute CALL: lines */
        char *call_results = execute_service_calls(output);

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

            /* Build follow-up prompt with call results */
            size_t followup_cap = output_len + cr_len + 256;
            char *followup = malloc(followup_cap);
            if (followup) {
                snprintf(followup, followup_cap,
                    "[PREVIOUS RESPONSE]\n%s\n\n"
                    "[CALL RESULTS]\n%s\n\n"
                    "[INSTRUCTION]\nYou made API calls above. The results are shown. "
                    "Continue your work based on these results. "
                    "You may make more CALL/MEMORY/PROMPT lines as needed.",
                    output, call_results);

                /* Free old prompt if it's not the original */
                if (current_prompt != aug_prompt) free(current_prompt);
                current_prompt = followup;
            }

            free(call_results);
            free(output);
            continue; /* next turn */
        }

        /* No CALL: lines -- loop is done */
        free(output);
        break;
    }

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
             HISTORY_DIR, ts, filename);

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
    snprintf(hist, sizeof(hist), "%s/%s_%s", HISTORY_DIR, ts, filename);
    rename(dst, hist);

    printf("[autoprompt] [%s] done: %s -> exit %d (%d turns)\n",
           ts, filename, exit_code, turn + 1);

    free(prompt);
}

/* Drain any .md files already in queue/ on startup. */
static void drain_existing(void) {
    DIR *d = opendir(QUEUE_DIR);
    if (!d) return;

    struct dirent *ent;
    while ((ent = readdir(d)) != NULL) {
        size_t len = strlen(ent->d_name);
        if (len > 3 && strcmp(ent->d_name + len - 3, ".md") == 0) {
            process_prompt(ent->d_name);
        }
    }
    closedir(d);
}

/* Recover any files left in active/ (crash recovery). */
static void recover_active(void) {
    DIR *d = opendir(ACTIVE_DIR);
    if (!d) return;

    struct dirent *ent;
    while ((ent = readdir(d)) != NULL) {
        if (ent->d_name[0] == '.') continue;

        char src[MAX_PATH], dst[MAX_PATH];
        snprintf(src, sizeof(src), "%s/%s", ACTIVE_DIR, ent->d_name);
        snprintf(dst, sizeof(dst), "%s/%s", QUEUE_DIR, ent->d_name);

        fprintf(stderr, "[autoprompt] recovering: %s\n", ent->d_name);
        rename(src, dst);
    }
    closedir(d);
}

int main(int argc, char **argv) {
    if (argc > 1)
        claude_bin = argv[1];

    /* Resolve all paths from NEIL_HOME */
    resolve_neil_paths();

    /* Ensure directories exist */
    mkdir(QUEUE_DIR, 0755);
    mkdir(ACTIVE_DIR, 0755);
    mkdir(HISTORY_DIR, 0755);

    signal(SIGINT, handle_signal);
    signal(SIGTERM, handle_signal);

    /* Crash recovery: move any active/ files back to queue/ */
    recover_active();

    /* Process any files already in queue/ */
    drain_existing();

    /* Set up inotify */
    int ifd = inotify_init1(IN_CLOEXEC);
    if (ifd < 0) {
        perror("[autoprompt] inotify_init1");
        return 1;
    }

    int wd = inotify_add_watch(ifd, QUEUE_DIR, IN_CLOSE_WRITE | IN_MOVED_TO);
    if (wd < 0) {
        perror("[autoprompt] inotify_add_watch");
        close(ifd);
        return 1;
    }

    printf("[autoprompt] watching %s/ for prompts...\n", QUEUE_DIR);
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
