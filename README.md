# Porch

A private place for a small group of friends to talk: channels, direct
messages, presence, and voice rooms. One small Rust server, one Svelte
client that runs as a Windows app, an Android app, or a web page.

It is built for a dozen to a few dozen people who know each other, on a
server one of them runs. It is not trying to be Discord.

## What's here

| Path | What it is |
| --- | --- |
| `server/` | `porch-server`: Rust, tokio + axum WebSocket at `/ws`, SQLite storage |
| `app/` | The client: Svelte 5 + TypeScript + Vite |
| `app/src-tauri/` | Tauri 2 shell that turns the client into a desktop/Android app |
| `docs/` | Architecture and design notes |
| `scripts/voice_e2e.py` | Two-browser voice test against real Cloudflare Realtime (see its docstring) |

## Running it locally

You need Rust (stable) and Node 22+.

```sh
# Terminal 1: the server, with an invite code so strangers can't sign up
cargo run -p porch-server -- --invite letmein

# Terminal 2: the client in a browser, with hot reload
cd app
npm install
npm run dev        # http://localhost:1420, proxies /ws to the server
```

Create an account with the invite code. Open a second browser profile (or
`http://127.0.0.1:1420`, which keeps its own storage) to talk to yourself.

Run the desktop app instead of the browser with `npm run tauri dev`.

## Server

```
porch-server [--db porch.sqlite3] [--bind 127.0.0.1:7878] [--invite CODE] [--web-dir app/dist]
             [--cf-app-id ID --cf-app-token TOKEN]
porch-server passwd <user>    # new password from stdin; signs them out everywhere
porch-server users            # list accounts
```

Each flag also reads an environment variable: `PORCH_DB`, `PORCH_BIND`,
`PORCH_INVITE`, `PORCH_WEB_DIR`, `PORCH_CF_APP_ID`, `PORCH_CF_APP_TOKEN`.
Logging follows `RUST_LOG`.

Voice runs through a Cloudflare Realtime SFU app (Cloudflare dashboard,
Realtime, SFU). Give the server its app ID and token, preferably through the
environment so the token stays out of shell history; without both, voice is
off and the client hides it. Browsers only hand over a microphone on `https://`
or `localhost`, so voice over the internet needs the TLS proxy below.
Cloudflare bills SFU egress past the free monthly allowance, so set a usage
alert on the account.

The server speaks plain `ws://`. On the internet, put a TLS proxy such as
Caddy in front of it so clients connect over `wss://`. `--web-dir` serves the
built client from the same origin, so friends without the app can use a
browser. `GET /healthz` answers `ok`.

It idles at a few megabytes of memory. Password hashing (Argon2id, about
19 MiB each, two at a time) is the only real spike, which is why a 1 GB VPS
is plenty.

## Building the apps

```sh
cd app
npm run tauri build    # Windows installer in src-tauri/target/release/bundle
```

Android needs the Android SDK and NDK; see `docs/ARCHITECTURE.md`.

## Checks

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd app && npm run check && npm run build
```

`cargo test` also regenerates the client's protocol types in
`app/src/lib/protocol/` from the Rust definitions. Commit them when they change.
