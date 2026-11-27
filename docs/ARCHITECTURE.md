# Architecture

Porch is one Rust process, one SQLite file, and a client that talks to it over a single
WebSocket. Voice (next milestone) will ride Cloudflare Realtime, with the Porch server
handing out session tokens; media never touches the VPS.

## Server (`server/src`)

| File | Job |
| --- | --- |
| `main.rs` | CLI: `serve` (default), `passwd`, `users` |
| `server.rs` | axum router (`/ws`, `/healthz`, optional static client), per-connection loop, request handling |
| `hub.rs` | Who is connected, presence, fan-out |
| `store.rs` | SQLite: schema migrations (`PRAGMA user_version`), users, sessions, channels, messages, read markers |
| `auth.rs` | Argon2id password hashing with a concurrency cap; session tokens (stored as SHA-256) |
| `protocol.rs` | Every frame in both directions, with `ts-rs` exports for the client |
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
| `lib/conn.ts` | The socket: server URL resolution, ping every 25 s, treat 60 s of silence as dead, reconnect with backoff |
| `lib/porch.svelte.ts` | All client state as one Svelte 5 runes class: sign-in, conversations, drafts, unread, typing, optimistic sends |
| `lib/markdown.ts` | marked + DOMPurify; mentions and links |
| `lib/platform.ts` | Tauri vs browser: notifications, opening links, visibility |
| `lib/components/` | UI, reading `porch` and calling its methods |
| `App.svelte` | Layout: sidebar, chat, members; drawers under 760 px |

Sends are optimistic: a message appears faded with a client nonce, and the server's echo
replaces it. If the socket is down the message is marked failed with Retry and Discard
rather than queued silently.

In a browser the client connects to `/ws` on its own origin (Vite proxies it in dev,
the server serves both in production). The Tauri app asks for the server address at
sign-in and remembers it.

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
