# Dumbcord

A self-hosted, native Rust chat application for a small group of friends. The implementation lives in [`dumbcord/`](dumbcord/README.md): a SQLite-backed TCP server, a terminal client, and an `egui` desktop client for Windows and Linux.

> **Security:** The current wire protocol is unencrypted TCP. Use a private LAN or WireGuard/Tailscale tunnel. Do not expose the listener directly to the public internet.

## Quick start

Install Rust, then see [the application README](dumbcord/README.md) for build and run instructions.
