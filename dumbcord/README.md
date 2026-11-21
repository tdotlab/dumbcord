# Dumbcord

**A small, native Rust Discord-like chat app for Windows and Linux.** One binary with three modes: a self-hosted server, an interactive terminal client, and a native desktop client (egui). No browser, JavaScript, Node, or Electron.

This is an MVP for a friend group, not a Discord replacement.

## Features

- Accounts with salted Argon2 password hashes
- Optional invitation code for registration
- Public text channels; any member can create channels
- Private direct messages, including offline history
- Live chat, online presence, and desktop typing indicators
- SQLite-backed history with 100-message pages
- Native desktop UI with unread markers and earlier-message loading
- CLI and desktop speaking one bounded JSON-lines TCP protocol

## Requirements

Install stable Rust via https://rustup.rs .

Windows: Rust MSVC toolchain and Visual Studio C++ Build Tools.

Linux: install the eframe/egui development libraries. For Ubuntu/Debian:

~~~sh
sudo apt update
sudo apt install build-essential pkg-config libx11-dev libxcursor-dev libxi-dev libxrandr-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev
~~~

## Quick start

Run these commands from the dumbcord/ folder.

Local server:

~~~sh
cargo run --release -- server --bind 127.0.0.1:7777 --db dumbcord.sqlite3
~~~

Server reachable by friends over a private LAN or VPN:

~~~sh
cargo run --release -- server --bind 0.0.0.0:7777 --db dumbcord.sqlite3
~~~

Set an invitation code before starting the server to restrict registration. Bash:

~~~sh
export DUMBCORD_INVITE="your-private-invite"
cargo run --release -- server --bind 0.0.0.0:7777
~~~

PowerShell:

~~~powershell
$env:DUMBCORD_INVITE = "your-private-invite"
cargo run --release -- server --bind 0.0.0.0:7777
~~~

Desktop client:

~~~sh
cargo run --release -- desktop --server 127.0.0.1:7777
~~~

Terminal client:

~~~sh
cargo run --release -- cli --server 127.0.0.1:7777
~~~

Replace the address with the host's private LAN or VPN IP for other devices. Allow port 7777 through the host firewall only on the trusted interface/network.

After building, run the executable directly from target/release/dumbcord (Linux) or target\release\dumbcord.exe (Windows).

The CLI prompts for sign-in or registration. Commands: /channels, /create NAME, /join NAME, /users, /dm NAME, /back, /history, /help, /quit.

## Security boundaries

**The wire protocol is unencrypted TCP.** Passwords and messages travel in plaintext unless the connection is protected by an encrypted tunnel. **Never expose port 7777 directly to the public internet.** Use WireGuard/Tailscale or a strictly trusted LAN. Untrusted LANs are not safe.

The person running the server can read its SQLite history. There is no end-to-end encryption, moderation, permissions, or TLS built into the app. Invitation codes control registration, not channel access. Anyone already registered can access every channel.

Clients require sign-in after restart. The desktop does not auto-reconnect. This is a small-group text chat MVP.

## Architecture and tests

See docs/ARCHITECTURE.md for the module map and protocol behavior.

~~~sh
cargo check --all-targets
cargo test
cargo fmt --check
~~~

GitHub Actions validates Windows and Linux. The first Rust build will fetch and compile the GUI dependencies.

## Not implemented

Voice/video, attachments, reactions, message editing/deletion, push notifications, TLS, password reset, and server moderation are out of scope.
