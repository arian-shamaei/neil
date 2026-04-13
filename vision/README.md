# Vision

Neil's visual perception system. Any image dropped in inbox/ is seen by
Neil on the next cycle. Automated captures can also be triggered.

## How it works

```
[any source]
  user drops screenshot ──→ vision/inbox/
  capture.sh runs ────────→ vision/captures/
  camera snaps ───────────→ vision/inbox/
  remote screen share ───→ vision/inbox/
                                │
                        observe.sh detects new images
                                │
                        prompt queued with image path
                                │
                        Claude reads the image (multimodal)
                                │
                        Neil describes what it sees
```

## Directory layout

```
~/.neil/vision/
  README.md           this file
  inbox/              drop any image here (jpg, png, bmp, gif)
  captures/           automated captures (screenshots, pane dumps)
  capture.sh          adaptive capture script
```

## Dropping an image for Neil

Put any image file in inbox/:
```sh
cp screenshot.png ~/.neil/vision/inbox/
# or
import -window root ~/.neil/vision/inbox/desktop.png
# or paste from clipboard
xclip -selection clipboard -t image/png -o > ~/.neil/vision/inbox/clipboard.png
```

Neil will see it on the next heartbeat or immediately if autoPrompter
detects the file via the filesystem watcher.

## Automated capture

```sh
~/.neil/vision/capture.sh              # auto-detect and capture
~/.neil/vision/capture.sh screenshot   # desktop screenshot
~/.neil/vision/capture.sh pane [name]  # tmux pane text
~/.neil/vision/capture.sh camera       # camera snapshot
~/.neil/vision/capture.sh window <id>  # specific window
```

## Neil requesting vision

Neil can output:
```
LOOK: [target]
```

- `LOOK:` -- capture whatever is available
- `LOOK: screenshot` -- desktop screenshot
- `LOOK: pane main` -- tmux pane named "main"
- `LOOK: camera` -- camera snapshot
- `LOOK: inbox` -- check inbox for user-dropped images

The broker executes capture.sh, image lands in captures/, and the
ReAct loop feeds it back to Neil for analysis.

## Supported formats

Claude can read: PNG, JPG, GIF, BMP, WebP
Text captures (tmux panes) are saved as .txt files.

## Cleanup

Processed images are moved from inbox/ to captures/ with a timestamp.
captures/ is periodically pruned (keep last 50).
