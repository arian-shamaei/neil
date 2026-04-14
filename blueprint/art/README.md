# Seal Art

File-based art system. Each .txt file is one art frame.
Neil can create new art files and they'll be picked up automatically.

## Format

Plain text, one frame per file. Lines are rendered top-to-bottom.
Use Unicode block/braille characters for smooth shapes.
Last line is the mood label (displayed below the art).

## Naming convention

Filename = mood state. The TUI picks art based on Neil's current state:
- happy.txt -- default, system healthy
- focused.txt -- has pending work
- working.txt -- processing a prompt
- stressed.txt -- unresolved failures
- sleeping.txt -- quiet hours
- curious.txt -- researching or exploring

## Creating new art

Just drop a .txt file in this directory. The TUI will find it.
Neil can create new art:
```
echo "new art here" > ~/.neil/blueprint/art/excited.txt
```

## Characters

Monochromatic. Use these for smooth seal shapes:
- Braille: ⣿ ⣤ ⣶ ⣴ ⣦ ⣀ ⠿ ⠤ ⠏ ⠃ ⠹ ⠘
- Block: █ ▓ ▒ ░
- Box: ─ │ ┌ ┐ └ ┘
- Eyes: ● ◉ ◎ ◑ ─ ×
- Mouth: ◡ ─ ∩ ○ ω ⌐
- Water: ~ ≈
- Misc: ▼ ♪ ♫ ★ ☆ ✦ ☁ ⚡
