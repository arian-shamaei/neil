/*
 * zettel - Zettelkasten note manager
 *
 * Atomic notes as .md files with YAML frontmatter.
 * Bidirectional links, tags, full-text search.
 * File-based index for fast lookups without a database.
 *
 * Directory layout:
 *   notes/        - atomic note files (<id>.md)
 *   index/        - flat-file indexes (tags.idx, links.idx)
 */

#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include <time.h>
#include <dirent.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <ctype.h>

#define DEFAULT_HOME ".zettel"

#define MAX_PATH     4096

/* Resolved paths -- set once at startup */
static char g_base[MAX_PATH];
static char g_notes[MAX_PATH];
static char g_index[MAX_PATH];
static char g_tags_idx[MAX_PATH];
static char g_links_idx[MAX_PATH];
static char g_rooms_idx[MAX_PATH];
#define MAX_NOTE     (256 * 1024)  /* 256 KB max note */
#define MAX_ID       64
#define MAX_LINE     4096
#define MAX_TAGS     64
#define MAX_LINKS    128

/* ── Note structure ──────────────────────────────────────────────── */

typedef struct {
    char id[MAX_ID];
    char created[32];
    char modified[32];
    char wing[64];
    char room[64];
    char tags[MAX_TAGS][64];
    int  ntags;
    char links[MAX_LINKS][MAX_ID];
    int  nlinks;
    char *body;
} Note;

/* ── Utilities ───────────────────────────────────────────────────── */

static void timestamp_now(char *buf, size_t cap) {
    time_t t = time(NULL);
    struct tm *tm = localtime(&t);
    strftime(buf, cap, "%Y-%m-%dT%H:%M:%S", tm);
}

static void generate_id(char *buf, size_t cap) {
    time_t t = time(NULL);
    struct tm *tm = localtime(&t);
    unsigned int r = 0;
    FILE *f = fopen("/dev/urandom", "r");
    if (f) { fread(&r, sizeof(r), 1, f); fclose(f); }
    r &= 0xFFFF;
    snprintf(buf, cap, "%04d%02d%02dT%02d%02d%02d_%04x",
             tm->tm_year + 1900, tm->tm_mon + 1, tm->tm_mday,
             tm->tm_hour, tm->tm_min, tm->tm_sec, r);
}

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
            if (cap > MAX_NOTE) { free(buf); close(fd); return NULL; }
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

static int write_file_atomic(const char *path, const char *data, size_t len) {
    char tmp[MAX_PATH];
    snprintf(tmp, sizeof(tmp), "%s.tmp.%d", path, getpid());

    int fd = open(tmp, O_WRONLY | O_CREAT | O_TRUNC, 0644);
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
    fsync(fd);
    close(fd);
    if (rename(tmp, path) < 0) { unlink(tmp); return -1; }
    return 0;
}

/* Strip leading/trailing whitespace in-place. */
static char *strip(char *s) {
    while (*s && isspace((unsigned char)*s)) s++;
    char *end = s + strlen(s);
    while (end > s && isspace((unsigned char)end[-1])) end--;
    *end = '\0';
    return s;
}

/* ── Note serialization ──────────────────────────────────────────── */

static int note_to_string(const Note *n, char *buf, size_t cap) {
    int off = 0;

    off += snprintf(buf + off, cap - off, "---\n");
    off += snprintf(buf + off, cap - off, "id: %s\n", n->id);
    off += snprintf(buf + off, cap - off, "created: %s\n", n->created);
    off += snprintf(buf + off, cap - off, "modified: %s\n", n->modified);

    if (n->wing[0])
        off += snprintf(buf + off, cap - off, "wing: %s\n", n->wing);
    if (n->room[0])
        off += snprintf(buf + off, cap - off, "room: %s\n", n->room);

    /* tags */
    off += snprintf(buf + off, cap - off, "tags: [");
    for (int i = 0; i < n->ntags; i++) {
        if (i > 0) off += snprintf(buf + off, cap - off, ", ");
        off += snprintf(buf + off, cap - off, "%s", n->tags[i]);
    }
    off += snprintf(buf + off, cap - off, "]\n");

    /* links */
    off += snprintf(buf + off, cap - off, "links: [");
    for (int i = 0; i < n->nlinks; i++) {
        if (i > 0) off += snprintf(buf + off, cap - off, ", ");
        off += snprintf(buf + off, cap - off, "%s", n->links[i]);
    }
    off += snprintf(buf + off, cap - off, "]\n");

    off += snprintf(buf + off, cap - off, "---\n\n");
    if (n->body)
        off += snprintf(buf + off, cap - off, "%s\n", n->body);

    return off;
}

/* Parse YAML array value like "[foo, bar, baz]" into tokens. */
static int parse_array(const char *val, char out[][64], int max) {
    int count = 0;
    const char *p = val;

    /* skip leading [ */
    while (*p && *p != '[') p++;
    if (*p == '[') p++;

    while (*p && *p != ']' && count < max) {
        while (*p && (isspace((unsigned char)*p) || *p == ',')) p++;
        if (*p == ']' || !*p) break;

        const char *start = p;
        while (*p && *p != ',' && *p != ']') p++;

        size_t len = (size_t)(p - start);
        /* trim trailing whitespace */
        while (len > 0 && isspace((unsigned char)start[len - 1])) len--;

        if (len > 0 && len < 64) {
            memcpy(out[count], start, len);
            out[count][len] = '\0';
            count++;
        }
    }
    return count;
}

static int parse_note(const char *data, Note *n) {
    memset(n, 0, sizeof(*n));

    const char *p = data;
    /* expect opening --- */
    while (*p && isspace((unsigned char)*p)) p++;
    if (strncmp(p, "---", 3) != 0) return -1;
    p += 3;
    while (*p == '-') p++;
    if (*p == '\n') p++;

    /* Parse frontmatter lines until closing --- */
    while (*p && strncmp(p, "---", 3) != 0) {
        const char *eol = strchr(p, '\n');
        if (!eol) eol = p + strlen(p);

        char line[MAX_LINE];
        size_t ll = (size_t)(eol - p);
        if (ll >= sizeof(line)) ll = sizeof(line) - 1;
        memcpy(line, p, ll);
        line[ll] = '\0';

        char *colon = strchr(line, ':');
        if (colon) {
            *colon = '\0';
            char *key = strip(line);
            char *val = strip(colon + 1);

            if (strcmp(key, "id") == 0)
                snprintf(n->id, sizeof(n->id), "%s", val);
            else if (strcmp(key, "created") == 0)
                snprintf(n->created, sizeof(n->created), "%s", val);
            else if (strcmp(key, "modified") == 0)
                snprintf(n->modified, sizeof(n->modified), "%s", val);
            else if (strcmp(key, "wing") == 0)
                snprintf(n->wing, sizeof(n->wing), "%s", val);
            else if (strcmp(key, "room") == 0)
                snprintf(n->room, sizeof(n->room), "%s", val);
            else if (strcmp(key, "tags") == 0)
                n->ntags = parse_array(val, n->tags, MAX_TAGS);
            else if (strcmp(key, "links") == 0)
                n->nlinks = parse_array(val, n->links, MAX_LINKS);
        }

        p = (*eol) ? eol + 1 : eol;
    }

    /* skip closing --- */
    if (strncmp(p, "---", 3) == 0) {
        p += 3;
        while (*p == '-') p++;
        if (*p == '\n') p++;
    }

    /* skip blank lines before body */
    while (*p == '\n') p++;

    if (*p) {
        n->body = strdup(p);
        /* trim trailing newline */
        size_t blen = strlen(n->body);
        while (blen > 0 && n->body[blen - 1] == '\n') {
            n->body[--blen] = '\0';
        }
    }

    return 0;
}

static int load_note(const char *id, Note *n) {
    char path[MAX_PATH];
    snprintf(path, sizeof(path), "%s/%s.md", g_notes, id);
    size_t len;
    char *data = read_file(path, &len);
    if (!data) return -1;
    int rc = parse_note(data, n);
    free(data);
    return rc;
}

static int save_note(const Note *n) {
    char buf[MAX_NOTE];
    int len = note_to_string(n, buf, sizeof(buf));
    if (len <= 0) return -1;

    char path[MAX_PATH];
    snprintf(path, sizeof(path), "%s/%s.md", g_notes, n->id);
    return write_file_atomic(path, buf, (size_t)len);
}

static void free_note(Note *n) {
    if (n->body) { free(n->body); n->body = NULL; }
}

/* ── Index ───────────────────────────────────────────────────────── */

static int reindex(void) {
    DIR *d = opendir(g_notes);
    if (!d) { perror("opendir notes"); return -1; }

    FILE *ftags = fopen(g_tags_idx, "w");
    FILE *flinks = fopen(g_links_idx, "w");
    FILE *frooms = fopen(g_rooms_idx, "w");
    if (!ftags || !flinks || !frooms) {
        if (ftags) fclose(ftags);
        if (flinks) fclose(flinks);
        if (frooms) fclose(frooms);
        closedir(d);
        return -1;
    }

    struct dirent *ent;
    while ((ent = readdir(d)) != NULL) {
        size_t nlen = strlen(ent->d_name);
        if (nlen <= 3 || strcmp(ent->d_name + nlen - 3, ".md") != 0)
            continue;

        char id[MAX_ID];
        snprintf(id, sizeof(id), "%.*s", (int)(nlen - 3), ent->d_name);

        Note n;
        if (load_note(id, &n) < 0) continue;

        for (int i = 0; i < n.ntags; i++)
            fprintf(ftags, "%s\t%s\n", n.tags[i], n.id);

        for (int i = 0; i < n.nlinks; i++)
            fprintf(flinks, "%s\t%s\n", n.id, n.links[i]);

        if (n.wing[0])
            fprintf(frooms, "%s\t%s\t%s\n", n.wing, n.room[0] ? n.room : "-", n.id);

        free_note(&n);
    }

    fclose(ftags);
    fclose(flinks);
    fclose(frooms);
    closedir(d);
    return 0;
}

/* ── Commands ────────────────────────────────────────────────────── */

static int cmd_new(int argc, char **argv) {
    if (argc < 1) {
        fprintf(stderr, "usage: zettel new \"content\" [--tags \"t1,t2\"]\n");
        return 1;
    }

    Note n;
    memset(&n, 0, sizeof(n));

    generate_id(n.id, sizeof(n.id));
    timestamp_now(n.created, sizeof(n.created));
    snprintf(n.modified, sizeof(n.modified), "%s", n.created);
    n.body = strdup(argv[0]);

    /* parse --tags, --wing, --room */
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--tags") == 0 && i + 1 < argc) {
            char *copy = strdup(argv[++i]);
            char *tok = strtok(copy, ",");
            while (tok && n.ntags < MAX_TAGS) {
                char *t = strip(tok);
                if (*t) snprintf(n.tags[n.ntags++], 64, "%s", t);
                tok = strtok(NULL, ",");
            }
            free(copy);
        } else if (strcmp(argv[i], "--wing") == 0 && i + 1 < argc) {
            snprintf(n.wing, sizeof(n.wing), "%s", argv[++i]);
        } else if (strcmp(argv[i], "--room") == 0 && i + 1 < argc) {
            snprintf(n.room, sizeof(n.room), "%s", argv[++i]);
        }
    }

    if (save_note(&n) < 0) {
        fprintf(stderr, "failed to save note\n");
        free_note(&n);
        return 1;
    }

    printf("%s\n", n.id);
    free_note(&n);
    reindex();
    return 0;
}

static int cmd_show(int argc, char **argv) {
    if (argc < 1) {
        fprintf(stderr, "usage: zettel show <id>\n");
        return 1;
    }

    Note n;
    if (load_note(argv[0], &n) < 0) {
        fprintf(stderr, "note not found: %s\n", argv[0]);
        return 1;
    }

    printf("id:       %s\n", n.id);
    printf("created:  %s\n", n.created);
    printf("modified: %s\n", n.modified);
    if (n.wing[0]) printf("wing:     %s\n", n.wing);
    if (n.room[0]) printf("room:     %s\n", n.room);

    printf("tags:     ");
    for (int i = 0; i < n.ntags; i++)
        printf("%s%s", i ? ", " : "", n.tags[i]);
    printf("\n");

    printf("links:    ");
    for (int i = 0; i < n.nlinks; i++)
        printf("%s%s", i ? ", " : "", n.links[i]);
    printf("\n");

    if (n.body)
        printf("\n%s\n", n.body);

    free_note(&n);
    return 0;
}

static int has_link(const Note *n, const char *id) {
    for (int i = 0; i < n->nlinks; i++)
        if (strcmp(n->links[i], id) == 0) return 1;
    return 0;
}

static int cmd_link(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: zettel link <id1> <id2>\n");
        return 1;
    }

    Note a, b;
    if (load_note(argv[0], &a) < 0) {
        fprintf(stderr, "note not found: %s\n", argv[0]);
        return 1;
    }
    if (load_note(argv[1], &b) < 0) {
        fprintf(stderr, "note not found: %s\n", argv[1]);
        free_note(&a);
        return 1;
    }

    int changed = 0;

    /* a -> b */
    if (!has_link(&a, b.id) && a.nlinks < MAX_LINKS) {
        snprintf(a.links[a.nlinks++], MAX_ID, "%s", b.id);
        timestamp_now(a.modified, sizeof(a.modified));
        changed = 1;
    }

    /* b -> a */
    if (!has_link(&b, a.id) && b.nlinks < MAX_LINKS) {
        snprintf(b.links[b.nlinks++], MAX_ID, "%s", a.id);
        timestamp_now(b.modified, sizeof(b.modified));
        changed = 1;
    }

    if (changed) {
        save_note(&a);
        save_note(&b);
        reindex();
        printf("linked %s <-> %s\n", a.id, b.id);
    } else {
        printf("already linked\n");
    }

    free_note(&a);
    free_note(&b);
    return 0;
}

/* Check if note ID is in wing (and optionally room) using rooms.idx. */
static int note_in_scope(const char *id, const char *wing, const char *room) {
    if (!wing || !wing[0]) return 1; /* no filter = everything matches */

    FILE *f = fopen(g_rooms_idx, "r");
    if (!f) return 0;

    char line[MAX_LINE];
    int found = 0;
    while (fgets(line, sizeof(line), f)) {
        char *t1 = strchr(line, '\t');
        if (!t1) continue;
        *t1 = '\0';
        char *t2 = strchr(t1 + 1, '\t');
        if (!t2) continue;
        *t2 = '\0';

        char *w = strip(line);
        char *r = strip(t1 + 1);
        char *nid = strip(t2 + 1);

        if (strcmp(w, wing) != 0) continue;
        if (room && room[0] && strcmp(r, room) != 0) continue;
        if (strcmp(nid, id) == 0) { found = 1; break; }
    }
    fclose(f);
    return found;
}

static int cmd_find(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: zettel find [--wing <w>] [--room <r>] --tag <tag> | --text <keyword>\n");
        return 1;
    }

    const char *wing_filter = NULL;
    const char *room_filter = NULL;
    const char *query = NULL;
    int by_tag = 0, by_text = 0;

    for (int i = 0; i < argc; i++) {
        if (strcmp(argv[i], "--wing") == 0 && i + 1 < argc)
            wing_filter = argv[++i];
        else if (strcmp(argv[i], "--room") == 0 && i + 1 < argc)
            room_filter = argv[++i];
        else if (strcmp(argv[i], "--tag") == 0 && i + 1 < argc) {
            by_tag = 1; query = argv[++i];
        } else if (strcmp(argv[i], "--text") == 0 && i + 1 < argc) {
            by_text = 1; query = argv[++i];
        }
    }

    if (!by_tag && !by_text) {
        fprintf(stderr, "use --tag or --text\n");
        return 1;
    }

    if (by_tag) {
        FILE *f = fopen(g_tags_idx, "r");
        if (!f) {
            reindex();
            f = fopen(g_tags_idx, "r");
            if (!f) return 1;
        }
        char line[MAX_LINE];
        while (fgets(line, sizeof(line), f)) {
            char *tab = strchr(line, '\t');
            if (!tab) continue;
            *tab = '\0';
            char *tag = strip(line);
            char *id = strip(tab + 1);
            if (strcmp(tag, query) == 0 && note_in_scope(id, wing_filter, room_filter))
                printf("%s\n", id);
        }
        fclose(f);
        return 0;
    }

    /* Full-text search */
    DIR *d = opendir(g_notes);
    if (!d) return 1;

    struct dirent *ent;
    while ((ent = readdir(d)) != NULL) {
        size_t nlen = strlen(ent->d_name);
        if (nlen <= 3 || strcmp(ent->d_name + nlen - 3, ".md") != 0)
            continue;

        char path[MAX_PATH];
        snprintf(path, sizeof(path), "%s/%s", g_notes, ent->d_name);

        size_t flen;
        char *data = read_file(path, &flen);
        if (!data) continue;

        if (strcasestr(data, query)) {
            char id[MAX_ID];
            snprintf(id, sizeof(id), "%.*s", (int)(nlen - 3), ent->d_name);

            if (note_in_scope(id, wing_filter, room_filter)) {
                Note n;
                if (parse_note(data, &n) == 0) {
                    char preview[80];
                    if (n.body) {
                        snprintf(preview, sizeof(preview), "%s", n.body);
                        char *nl = strchr(preview, '\n');
                        if (nl) *nl = '\0';
                    } else {
                        preview[0] = '\0';
                    }
                    printf("%s  %s\n", id, preview);
                    free_note(&n);
                }
            }
        }
        free(data);
    }
    closedir(d);
    return 0;
}

static int cmd_graph(int argc, char **argv) {
    if (argc < 1) {
        fprintf(stderr, "usage: zettel graph <id> [depth]\n");
        return 1;
    }

    const char *root_id = argv[0];
    int depth = (argc > 1) ? atoi(argv[1]) : 2;
    if (depth < 1) depth = 1;
    if (depth > 5) depth = 5;

    /* BFS with visited set (simple linear scan -- fine for <1000 notes) */
    typedef struct { char id[MAX_ID]; int depth; } QItem;
    QItem queue[1024];
    char visited[1024][MAX_ID];
    int qhead = 0, qtail = 0, nvisited = 0;

    snprintf(queue[qtail].id, MAX_ID, "%s", root_id);
    queue[qtail].depth = 0;
    qtail++;
    snprintf(visited[nvisited++], MAX_ID, "%s", root_id);

    while (qhead < qtail) {
        QItem cur = queue[qhead++];

        Note n;
        if (load_note(cur.id, &n) < 0) continue;

        /* indent by depth */
        for (int i = 0; i < cur.depth; i++) printf("  ");

        /* show note summary */
        char preview[60] = "";
        if (n.body) {
            snprintf(preview, sizeof(preview), "%s", n.body);
            char *nl = strchr(preview, '\n');
            if (nl) *nl = '\0';
        }
        printf("%s%s  %s\n", cur.depth == 0 ? "" : "-> ", n.id, preview);

        /* enqueue links if within depth */
        if (cur.depth < depth) {
            for (int i = 0; i < n.nlinks; i++) {
                /* check visited */
                int seen = 0;
                for (int v = 0; v < nvisited; v++) {
                    if (strcmp(visited[v], n.links[i]) == 0) { seen = 1; break; }
                }
                if (!seen && qtail < 1024 && nvisited < 1024) {
                    snprintf(queue[qtail].id, MAX_ID, "%s", n.links[i]);
                    queue[qtail].depth = cur.depth + 1;
                    qtail++;
                    snprintf(visited[nvisited++], MAX_ID, "%s", n.links[i]);
                }
            }
        }
        free_note(&n);
    }
    return 0;
}

/* Escape a string for JSON output: standard escapes for ASCII, validate
 * UTF-8 multi-byte sequences and emit � for malformed ones (note
 * bodies can contain mojibake from arbitrary clipboard input). Streams
 * straight to FILE* so we don't have to build a buffer. */
static void json_escape(FILE *f, const char *s) {
    if (!s) return;
    const unsigned char *p = (const unsigned char *)s;
    while (*p) {
        unsigned char c = *p;
        if (c == '"')  { fputs("\\\"", f); p++; continue; }
        if (c == '\\') { fputs("\\\\", f); p++; continue; }
        if (c == '\n') { fputs("\\n",  f); p++; continue; }
        if (c == '\r') { fputs("\\r",  f); p++; continue; }
        if (c == '\t') { fputs("\\t",  f); p++; continue; }
        if (c < 0x20)  { fprintf(f, "\\u%04x", c); p++; continue; }
        if (c < 0x80)  { fputc(c, f); p++; continue; }

        /* Multi-byte UTF-8 — validate length + continuation bytes. */
        int n_bytes;
        if      ((c & 0xE0) == 0xC0) n_bytes = 2;
        else if ((c & 0xF0) == 0xE0) n_bytes = 3;
        else if ((c & 0xF8) == 0xF0) n_bytes = 4;
        else { fputs("\\ufffd", f); p++; continue; }

        int valid = 1;
        for (int k = 1; k < n_bytes; k++) {
            if (p[k] == '\0' || (p[k] & 0xC0) != 0x80) { valid = 0; break; }
        }
        if (!valid) { fputs("\\ufffd", f); p++; continue; }

        for (int k = 0; k < n_bytes; k++) fputc(p[k], f);
        p += n_bytes;
    }
}

static int cmd_list(int argc, char **argv) {
    const char *wing_filter = NULL;
    int as_json = 0;

    for (int i = 0; i < argc; i++) {
        if (strcmp(argv[i], "--wing") == 0 && i + 1 < argc)
            wing_filter = argv[++i];
        else if (strcmp(argv[i], "--json") == 0)
            as_json = 1;
    }

    DIR *d = opendir(g_notes);
    if (!d) {
        if (as_json) printf("{\"notes\":[]}\n");
        return 1;
    }

    char ids[4096][MAX_ID];
    int count = 0;

    struct dirent *ent;
    while ((ent = readdir(d)) != NULL && count < 4096) {
        size_t nlen = strlen(ent->d_name);
        if (nlen <= 3 || strcmp(ent->d_name + nlen - 3, ".md") != 0)
            continue;
        snprintf(ids[count++], MAX_ID, "%.*s", (int)(nlen - 3), ent->d_name);
    }
    closedir(d);

    /* sort descending (most recent first) */
    for (int i = 0; i < count - 1; i++)
        for (int j = i + 1; j < count; j++)
            if (strcmp(ids[i], ids[j]) < 0) {
                char tmp[MAX_ID];
                memcpy(tmp, ids[i], MAX_ID);
                memcpy(ids[i], ids[j], MAX_ID);
                memcpy(ids[j], tmp, MAX_ID);
            }

    if (as_json) {
        printf("{\"notes\":[");
        int emitted = 0;
        for (int i = 0; i < count; i++) {
            if (wing_filter && !note_in_scope(ids[i], wing_filter, NULL))
                continue;

            Note n;
            if (load_note(ids[i], &n) < 0) continue;

            char preview[120] = "";
            if (n.body) {
                snprintf(preview, sizeof(preview), "%s", n.body);
                char *nl = strchr(preview, '\n');
                if (nl) *nl = '\0';
            }

            if (emitted) fputc(',', stdout);
            emitted++;

            fputs("{\"id\":\"", stdout);          json_escape(stdout, n.id);
            fputs("\",\"wing\":\"", stdout);      json_escape(stdout, n.wing);
            fputs("\",\"room\":\"", stdout);      json_escape(stdout, n.room);
            fputs("\",\"preview\":\"", stdout);   json_escape(stdout, preview);
            fputs("\",\"tags\":[", stdout);
            for (int t = 0; t < n.ntags; t++) {
                if (t) fputc(',', stdout);
                fputc('"', stdout); json_escape(stdout, n.tags[t]); fputc('"', stdout);
            }
            fputs("],\"links\":[", stdout);
            for (int t = 0; t < n.nlinks; t++) {
                if (t) fputc(',', stdout);
                fputc('"', stdout); json_escape(stdout, n.links[t]); fputc('"', stdout);
            }
            fputs("]}", stdout);
            free_note(&n);
        }
        printf("]}\n");
        return 0;
    }

    for (int i = 0; i < count; i++) {
        if (wing_filter && !note_in_scope(ids[i], wing_filter, NULL))
            continue;

        Note n;
        if (load_note(ids[i], &n) < 0) continue;

        char preview[60] = "";
        if (n.body) {
            snprintf(preview, sizeof(preview), "%s", n.body);
            char *nl = strchr(preview, '\n');
            if (nl) *nl = '\0';
        }

        printf("%s  [%s/%s] [%d tags, %d links]  %s\n",
               n.id,
               n.wing[0] ? n.wing : "-",
               n.room[0] ? n.room : "-",
               n.ntags, n.nlinks, preview);
        free_note(&n);
    }
    return 0;
}

static int cmd_rm(int argc, char **argv) {
    if (argc < 1) {
        fprintf(stderr, "usage: zettel rm <id>\n");
        return 1;
    }

    const char *target = argv[0];

    /* Load target to get its links */
    Note n;
    if (load_note(target, &n) < 0) {
        fprintf(stderr, "note not found: %s\n", target);
        return 1;
    }

    /* Remove backlinks from linked notes */
    for (int i = 0; i < n.nlinks; i++) {
        Note linked;
        if (load_note(n.links[i], &linked) < 0) continue;

        /* Remove target from linked note's links */
        int removed = 0;
        for (int j = 0; j < linked.nlinks; j++) {
            if (strcmp(linked.links[j], target) == 0) {
                /* shift remaining links down */
                for (int k = j; k < linked.nlinks - 1; k++)
                    memcpy(linked.links[k], linked.links[k + 1], MAX_ID);
                linked.nlinks--;
                removed = 1;
                j--;
            }
        }
        if (removed) {
            timestamp_now(linked.modified, sizeof(linked.modified));
            save_note(&linked);
        }
        free_note(&linked);
    }

    free_note(&n);

    /* Delete the file */
    char path[MAX_PATH];
    snprintf(path, sizeof(path), "%s/%s.md", g_notes, target);
    if (unlink(path) < 0) {
        perror("unlink");
        return 1;
    }

    printf("removed %s\n", target);
    reindex();
    return 0;
}

/* ── Context (layered loading L0/L1) ────────────────────────────── */

static int cmd_context(int argc, char **argv) {
    const char *wing_filter = NULL;

    for (int i = 0; i < argc; i++) {
        if (strcmp(argv[i], "--wing") == 0 && i + 1 < argc)
            wing_filter = argv[++i];
    }

    /* Build wing/room tree from rooms.idx */
    typedef struct { char name[64]; int count; } WingInfo;
    typedef struct { char wing[64]; char room[64]; int count; } RoomInfo;
    WingInfo wings[64];
    RoomInfo rooms[256];
    int nwings = 0, nrooms = 0;

    FILE *f = fopen(g_rooms_idx, "r");
    if (f) {
        char line[MAX_LINE];
        while (fgets(line, sizeof(line), f)) {
            char *t1 = strchr(line, '\t');
            if (!t1) continue;
            *t1 = '\0';
            char *t2 = strchr(t1 + 1, '\t');
            if (!t2) continue;
            *t2 = '\0';

            char *w = strip(line);
            char *r = strip(t1 + 1);

            /* update wing count */
            int wi = -1;
            for (int i = 0; i < nwings; i++) {
                if (strcmp(wings[i].name, w) == 0) { wi = i; break; }
            }
            if (wi < 0 && nwings < 64) {
                wi = nwings;
                snprintf(wings[nwings].name, 64, "%s", w);
                wings[nwings].count = 0;
                nwings++;
            }
            if (wi >= 0) wings[wi].count++;

            /* update room count */
            int ri = -1;
            for (int i = 0; i < nrooms; i++) {
                if (strcmp(rooms[i].wing, w) == 0 && strcmp(rooms[i].room, r) == 0) {
                    ri = i; break;
                }
            }
            if (ri < 0 && nrooms < 256) {
                ri = nrooms;
                snprintf(rooms[nrooms].wing, 64, "%s", w);
                snprintf(rooms[nrooms].room, 64, "%s", r);
                rooms[nrooms].count = 0;
                nrooms++;
            }
            if (ri >= 0) rooms[ri].count++;
        }
        fclose(f);
    }

    /* Also count unclassified notes */
    int total_notes = 0, classified = 0;
    DIR *d = opendir(g_notes);
    if (d) {
        struct dirent *ent;
        while ((ent = readdir(d)) != NULL) {
            size_t nlen = strlen(ent->d_name);
            if (nlen > 3 && strcmp(ent->d_name + nlen - 3, ".md") == 0)
                total_notes++;
        }
        closedir(d);
    }
    for (int i = 0; i < nwings; i++) classified += wings[i].count;

    if (!wing_filter) {
        /* L0: palace overview */
        printf("palace: %d notes, %d wings, %d classified, %d unclassified\n",
               total_notes, nwings, classified, total_notes - classified);
        for (int i = 0; i < nwings; i++) {
            printf("  wing/%s: %d notes", wings[i].name, wings[i].count);
            /* list rooms inline */
            int first = 1;
            for (int j = 0; j < nrooms; j++) {
                if (strcmp(rooms[j].wing, wings[i].name) == 0) {
                    printf("%s %s(%d)", first ? " ->" : ",", rooms[j].room, rooms[j].count);
                    first = 0;
                }
            }
            printf("\n");
        }
    } else {
        /* L1: wing detail */
        printf("wing/%s:\n", wing_filter);
        for (int j = 0; j < nrooms; j++) {
            if (strcmp(rooms[j].wing, wing_filter) != 0) continue;
            printf("  room/%s: %d notes\n", rooms[j].room, rooms[j].count);
        }
        /* show 3 most recent notes in this wing */
        printf("recent:\n");
        char ids[4096][MAX_ID];
        int count = 0;
        d = opendir(g_notes);
        if (d) {
            struct dirent *ent;
            while ((ent = readdir(d)) != NULL && count < 4096) {
                size_t nlen = strlen(ent->d_name);
                if (nlen > 3 && strcmp(ent->d_name + nlen - 3, ".md") == 0)
                    snprintf(ids[count++], MAX_ID, "%.*s", (int)(nlen - 3), ent->d_name);
            }
            closedir(d);
        }
        /* sort descending */
        for (int i = 0; i < count - 1; i++)
            for (int j = i + 1; j < count; j++)
                if (strcmp(ids[i], ids[j]) < 0) {
                    char tmp[MAX_ID];
                    memcpy(tmp, ids[i], MAX_ID);
                    memcpy(ids[i], ids[j], MAX_ID);
                    memcpy(ids[j], tmp, MAX_ID);
                }
        int shown = 0;
        for (int i = 0; i < count && shown < 3; i++) {
            if (!note_in_scope(ids[i], wing_filter, NULL)) continue;
            Note n;
            if (load_note(ids[i], &n) < 0) continue;
            char preview[60] = "";
            if (n.body) {
                snprintf(preview, sizeof(preview), "%s", n.body);
                char *nl = strchr(preview, '\n');
                if (nl) *nl = '\0';
            }
            printf("  %s [%s] %s\n", n.id, n.room[0] ? n.room : "-", preview);
            free_note(&n);
            shown++;
        }
    }
    return 0;
}

/* ── Path resolution ─────────────────────────────────────────────── */

/*
 * Resolve base directory. Priority:
 *   1. --dir <path> flag
 *   2. ZETTEL_HOME environment variable
 *   3. ~/.zettel (default)
 */
static void resolve_paths(const char *dir_override) {
    if (dir_override) {
        snprintf(g_base, sizeof(g_base), "%s", dir_override);
    } else {
        const char *env = getenv("ZETTEL_HOME");
        if (env && *env) {
            snprintf(g_base, sizeof(g_base), "%s", env);
        } else {
            const char *home = getenv("HOME");
            if (!home) home = "/tmp";
            snprintf(g_base, sizeof(g_base), "%s/%s", home, DEFAULT_HOME);
        }
    }

    snprintf(g_notes, sizeof(g_notes), "%s/notes", g_base);
    snprintf(g_index, sizeof(g_index), "%s/index", g_base);
    snprintf(g_tags_idx, sizeof(g_tags_idx), "%s/index/tags.idx", g_base);
    snprintf(g_links_idx, sizeof(g_links_idx), "%s/index/links.idx", g_base);
    snprintf(g_rooms_idx, sizeof(g_rooms_idx), "%s/index/rooms.idx", g_base);
}

/* ── Main ────────────────────────────────────────────────────────── */

static void usage(void) {
    fprintf(stderr,
        "usage: zettel [--dir <path>] <command> [args]\n\n"
        "commands:\n"
        "  new     \"content\" [--tags t] [--wing w] [--room r]  create a note\n"
        "  show    <id>                             display a note\n"
        "  link    <id1> <id2>                      bidirectional link\n"
        "  find    [--wing w] [--room r] --tag <t>  find by tag (scoped)\n"
        "  find    [--wing w] [--room r] --text <k> full-text search (scoped)\n"
        "  graph   <id> [depth]                     walk links (default 2)\n"
        "  list    [--wing w]                       list notes (optionally scoped)\n"
        "  context [--wing w]                       L0/L1 palace overview\n"
        "  rm      <id>                             remove note + clean backlinks\n"
        "  reindex                                  rebuild index files\n"
        "\n"
        "data directory (in priority order):\n"
        "  --dir <path>       explicit path\n"
        "  ZETTEL_HOME        environment variable\n"
        "  ~/.zettel          default\n"
    );
}

int main(int argc, char **argv) {
    if (argc < 2) { usage(); return 1; }

    /* Parse --dir before the command */
    const char *dir_override = NULL;
    int cmd_idx = 1;

    if (strcmp(argv[1], "--dir") == 0) {
        if (argc < 4) { usage(); return 1; }
        dir_override = argv[2];
        cmd_idx = 3;
    }

    if (cmd_idx >= argc) { usage(); return 1; }

    resolve_paths(dir_override);

    /* Ensure directories exist */
    mkdir(g_base, 0755);
    mkdir(g_notes, 0755);
    mkdir(g_index, 0755);

    const char *cmd = argv[cmd_idx];
    int cargc = argc - cmd_idx - 1;
    char **cargv = argv + cmd_idx + 1;

    if (strcmp(cmd, "new") == 0)       return cmd_new(cargc, cargv);
    if (strcmp(cmd, "show") == 0)      return cmd_show(cargc, cargv);
    if (strcmp(cmd, "link") == 0)      return cmd_link(cargc, cargv);
    if (strcmp(cmd, "find") == 0)      return cmd_find(cargc, cargv);
    if (strcmp(cmd, "graph") == 0)     return cmd_graph(cargc, cargv);
    if (strcmp(cmd, "list") == 0)      return cmd_list(cargc, cargv);
    if (strcmp(cmd, "context") == 0)   return cmd_context(cargc, cargv);
    if (strcmp(cmd, "rm") == 0)        return cmd_rm(cargc, cargv);
    if (strcmp(cmd, "reindex") == 0)   { reindex(); printf("done\n"); return 0; }

    fprintf(stderr, "unknown command: %s\n", cmd);
    usage();
    return 1;
}
