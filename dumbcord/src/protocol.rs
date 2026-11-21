use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const MAX_TEXT_CHARS: usize = 2_000;
pub const MAX_FRAME_BYTES: usize = 16 * 1024;
pub const HISTORY_PAGE_SIZE: i64 = 100;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub id: i64,
    pub channel: Option<String>,
    pub author: String,
    pub recipient: Option<String>,
    pub text: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Register {
        username: String,
        password: String,
        invite: Option<String>,
    },
    Login {
        username: String,
        password: String,
    },
    CreateChannel { name: String },
    Channels,
    Users,
    Join { channel: String },
    Send { channel: String, text: String },
    DirectMessage { to: String, text: String },
    History { channel: String, before: Option<i64> },
    DirectHistory { peer: String, before: Option<i64> },
    Typing { channel: String },
    Ping,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Authenticated { username: String },
    Error { message: String },
    Channels { channels: Vec<String> },
    Users { members: Vec<String>, online: Vec<String> },
    History { channel: String, messages: Vec<ChatMessage> },
    DirectHistory { peer: String, messages: Vec<ChatMessage> },
    Message { message: ChatMessage },
    DirectMessage { message: ChatMessage },
    Typing { channel: String, username: String },
    Pong,
    /// Emitted locally by a client when its TCP connection terminates.
    Disconnected { message: String },
}

pub fn username(raw: &str) -> Result<String> {
    let result = raw.trim().to_ascii_lowercase();
    if !(2..=24).contains(&result.len())
        || !result
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
    {
        bail!("username must be 2-24 ASCII letters, digits, '_' or '-'");
    }
    Ok(result)
}

pub fn channel(raw: &str) -> Result<String> {
    let result = raw.trim().trim_start_matches('#').to_ascii_lowercase();
    if !(1..=32).contains(&result.len())
        || !result.bytes().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-'
        })
    {
        bail!("channel must be 1-32 ASCII letters, digits, '_' or '-'");
    }
    Ok(result)
}

pub fn message_text(raw: &str) -> Result<String> {
    let result = raw.trim().to_owned();
    if result.is_empty() || result.chars().count() > MAX_TEXT_CHARS {
        bail!("message must contain 1-{MAX_TEXT_CHARS} characters");
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_names_and_rejects_bad_input() {
        assert_eq!(username("  ALICE-1  ").unwrap(), "alice-1");
        assert_eq!(channel(" #General ").unwrap(), "general");
        assert!(username("a").is_err());
        assert!(username("a b").is_err());
        assert!(channel("../etc").is_err());
        assert!(channel("").is_err());
        assert!(message_text("  ").is_err());
        assert!(message_text(&"a".repeat(2_001)).is_err());
    }

    #[test]
    fn protocol_round_trip() {
        let request = ClientMessage::Send {
            channel: "general".to_owned(),
            text: "hello".to_owned(),
        };
        let encoded = serde_json::to_string(&request).unwrap();
        assert!(encoded.contains("\"type\":\"send\""));
        let decoded: ClientMessage = serde_json::from_str(&encoded).unwrap();
        assert!(matches!(decoded, ClientMessage::Send { .. }));
    }
}
