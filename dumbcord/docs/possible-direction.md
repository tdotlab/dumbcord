# Possible direction for Dumbcord

**Status: exploratory.** This is a proposal for evolving the existing experiment into a practical, private communications app for a small friend group. It is not an approved roadmap or a change to the current project scope.

## Recommendation

Evolve Dumbcord rather than start over. The existing native Rust application already has a server, SQLite history, Argon2 authentication, optional registration invites, channels, DMs, presence, typing indicators, a desktop UI, a terminal client, a shared protocol, and tests. The architecture is a reasonable foundation for a small group.

Keep the scope deliberately narrow: reliable persistent text chat first, occasional voice chat second. Avoid JavaScript, TypeScript, Electron, and a large distributed backend. A browser client, mobile apps, bots, multiple communities, video, and elaborate permissions are not initial requirements.

## Architecture

| Area | Direction |
| --- | --- |
| Language | Keep Rust throughout the application. |
| Desktop | Keep egui/eframe for now; evaluate GPUI or Iced in a small prototype only if egui becomes a limitation for chat UX or accessibility. |
| Storage | Keep SQLite and rusqlite. Add versioned schema migrations, backups, and restoration tests. |
| Protocol | Preserve the typed Serde JSON message model. Add versioning and evolve it incrementally. |
| Networking | Keep the present approach if it meets requirements; consider Tokio and rustls if async transport and TLS simplify reliability. |
| Deployment | Separate the headless server from GUI dependencies (e.g. feature flags or separate workspace binaries). Retain the terminal client as a debugging tool. |
| Voice | Evaluate the LiveKit Rust SDK for WebRTC audio, with a managed or self-hosted LiveKit server. Keep audio separate from the text transport. |

No need for PostgreSQL, Supabase, Kubernetes, or microservices at this scale. A small Linux host running one chat server with SQLite is sufficient in principle. Keep deployment and voice providers replaceable.

## Before inviting users

- **Transport security:** the current JSON-lines TCP protocol is unencrypted, including passwords. Never expose it directly to the public internet. Use an encrypted private network for an early trial; for direct public connections, add authenticated TLS with certificate validation.
- **Connection reliability:** add bounded per-client outbound queues and write deadlines so a slow receiver cannot stall a broadcast. Bound incoming work and concurrent connections; add sensible authentication rate limits. The existing synchronous architecture is not inherently a problem for a small group.
- **Reconnect and synchronization:** automatic reconnect, secure session handling, catch-up by message ID, deduplication and history/live-event ordering. Store credentials/tokens safely if implementing persistent login.
- **Persistence:** add versioned migrations, automated SQLite-aware backups and restoration testing.
- **Access control:** require invitations for a private deployment, add basic account revocation and channel administration. The server operator can read stored messages; do not imply end-to-end encryption.
- **Testing and packaging:** verify the existing build, tests and Windows/Linux CI, then add coverage for disconnects, slow clients, authentication, migrations, permissions and releases. Commit the application lockfile for reproducible builds if appropriate.

## Suggested implementation sequence (if approved)

1. **Baseline:** build and test the current application, run several clients against one server, inspect the existing UI, and resolve any regressions. Confirm target operating systems and desired features with intended users.
2. **Reliable text MVP:** secure transport, reconnect/catch-up, persistent login, notifications, basic administration, and straightforward installation. Test an actual private deployment.
3. **Make text chat enjoyable:** replies, reactions, message edits/deletes, search, image and GIF sharing, Markdown, avatars and improved conversation layout. Prioritize what users actually want. Use size limits, safe image decoding, controlled attachment access, and a storage/retention policy. Keep attachment blobs outside SQLite unless a measured reason favors storing them there.
4. **Voice proof of concept:** connect two native Rust desktop clients to a LiveKit room using the existing account identity and a server-issued, short-lived room token. Verify audio capture and playback, device selection, mute, push-to-talk, noise suppression/echo cancellation, latency and reconnects on target operating systems. Confirm SDK support and integration costs before committing to the provider. Expand to drop-in voice channels only after the two-person experience works.
5. **Polish:** tray behavior, notifications, installer/update flow, theme and accessibility improvements. Video and screen sharing remain optional future work.

Do not rewrite the GUI or networking solely to adopt a newer framework. Make each change earn its complexity.

## Hosting and rough costs

For a small community, start with a headless Rust server, SQLite, and a persistent disk. A modest VPS (roughly 1–2 vCPUs, 1–2 GB RAM) is a reasonable starting estimate for text chat, not a benchmark. A machine already running at home over an encrypted private network is another option.

- Existing home server: potentially no additional hosting bill beyond electricity, network access and backups.
- Small VPS: roughly US$6–12/month for predictable always-on hosting; verify provider pricing, bandwidth, storage and backup terms.
- Managed voice: potentially free at very low usage, but verify actual provider quotas, participant-minute pricing, egress and relay costs. Do not assume Cloudflare's free SFU bandwidth is compatible with the LiveKit SDK or that managed LiveKit pricing uses the same model.

At 64 kbps per continuously transmitted audio stream, two people talking four hours per day for 30 days generate about 7 GB of SFU outbound audio payload, before protocol overhead. Typical intermittent use may be lower with silence suppression. Text is negligible by comparison; attachments, video and screen sharing can dominate storage and bandwidth.

Measure real CPU, RAM, disk usage and egress after deployment instead of optimizing for hypothetical scale.

## Review limitations

This proposal is based on inspection of the current repository source and documentation, not an executed build, benchmark, penetration test, or voice prototype. Existing code should be validated before treating it as production-ready. No implementation changes are authorized by this document.

## Starting point for an implementation agent

Read the repository AGENTS.md, README and ARCHITECTURE.md. Establish the baseline and report any failing tests or architectural constraints before making substantial changes. Propose a small first milestone for a secure, reliable text-only trial, and keep the existing working features intact. Leave later milestones optional until the direction is approved.
