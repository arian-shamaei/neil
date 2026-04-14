# Seal Art Engine

Parameterized seal renderer. Neil controls the seal's expression and
state by writing to ~/.neil/.seal_pose.json. The TUI reads this file
and renders the seal with the specified parameters.

## State file: ~/.neil/.seal_pose.json

```json
{
  "eyes": "open",
  "mouth": "smile",
  "whiskers": "normal",
  "body": "float",
  "breath_phase": 0.5,
  "water_phase": 0.0,
  "indicator": "none",
  "label": "feeling good :)"
}
```

### Parameters

**eyes**: open, half, closed, wide, focused, stressed, wink
**mouth**: smile, neutral, frown, open, relaxed, smirk, o
**whiskers**: normal, perked, droopy, spread
**body**: float, curl, stretch, dive, surface
**breath_phase**: 0.0-1.0 (exhale to inhale, auto-cycles if not set)
**water_phase**: 0.0-1.0 (wave offset, auto-cycles if not set)
**indicator**: none, zzz, alert, thought, bubbles, music, heart
**label**: text shown below the seal (max ~20 chars)

### Neil controls the seal

Neil can set any parameter:
```sh
echo '{"eyes":"focused","mouth":"neutral","indicator":"thought","label":"thinking..."}' > ~/.neil/.seal_pose.json
```

If the file doesn't exist or is invalid, defaults to happy floating seal.
The TUI auto-animates breath_phase and water_phase for liveliness.

## Art template files

Each .txt file in this directory is a static fallback frame.
The engine uses these as reference but renders dynamically.
Neil can still create .txt files for custom moods beyond the
parameterized system.

## Character reference

### Body (braille smooth)
⣀ ⣤ ⣶ ⣴ ⣦ ⣿ ⠿ ⠤ ⠏ ⠃ ⠹ ⠘ ⣠ ⣄ ⣷ ⣾

### Eyes
● open     ◉ focused    ◎ wide      ◑ half
─ closed   × stressed   ◐ wink-L    ◑ wink-R

### Mouth
◡ smile    ─ neutral    ∩ frown     ○ open
ω relaxed  ⌐ smirk     ◯ big-open

### Whiskers
═══ normal   ⟋⟍ perked   ─── droopy  ⟋⟍ spread

### Water
~ ≈ ∼ ～ ˜

### Indicators
z z z  sleep     ! ! !  alert     . o O  thought
♪ ♫    music     ♥      heart     ° ○    bubbles
