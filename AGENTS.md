# Porch repository guide

- Server: `server/` (Rust). Client: `app/` (Svelte 5 + TypeScript, Vite). Desktop and
  Android shell: `app/src-tauri/` (Tauri 2, its own Cargo lockfile, excluded from the
  root workspace).
- Read `README.md` for setup and usage, `docs/ARCHITECTURE.md` for how the pieces fit,
  and `docs/DESIGN.md` before changing anything visual.
- The wire protocol lives in `server/src/protocol.rs`. Never hand-edit
  `app/src/lib/protocol/*.ts`; `cargo test -p porch-server` regenerates them. Commit both.
- Client state lives in one place, `app/src/lib/porch.svelte.ts`. Components read it and
  call its methods; they do not talk to the socket.
- Keep the server small enough for a 1 GB VPS: no new runtime services, SQLite only,
  bounded queues and rate limits on anything a client can trigger.
- `dumbcord/` is the retired prototype. Don't build on it.

Validate before committing:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cd app && npm run check && npm run build
```

The server speaks plain WebSocket. Never describe it as safe to expose without a TLS
proxy in front.
