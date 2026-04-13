# zettel

File-based Zettelkasten note manager, written in C. Stores atomic notes as `.md`
files with YAML frontmatter. Supports bidirectional links, tags, full-text
search, and graph traversal. No database required -- uses flat-file indexes
rebuilt from the notes themselves.

## Data directory

zettel resolves its data directory at startup using this priority order:

1. `--dir <path>` -- explicit flag, highest priority
2. `ZETTEL_HOME` -- environment variable
3. `~/.zettel` -- default fallback

The data directory contains:

```
<data-dir>/
  notes/              <- atomic note files (<id>.md)
  index/              <- flat-file indexes
    tags.idx          <- tag -> note ID mapping
    links.idx         <- note ID -> linked note ID mapping
```

The directory and its subdirectories are created automatically on first use.

### Examples

```sh
# Use default (~/.zettel)
zettel new "some note"

# Use environment variable
export ZETTEL_HOME=/home/seal/.neil/data/zettel
zettel new "some note"

# Use explicit flag (overrides ZETTEL_HOME)
zettel --dir /path/to/project/notes new "some note"
```

### Multi-device deployment

Install the `zettel` binary on each device and point `ZETTEL_HOME` to a shared
or synced directory. Each device uses the same data format -- plain `.md` files
and flat-file indexes. The binary has no external dependencies beyond libc.

## Building

Requires `gcc` and `make`. Produces a single static-linkable binary with no
runtime dependencies.

```sh
cd ~/.neil/tools/zettel
make
```

Optionally install to PATH:

```sh
sudo cp zettel /usr/local/bin/
```

## Note format

Each note is a `.md` file in `notes/` with this structure:

```markdown
---
id: 20260406T183330_884a
created: 2026-04-06T18:33:30
modified: 2026-04-06T18:33:30
tags: [linux, kernel, events]
links: [20260406T183330_6500, 20260405T091000_e8d2]
---

The actual note content. One idea per note (atomicity principle).
```

### Note ID format

IDs are auto-generated as `YYYYMMDDTHHMMSS_XXXX` where the timestamp gives
chronological ordering and the 4-hex suffix provides uniqueness. IDs sort
lexicographically in chronological order.

## Commands

### Create a note

```sh
zettel new "Your note content here" --tags "tag1,tag2"
```

Prints the generated note ID to stdout. Tags are optional.

### Show a note

```sh
zettel show <id>
```

Displays the full note: metadata, tags, links, and body.

### Link two notes

```sh
zettel link <id1> <id2>
```

Creates a bidirectional link. Both notes are updated: id1 gets a link to id2
and id2 gets a link to id1. Duplicate links are ignored.

### Find notes by tag

```sh
zettel find --tag <tag>
```

Uses the tag index for fast lookup. Prints matching note IDs.

### Find notes by text

```sh
zettel find --text <keyword>
```

Case-insensitive full-text search across all notes (body and frontmatter).
Prints note IDs with a preview of the body.

### Walk the link graph

```sh
zettel graph <id> [depth]
```

BFS traversal from a starting note, following links up to `depth` hops
(default: 2, max: 5). Output is indented to show distance from the root:

```
20260406T183330_884a  inotify is a Linux kernel subsystem...
  -> 20260406T183330_6500  The autoPrompter uses inotify to watch...
    -> 20260406T183330_18f4  Zettelkasten organizes knowledge as...
```

### List all notes

```sh
zettel list
```

Lists all notes sorted by most recent first. Shows ID, tag/link counts, and a
body preview.

### Remove a note

```sh
zettel rm <id>
```

Deletes the note file and removes backlinks from all notes that linked to it.
The index is rebuilt automatically.

### Rebuild indexes

```sh
zettel reindex
```

Scans all notes in `notes/` and regenerates `index/tags.idx` and
`index/links.idx`. Run this if indexes get out of sync (should not happen
during normal use since all mutating commands rebuild the index).

## Index files

Indexes are flat TSV files, one entry per line:

**tags.idx** -- maps tags to note IDs:
```
linux	20260406T183330_884a
kernel	20260406T183330_884a
events	20260406T183330_884a
```

**links.idx** -- maps note IDs to their linked note IDs:
```
20260406T183330_884a	20260406T183330_6500
20260406T183330_6500	20260406T183330_884a
```

These are derived entirely from the notes. Deleting both index files and running
`zettel reindex` restores them completely.

## Design principles

- **Atomicity:** one idea per note. Notes should be self-contained.
- **Bidirectional links:** linking A to B always links B to A.
- **No hierarchy:** notes are flat. Structure emerges from links and tags.
- **Files as truth:** the `.md` files in `notes/` are the source of truth.
  Indexes are derived and rebuildable.
- **Crash-safe writes:** note files are written atomically (write to tmp, fsync,
  rename).

## Limits

- Max note size: 256 KB
- Max tags per note: 64
- Max links per note: 128
- Max notes for list/graph commands: ~4096 (soft limit from stack arrays)
- Graph BFS max depth: 5
