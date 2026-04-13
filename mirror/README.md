# Mirror

Local mirror of cloud files with git-tracked change history.
Neil syncs cloud sources, commits changes, and analyzes diffs.

## How it works

```
cloud (Google Drive, Dropbox, S3, etc.)
    │
    ▼  rclone sync (periodic)
~/.neil/mirror/<remote-name>/
    │
    ▼  git commit (automatic after sync)
    │
    ▼  git diff HEAD~1 (what changed?)
    │
    ▼  prompt queued if changes detected
    │
Neil analyzes the diff
```

## Directory layout

```
~/.neil/mirror/
  README.md            this file
  sync.sh              sync script (runs via cron or watcher)
  remotes/             one subdirectory per cloud source
    gdrive/            Google Drive mirror (git-tracked)
    dropbox/           Dropbox mirror (git-tracked)
    ...
```

Each remote directory is an independent git repo. Every sync creates a
commit if files changed, preserving full history.

## Setup a new remote

### 1. Configure rclone

```sh
rclone config
# Follow prompts to add a remote (e.g., "gdrive" for Google Drive)
# For headless servers, use: rclone authorize on a machine with a browser
```

### 2. Register the remote with Neil

```sh
~/.neil/mirror/sync.sh add <remote-name> <rclone-remote>:<path>
# Example: ~/.neil/mirror/sync.sh add gdrive gdrive:/MyProject
```

This creates `remotes/<remote-name>/`, initializes git, and does first sync.

### 3. Enable periodic sync

Add to cron (e.g., every 15 minutes):
```sh
*/15 * * * * ~/.neil/mirror/sync.sh sync >> /tmp/mirror_sync.log 2>&1
```

## What Neil sees

When files change, a prompt is queued:
```
[EVENT] source=mirror type=file_changes time=...
remote: gdrive
changed: 3 files

--- diff ---
(git diff output showing exactly what changed)
```

Neil can then:
- Analyze the changes
- Store key facts as MEMORY: lines
- Alert the operator via NOTIFY: if something important changed
- Update its own knowledge based on document changes

## Supported cloud providers

rclone supports 40+ providers including:
- Google Drive
- Dropbox
- OneDrive
- Amazon S3
- SFTP
- WebDAV
- Any S3-compatible storage

Run `rclone config` to set up any of them.
