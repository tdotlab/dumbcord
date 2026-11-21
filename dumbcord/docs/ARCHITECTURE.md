# Architecture

Dumbcord is one Rust executable with three subcommands.

- src/main.rs: clap entry point and mode selection.
- src/protocol.rs: shared JSON-line message enums, validation and limits.
- src/store.rs: SQLite, Argon2 credentials, channel and direct-message history.
- src/server.rs: threaded TCP listener, authenticated sessions, message delivery, presence.
- src/client.rs: socket read/write worker threads communicating via Rust mpsc channels.
- src/cli.rs: terminal chat, live printer thread, slash commands.
- src/desktop.rs: native eframe/egui UI, state cache, unread and typing indicators.

Transport: newline-delimited UTF-8 JSON on one TCP connection. A client authenticates first, then sends typed requests; the server emits typed events. Client requests are limited to 16 KiB and incoming server messages to 512 KiB. Chat text is limited to 2,000 Unicode scalar values.

Channels are public to authenticated users. Channel messages are broadcast to all signed-in sessions. DMs are persisted in a separate table and delivered only to the sender/recipient sessions; DM history is scoped to the authenticated account. Multiple sessions per account are supported.

The server uses one rusqlite connection behind a mutex, with WAL when available. This intentionally targets small groups. Passwords use Argon2 and random salts, but the transport has no application TLS. A trusted encrypted tunnel is required on untrusted networks.

The desktop never performs blocking socket I/O on the GUI thread. A connection worker handles the initial dial, then socket worker threads own the network, and the UI consumes channel events each frame. The terminal uses the same client implementation.

Future public deployment would require transport encryption, authorization, abuse controls, migrations, and operational monitoring.
