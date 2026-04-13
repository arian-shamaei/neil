# Vision

Neil's visual perception. Capture screenshots, terminal panes, camera
frames, or read images dropped in the inbox.

## Account

- **identity**: local (no auth needed)
- **scope**: read-only visual capture

## Actions

### look

Auto-detect and capture whatever is available.

```
CALL: service=vision action=look
```

### screenshot

Capture the desktop (requires X11/Wayland display).

```
CALL: service=vision action=screenshot
```

### pane

Capture a tmux pane's text content.

```
CALL: service=vision action=pane target=<session:window.pane>
```

### camera

Capture from a camera (webcam or IP camera).

```
CALL: service=vision action=camera url=<optional-ip-url>
```

### inbox

Check for user-dropped images in vision/inbox/.

```
CALL: service=vision action=inbox
```

### list

List available capture methods on this system.

```
CALL: service=vision action=list
```

## Notes

- Image captures are saved to vision/captures/ with timestamps.
- Text captures (tmux panes) are .txt files, images are .png/.jpg.
- Old captures are auto-pruned (keeps last 50).
- Users can drop any image in vision/inbox/ for Neil to see.
