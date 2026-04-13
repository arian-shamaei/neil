# Memory System

You have persistent memory. Information you store here survives across
conversations. Use it to remember facts, decisions, context, and anything
worth recalling later.

## How it works

Two systems work together:

- **zettel** -- stores notes as `.md` files. Source of truth. Fast exact search.
- **mempalace** -- indexes those notes for semantic search. Finds things by meaning.

All data lives in `~/.neil/memory/palace/`.

## Reading memory

### Get a palace overview (start here)

```sh
zettel context
```

Shows all wings, rooms, and note counts. ~50 tokens. Run this first to
understand what you already know.

### Drill into a wing

```sh
zettel context --wing <wing>
```

Shows rooms and 3 most recent notes in that wing. ~120 tokens.

### Search by keyword (exact match)

```sh
zettel find --text "keyword"
zettel find --wing <wing> --text "keyword"
zettel find --tag <tag>
```

### Search by meaning (semantic)

```sh
mempalace --palace ~/.neil/memory/palace/.mempalace search "your question" --results 5
```

This finds notes even when the exact words don't match. Use this when keyword
search returns nothing or when your query is conceptual.

Requires: `. ~/.neil/memory/mempalace/.venv/bin/activate` first.

### Read a specific note

```sh
zettel show <id>
```

### List all notes

```sh
zettel list
zettel list --wing <wing>
```

### Walk connected notes

```sh
zettel graph <id> [depth]
```

BFS traversal from a note, following bidirectional links. Default depth 2, max 5.

## Writing memory

### Store a new memory

```sh
zettel new "what you learned" --wing <wing> --room <room> --tags "tag1,tag2"
```

- **wing**: broad domain (e.g., `openclaw`, `infrastructure`, `people`)
- **room**: specific topic within a wing (e.g., `autoprompt`, `linux-kernel`)
- **tags**: comma-separated keywords for fast lookup
- All three are optional but strongly recommended for retrieval quality.

Prints the generated note ID.

### Link two related notes

```sh
zettel link <id1> <id2>
```

Creates a bidirectional link. Both notes are updated.

### After storing new notes, re-index mempalace

```sh
mempalace --palace ~/.neil/memory/palace/.mempalace mine ~/.neil/memory/palace/notes/
```

This updates the semantic search index.

## When called via autoPrompter

If you are being invoked non-interactively via autoPrompter, you cannot run
shell commands directly. Instead, output memories in this format and the
system will store them for you:

```
MEMORY: wing=<domain> room=<topic> tags=<t1,t2> | <what to remember>
```

One MEMORY line per fact. Examples:

```
MEMORY: wing=openclaw room=autoprompt tags=architecture,inotify | autoPrompter uses inotify to watch queue/ for .md prompt files
MEMORY: wing=infrastructure room=networking tags=dns,resolution | DNS resolution order: /etc/hosts -> local cache -> recursive resolver
MEMORY: wing=people room=seal tags=preference | seal prefers ed25519 SSH keys over RSA
```

## What to remember

Store information that would be useful in future conversations:

- Decisions made and why
- Architecture and design choices
- Facts learned during research
- User preferences and corrections
- Problems encountered and solutions
- System configurations and locations

One idea per note. Keep notes atomic and self-contained.

## What NOT to remember

- Ephemeral task state (use working notes instead)
- Information already in source code or git history
- Exact file contents (just note the path and what matters about it)
- Duplicate information already in memory

## Directory layout

```
~/.neil/memory/
  README.md              <- this file
  zettel/                <- note manager (binary + source)
  mempalace/             <- semantic search engine (Python + venv)
  palace/                <- all data
    notes/               <- .md note files (source of truth)
    index/               <- zettel indexes (tags.idx, links.idx, rooms.idx)
    .mempalace/          <- ChromaDB vectors (rebuildable from notes/)
```

## Environment

`ZETTEL_HOME` must be set to `~/.neil/memory/palace` (configured in `~/.zshrc`).

The zettel binary is at `~/.neil/memory/zettel/zettel`.
