//! Wire protocol: JSON text frames over one WebSocket at `/ws`.
//!
//! Every frame is an object with a `type` tag. Conversations are addressed by
//! a string target: `#name` for a channel, `@user` for a direct message. A DM
//! target is always written from the reader's point of view, so Alice sees her
//! conversation with Bob as `@bob` and Bob sees the same one as `@alice`; the
//! server stores both under one internal key.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_TEXT_CHARS: usize = 2000;
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const HISTORY_PAGE: usize = 50;
pub const MAX_SDP_BYTES: usize = 48 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
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
        id: i64,
        text: String,
    },
    Delete {
        id: i64,
    },
    Typing {
        to: String,
    },
    /// Everything in `to` up to and including `id` has been seen.
    MarkRead {
        to: String,
        id: i64,
    },
    Ping,
    /// Take a seat in a channel's voice room, leaving any other room. This
    /// connection's Realtime session is created here.
    VoiceJoin {
        rid: u32,
        room: String,
    },
    /// Publish the microphone: an SDP offer and the transceiver's `mid`.
    /// The reply carries the SDP answer.
    VoicePublish {
        rid: u32,
        sdp: String,
        mid: String,
    },
    /// Subscribe to these room members' microphones. The reply carries an SDP
    /// offer to answer with `voice_answer`, and which `mid` is whose.
    VoicePull {
        rid: u32,
        users: Vec<String>,
    },
    VoiceAnswer {
        rid: u32,
        sdp: String,
    },
    VoiceMute {
        muted: bool,
    },
    VoiceLeave,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
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
        /// Whether this server has voice set up at all.
        voice_enabled: bool,
        voice: Vec<VoiceMember>,
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
        id: i64,
    },
    Pong,
    /// The answer to one `voice_*` request with a `rid`. `error` is set when it failed.
    VoiceReply {
        rid: u32,
        sdp: Option<String>,
        tracks: Vec<VoiceTrack>,
        error: Option<String>,
    },
    /// Someone joined a voice room, started talking (`live`), or (un)muted.
    VoiceState {
        member: VoiceMember,
    },
    VoiceLeft {
        user: String,
    },
    /// This connection lost its voice seat, e.g. to the same user joining
    /// from another device.
    VoiceEnded {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoiceMember {
    pub user: String,
    /// The channel, written `#name`.
    pub room: String,
    /// Their microphone is published and can be pulled.
    pub live: bool,
    pub muted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoiceTrack {
    pub user: String,
    pub mid: String,
}

/// One message as stored. For a DM, `to` is `@recipient` as the author wrote
/// it; a client finds the conversation with `author == me ? to : "@" + author`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: i64,
    pub to: String,
    pub author: String,
    pub text: String,
    /// Unix milliseconds.
    pub created_at: i64,
    pub edited_at: Option<i64>,
}

/// Where a user stands in one conversation, for unread badges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConvState {
    pub to: String,
    pub last_id: i64,
    pub last_read: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
