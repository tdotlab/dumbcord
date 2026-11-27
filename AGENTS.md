# Porch repository guide

- Server: `server/` (Rust). Client: `app/` (Svelte 5, plain JavaScript, Vite). Desktop and
  Android shell: `app/src-tauri/` (Tauri 2, its own Cargo lockfile, excluded from the
  root workspace).
- Read `README.md` for setup and usage, `docs/ARCHITECTURE.md` for how the pieces fit,
  and `docs/DESIGN.md` before changing anything visual.
- The wire protocol lives in `server/src/protocol.rs`; it is the source of truth. When
  a frame changes, update the client's handling in `porch.svelte.js` / `voice.svelte.js`.
- The client is plain JavaScript on purpose. Don't add TypeScript back.
- Client state lives in one place, `app/src/lib/porch.svelte.js`. Components read it and
  call its methods; they do not talk to the socket.
- Keep the server small enough for a 1 GB VPS: no new runtime services, SQLite only,
  bounded queues and rate limits on anything a client can trigger.

Validate before committing:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cd app && npm run build
```

The server speaks plain WebSocket. Never describe it as safe to expose without a TLS
proxy in front.
