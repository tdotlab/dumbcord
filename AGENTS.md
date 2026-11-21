# Dumbcord repository guide

- Application source: `dumbcord/`. Use Rust, not JavaScript, HTML, or Electron.
- Read `dumbcord/README.md` for setup, CLI usage, security limits, and project scope.
- Shared wire types: `dumbcord/src/protocol.rs`; persistence: `dumbcord/src/store.rs`.
- Networking is synchronous, line-delimited JSON over TCP; GUI networking must stay off the UI thread.
- Validate with `cargo check --all-targets`, `cargo test`, and Windows/Linux CI before shipping.
- Never claim the raw TCP transport is safe on the public internet; use a private LAN or VPN.
