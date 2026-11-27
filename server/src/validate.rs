//! Input normalization. Everything a client sends passes through here before
//! it reaches the database or another client.

use crate::error::{Error, Result};
use crate::protocol::MAX_TEXT_CHARS;

fn name_chars(s: &str) -> bool {
    s.bytes()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
}

pub fn username(raw: &str) -> Result<String> {
    let name = raw.trim().trim_start_matches('@').to_ascii_lowercase();
    if !(2..=24).contains(&name.len()) || !name_chars(&name) {
        return Err(Error::bad(
            "Usernames are 2 to 24 characters: letters, numbers, - and _.",
        ));
    }
    Ok(name)
}

pub fn channel(raw: &str) -> Result<String> {
    let name = raw.trim().trim_start_matches('#').to_ascii_lowercase();
    if !(1..=32).contains(&name.len()) || !name_chars(&name) {
        return Err(Error::bad(
            "Channel names are 1 to 32 characters: letters, numbers, - and _.",
        ));
    }
    Ok(name)
}

pub fn password(raw: &str) -> Result<()> {
    if !(8..=128).contains(&raw.chars().count()) {
        return Err(Error::bad("Passwords are 8 to 128 characters."));
    }
    Ok(())
}

/// Strip control characters (keeping newline and tab), normalize line
/// endings, trim, and enforce the length limit.
pub fn text(raw: &str) -> Result<String> {
    let cleaned: String = raw
        .chars()
        .filter(|&c| c == '\n' || c == '\t' || !c.is_control())
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return Err(Error::bad("Messages can't be empty."));
    }
    if cleaned.chars().count() > MAX_TEXT_CHARS {
        return Err(Error::bad(format!(
            "Messages are limited to {MAX_TEXT_CHARS} characters."
        )));
    }
    Ok(cleaned.to_owned())
}

/// A conversation as a client names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Channel(String),
    Dm(String),
}

impl Target {
    pub fn parse(raw: &str) -> Result<Self> {
        let raw = raw.trim();
        if let Some(name) = raw.strip_prefix('#') {
            Ok(Self::Channel(channel(name)?))
        } else if let Some(name) = raw.strip_prefix('@') {
            Ok(Self::Dm(username(name)?))
        } else {
            Err(Error::bad(
                "Targets start with # for a channel or @ for a person.",
            ))
        }
    }

    /// The storage key: `#name`, or `@a:b` with the two names sorted.
    pub fn key(&self, me: &str) -> String {
        match self {
            Self::Channel(name) => format!("#{name}"),
            Self::Dm(peer) => dm_key(me, peer),
        }
    }
}

pub fn dm_key(a: &str, b: &str) -> String {
    if a <= b {
        format!("@{a}:{b}")
    } else {
        format!("@{b}:{a}")
    }
}

/// The two members of a DM key, or None for a channel key.
pub fn dm_members(key: &str) -> Option<(&str, &str)> {
    key.strip_prefix('@')?.split_once(':')
}

/// Turn a storage key into the target `me` would use for it.
pub fn view_of(key: &str, me: &str) -> String {
    match dm_members(key) {
        Some((a, b)) => format!("@{}", if a == me { b } else { a }),
        None => key.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_normalized() {
        assert_eq!(username("  ALICE-1 ").unwrap(), "alice-1");
        assert_eq!(username("@bob").unwrap(), "bob");
        assert_eq!(channel(" #General ").unwrap(), "general");
        assert!(username("a").is_err());
        assert!(username("a b").is_err());
        assert!(username("al:ce").is_err());
        assert!(channel("../etc").is_err());
        assert!(channel("").is_err());
    }

    #[test]
    fn text_is_cleaned() {
        assert_eq!(
            text("hi\u{1b}[2J\u{1b}]0;x\u{7}\r\nthere\t!").unwrap(),
            "hi[2J]0;x\nthere\t!"
        );
        assert!(text(" \n\t ").is_err());
        assert!(text(&"a".repeat(MAX_TEXT_CHARS + 1)).is_err());
        assert!(text(&"漢".repeat(MAX_TEXT_CHARS)).is_ok());
    }

    #[test]
    fn targets_round_trip() {
        let t = Target::parse("@Bob").unwrap();
        assert_eq!(t, Target::Dm("bob".into()));
        assert_eq!(t.key("alice"), "@alice:bob");
        assert_eq!(Target::Dm("alice".into()).key("bob"), "@alice:bob");
        assert_eq!(view_of("@alice:bob", "alice"), "@bob");
        assert_eq!(view_of("@alice:bob", "bob"), "@alice");
        assert_eq!(view_of("#general", "bob"), "#general");
        assert!(Target::parse("general").is_err());
    }
}
