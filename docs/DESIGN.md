# Design: porch at dusk

Porch should feel like the porch of a house after sundown: dark boards, a painted blue
ceiling, one lamp on. Calm, warm, a little homemade. Not a gamer app, not a SaaS dashboard.

## Colour

| Token | Hex | Use |
| --- | --- | --- |
| `--night` | `#1e2433` | Page background |
| `--boards` | `#232a3c` | Sidebar, panels |
| `--raise` | `#2c3449` | Hover, inputs, toasts |
| `--sunk` | `#181d2a` | Wells, code |
| `--haint` | `#9fd8cb` | Haint blue: selection, focus, the send button |
| `--porchlight` | `#f2c46d` | Amber: presence, unread, mentions, who is talking. Nothing else. |

Amber is rationed. If something is amber, it is about a person: they are here, they
said something you haven't read, they said your name, or they are talking right now
(the glowing ring on a voice avatar, and the head count on a channel's voice button).
Don't spend it on buttons or decoration. Your own voice connection is a state, not a
person, so the voice bar at the foot of the sidebar is haint blue; Mute turns red only
while you are muted.

## The lantern

The one bold element. It hangs in the top of the sidebar and its glow tracks how many
people are online (`glowOf` in `PorchLight.svelte`): barely lit when you're alone,
bright with a crowd. The sign-in card and the loading splash use it too. Nothing else
gets an illustration.

## Type

Atkinson Hyperlegible Next for everything you read; Young Serif only for the wordmark
and conversation titles. Sentence case throughout, no all-caps labels.

## Copy

Plain and a little warm, matching the porch idea where it fits naturally: "On the
porch" for online, "Inside" for offline, "Just you out here" when alone. Voice is
"Join voice" and "Leave voice", "Voice connected" once you're in. Errors say what
happened and what to do. Don't force the metaphor into every string.

## Motion

Only in answer to something: drawers sliding, the toast rising, typing dots, the lantern
lighting at sign-in. Respect `prefers-reduced-motion`.
