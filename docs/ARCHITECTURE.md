# Architecture

Porch is one Rust process, one SQLite file, and a client that talks to it over a single
WebSocket. Voice rides Cloudflare Realtime (an SFU); the Porch server brokers the
signalling and media never touches the VPS.

## Server (`server/src`)

| File | Job |
| --- | --- |
| `main.rs` | CLI: `serve` (default), `passwd`, `users` |
| `server.rs` | axum router (`/ws`, `/healthz`, optional static client), per-connection loop, request handling |
| `hub.rs` | Who is connected, presence, fan-out, voice seats |
| `voice.rs` | Cloudflare Realtime client: new session, publish, pull, renegotiate |
| `store.rs` | SQLite: schema migrations (`PRAGMA user_version`), users, sessions, channels, messages, read markers |
| `auth.rs` | Argon2id password hashing with a concurrency cap; session tokens (stored as SHA-256) |
| `protocol.rs` | Every frame in both directions; the source of truth for the client too |
| `validate.rs` | Names, targets, text limits |
| `error.rs` | Error codes sent to clients |

A connection signs in with `Register`, `Login`, or `Resume` (a stored token), then
receives a `Welcome` snapshot: channels, users, who is online, and per-conversation
read state. After that it asks for `History` pages (50 at a time, newest first, with
`has_more`) and sends `Send`, `Edit`, `Delete`, `Typing`, `MarkRead`, `CreateChannel`.

Conversations are named by target: `#general` for a channel, `@name` for a DM. DMs are
stored under a canonical key, `@alice:bob`, so both sides see the same history.

Fan-out never blocks. Each connection has a bounded outbound queue; when it fills, that
connection is closed and the client reconnects and catches up from history. Clients are
rate-limited per action, and frames over 64 KiB are refused.

## Client (`app/src`)

| File | Job |
| --- | --- |
| `lib/conn.js` | The socket: server URL resolution, ping every 25 s, treat 60 s of silence as dead, reconnect with backoff |
| `lib/porch.svelte.js` | All client state as one Svelte 5 runes class: sign-in, conversations, drafts, unread, typing, optimistic sends |
| `lib/voice.svelte.js` | Voice: microphone, the one `RTCPeerConnection`, who is in which room, mute, speaking detection |
| `lib/markdown.js` | marked + DOMPurify; mentions and links |
| `lib/platform.js` | Tauri vs browser: notifications, opening links, visibility |
| `lib/components/` | UI, reading `porch` and calling its methods |
| `App.svelte` | Layout: top bar, sidebar, chat; the sidebar is a drawer under 700 px |

Sends are optimistic: a message appears faded with a client nonce, and the server's echo
replaces it. If the socket is down the message is marked "Didn't send." with Try again and Discard
rather than queued silently.

In a browser the client connects to `/ws` on its own origin (Vite proxies it in dev,
the server serves both in production). The Tauri app asks for the server address at
sign-in and remembers it.

## Voice

Every channel has a voice room. A user holds at most one seat (`Seat` in `hub.rs`:
connection, room, Realtime session, live, muted); joining from a second device moves
the seat there and tells the first device `VoiceEnded`. Dropping the socket stands the
seat, and `Welcome` carries the roster so late arrivals see who is sitting where.

The Realtime app token stays on the server. The client sends four requests over the
WebSocket, each with a `rid` it matches against `VoiceReply`, and the server checks
each against the seats before calling Cloudflare:

- `VoiceJoin`: new Realtime session, take the seat, broadcast `VoiceState`.
- `VoicePublish`: the client's offer for its sendonly mic transceiver; returns the answer
  and marks the seat live.
- `VoicePull`: subscribe to other live seats in the same room; returns an SFU offer and
  which `mid` belongs to which user.
- `VoiceAnswer`: the client's answer to that offer (Realtime's renegotiate).

`VoiceMute` and `VoiceLeave` are plain broadcasts. Mute is enforced locally by disabling
the mic track; the flag is only for display.

The client keeps one `RTCPeerConnection` per session (Cloudflare STUN, max-bundle) and
runs negotiation steps one at a time on a promise chain; a generation counter discards
steps from a session that has since been torn down. Cloudflare can't hand out a track
until the publisher's media has reached it, so pulls that come back short retry after
0.5, 1, 2 and 4 s. If the socket drops the call keeps playing, and on the next `Welcome`
the client rejoins; if ICE fails it tears down and rejoins. Speaking rings come from an
`AnalyserNode` per stream (RMS over a threshold, held for 350 ms).

Known gap: pulled transceivers from people who left stay on the connection until the
next rejoin. Fine for a dozen friends; a long session with lots of churn would want
Realtime's tracks/close.

## Desktop and Android (`app/src-tauri`)

A thin Tauri 2 shell with the opener and notification plugins and no custom commands.
It is excluded from the root Cargo workspace so the server builds without pulling in
WebView dependencies.

Android builds need the Android SDK, NDK, and a JDK; then `npx tauri android init`
once and `npx tauri android build --apk`. Not set up yet.

## Deployment sketch

One small VPS: `porch-server --web-dir /srv/porch/dist` bound to localhost, with Caddy in
front for TLS (`reverse_proxy 127.0.0.1:7878`; Caddy upgrades WebSockets on its own).
Back up by copying the SQLite file, or `sqlite3 porch.sqlite3 ".backup ..."` while live.
