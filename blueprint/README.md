# Blueprint

Neil's terminal user interface -- the human-facing console. Built in Rust
with ratatui. Modular panel system where each panel is a "cartridge" that
plugs into the console.

## Running

```sh
neil-blueprint
# or
~/.neil/blueprint/target/release/neil-blueprint
```

Requires a real terminal (SSH with -t, or direct console). Not compatible
with non-interactive sessions.

## Architecture

```
Console (main.rs)
  ├── Header panel     NEIL | date time | beats | queue
  ├── Grid (2x2)
  │   ├── Heartbeat    scrolling beat log with status colors
  │   ├── Memory       palace overview: wings, rooms, counts
  │   ├── Intentions   pending tasks with priority/due/tag
  │   └── System       essence, services, autoprompt, queue status
  └── Status bar       last beat | intents | failures | notes
```

## Panel Trait

Every panel implements:
```rust
pub trait Panel {
    fn id(&self) -> &str;         // unique identifier
    fn title(&self) -> &str;      // border title
    fn render(&self, area, buf, state);  // draw into terminal
    fn update(&mut self, state);  // tick update (optional)
    fn priority(&self) -> u8;     // narrow mode priority (0-3)
}
```

## Adding a new panel

1. Create `src/panels/my_panel.rs` implementing `Panel`
2. Add `pub mod my_panel;` to `src/panels/mod.rs`
3. Instantiate in `main.rs` and add to the grid layout
4. `cargo build --release`

## Data source

All panels read from `NeilState` (src/state.rs), which loads Neil's files:
- heartbeat_log.json
- memory/palace/index/rooms.idx
- memory/palace/notes/*.md
- intentions.json
- self/failures.json
- essence/*.md
- services/registry/*.md

State is reloaded from disk every tick (500ms).

## Building

```sh
cd ~/.neil/blueprint
cargo build --release
cp target/release/neil-blueprint ~/.local/bin/
```

## Keys

- `q` -- quit
- `r` -- force refresh
