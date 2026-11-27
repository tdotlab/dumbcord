# Design: a plain tool

Porch looks like what it grew out of: the egui DumbCord window. One flat charcoal
surface, 1px separators, black wells for typing, grey square buttons, Ubuntu type with
big light headings. It should look like a utility a friend wrote, not a product, and
it should not look like Discord or like every other generated chat app.

What that rules out: avatars, rounded cards, pill buttons, shadows, gradients, glows,
blurple, icon fonts, mascot illustrations. Text labels on buttons instead of icons.

## Colour

All tokens live in `app/src/app.css`.

| Token | Hex | Use |
| --- | --- | --- |
| `--bg` | `#1b1b1b` | Everything |
| `--well` | `#0a0a0a` | Inputs, the composer, code |
| `--line` | `#3c3c3c` | Separators, field borders |
| `--button` | `#3c3c3c` | Buttons (hover `#464646` with a `#969696` outline, pressed `#373737`) |
| `--text` / `--strong` / `--weak` | `#a8a8a8` / `#e6e6e6` / `#858585` | Body, names and headings, metadata. `--weak` still meets 4.5:1. |
| `--select` | `#005c80` | The current conversation, pressed toggles (Leave voice, Unmute), focus |
| `--attention` | `#ff8f00` | Something wants you: a mention, the unread divider, a reconnect warning |
| `--live` | `#6cc070` | Voice only: connected, online, someone speaking |
| `--link` | `#5aaaff` | Links and other people's @mentions |
| `--error` | `#ef6a5e` | Failed sends, sign-in errors |

One colour, one meaning. Orange is never decoration and green never means anything but
live audio or presence.

## Type

Ubuntu at 300, 400 and 500. Headings are big, light and upper case ("CHANNELS",
"PEOPLE") the way egui draws them; that is the one typographic flourish, so nothing
else goes upper case. Names are 500, times are small and weak. Body 14px.

## Layout

A top bar (wordmark, a rule, the status line, Sign out), a 232px sidebar and the chat.
Messages are a name and time on one line with the text below; follow-ups from the same
person in a short span drop the header. A mention of you gets a 2px orange bar on the
left. Edit and Delete appear as small buttons on hover. Under 700px the sidebar becomes
a drawer behind a Menu button.

## Voice

People in a voice room are listed under its channel with a small square: outlined while
connected, filled green while talking, with "muted" after the name. The panel at the
foot of the sidebar says "Voice connected #room" with Mute and Leave.

## Copy

Plain and literal. "Join voice", "Leave voice", "Didn't send." with "Try again",
"Connected to host as name". Errors say what happened and what to do.

## Motion

Only the drawer slides. Respect `prefers-reduced-motion`.
