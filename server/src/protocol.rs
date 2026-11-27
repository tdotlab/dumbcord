//! Wire protocol: JSON text frames over one WebSocket at `/ws`.
//!
//! Every frame is an object with a `type` tag. Conversations are addressed by
//! a string target: `#name` for a channel, `@user` for a direct message. A DM
//! target is always written from the reader's point of view, so Alice sees her
//! conversation with Bob as `@bob` and Bob sees the same one as `@alice`; the
//! server stores both under one internal key.
//!
//! `cargo test` regenerates the TypeScript copies of these types in
//! `app/src/lib/protocol/` (see `.cargo/config.toml`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_TEXT_CHARS: usize = 2000;
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const HISTORY_PAGE: usize = 50;

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum ClientMessage {
    /// Create an account. `invite` is required when the server has one set.
    Register {
        username: String,
        password: String,
        invite: Option<String>,
    },
    Login {
        username: String,
        password: String,
    },
    /// Sign back in with a token from an earlier `welcome`.
    Resume {
        token: String,
    },
    /// Revoke this connection's token and disconnect.
    Logout,
    CreateChannel {
        name: String,
    },
    /// Fetch up to one page of messages older than `before` (newest if null).
    History {
        to: String,
        #[ts(type = "number | null")]
        before: Option<i64>,
    },
    /// `nonce` is echoed back on the resulting `message` so the sender can
    /// match it to the optimistic copy it already drew.
    Send {
        to: String,
        text: String,
        nonce: Option<String>,
    },
    Edit {
        #[ts(type = "number")]
        id: i64,
        text: String,
    },
    Delete {
        #[ts(type = "number")]
        id: i64,
    },
    Typing {
        to: String,
    },
    /// Everything in `to` up to and including `id` has been seen.
    MarkRead {
        to: String,
        #[ts(type = "number")]
        id: i64,
    },
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum ServerMessage {
    /// Sent once after a successful register, login or resume.
    Welcome {
        protocol: u32,
        user: String,
        token: String,
        channels: Vec<String>,
        users: Vec<String>,
        online: Vec<String>,
        convs: Vec<ConvState>,
    },
    Error {
        code: ErrorCode,
        message: String,
    },
    History {
        to: String,
        messages: Vec<ChatMessage>,
        has_more: bool,
    },
    Message {
        message: ChatMessage,
        nonce: Option<String>,
    },
    Edited {
        message: ChatMessage,
    },
    Deleted {
        #[ts(type = "number")]
        id: i64,
        to: String,
        author: String,
    },
    ChannelCreated {
        name: String,
    },
    UserJoined {
        user: String,
    },
    Presence {
        user: String,
        online: bool,
    },
    Typing {
        user: String,
        to: String,
    },
    /// Read position changed on another of this user's devices.
    Read {
        to: String,
        #[ts(type = "number")]
        id: i64,
    },
    Pong,
}

/// One message as stored. For a DM, `to` is `@recipient` as the author wrote
/// it; a client finds the conversation with `author == me ? to : "@" + author`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChatMessage {
    #[ts(type = "number")]
    pub id: i64,
    pub to: String,
    pub author: String,
    pub text: String,
    /// Unix milliseconds.
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number | null")]
    pub edited_at: Option<i64>,
}

/// Where a user stands in one conversation, for unread badges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConvState {
    pub to: String,
    #[ts(type = "number")]
    pub last_id: i64,
    #[ts(type = "number")]
    pub last_read: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ErrorCode {
    /// The request was malformed or failed validation.
    BadRequest,
    /// Wrong username or password, or a bad invite.
    AuthFailed,
    /// The resume token is unknown or expired; sign in again.
    TokenInvalid,
    /// The request needs a signed-in connection.
    NotSignedIn,
    Taken,
    NotFound,
    Forbidden,
    RateLimited,
    Internal,
}
