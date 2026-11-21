use std::{
    fs,
    path::Path,
    sync::{Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, bail, Result};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::rngs::OsRng;
use rusqlite::{params, Connection, OptionalExtension};

use crate::protocol::{self, ChatMessage, HISTORY_PAGE_SIZE};

pub struct Store {
    db: Mutex<Connection>,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

impl Store {
    pub fn open(path: &str) -> Result<Self> {
        if path != ":memory:" {
            if let Some(parent) = Path::new(path).parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)?;
                }
            }
        }

        let db = Connection::open(path)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "foreign_keys", "ON")?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS users (
                username TEXT PRIMARY KEY,
                password_hash TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS channels (
                name TEXT PRIMARY KEY,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS channel_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel TEXT NOT NULL REFERENCES channels(name),
                author TEXT NOT NULL REFERENCES users(username),
                text TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_messages_channel
                ON channel_messages(channel, id);
            CREATE TABLE IF NOT EXISTS direct_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                author TEXT NOT NULL REFERENCES users(username),
                recipient TEXT NOT NULL REFERENCES users(username),
                text TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_dm_pair
                ON direct_messages(author, recipient, id);",
        )?;
        db.execute(
            "INSERT OR IGNORE INTO channels (name, created_at) VALUES ('general', ?1)",
            [now()],
        )?;
        Ok(Self { db: Mutex::new(db) })
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>> {
        self.db.lock().map_err(|_| anyhow!("database lock poisoned"))
    }

    pub fn register(&self, raw_username: &str, password: &str) -> Result<String> {
        let name = protocol::username(raw_username)?;
        if !(8..=128).contains(&password.chars().count()) {
            bail!("password must be 8-128 characters");
        }

        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow!("password hashing failed: {e}"))?
            .to_string();

        let db = self.lock()?;
        if db
            .query_row(
                "SELECT 1 FROM users WHERE username = ?1",
                [&name],
                |_| Ok(()),
            )
            .optional()?
            .is_some()
        {
            bail!("username is taken");
        }
        db.execute(
            "INSERT INTO users (username, password_hash, created_at) VALUES (?1, ?2, ?3)",
            params![name, hash, now()],
        )?;
        Ok(name)
    }

    pub fn authenticate(&self, raw_username: &str, password: &str) -> Result<String> {
        let name = protocol::username(raw_username)?;
        let hash: Option<String> = self
            .lock()?
            .query_row(
                "SELECT password_hash FROM users WHERE username = ?1",
                [&name],
                |row| row.get(0),
            )
            .optional()?;

        let Some(hash) = hash else {
            bail!("incorrect username or password");
        };
        let parsed = PasswordHash::new(&hash)
            .map_err(|_| anyhow!("incorrect username or password"))?;

        if Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_err()
        {
            bail!("incorrect username or password");
        }
        Ok(name)
    }

    pub fn users(&self) -> Result<Vec<String>> {
        let db = self.lock()?;
        let mut stmt = db.prepare("SELECT username FROM users ORDER BY username")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<String>>>()?)
    }

    pub fn channels(&self) -> Result<Vec<String>> {
        let db = self.lock()?;
        let mut stmt = db.prepare("SELECT name FROM channels ORDER BY name")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<String>>>()?)
    }

    pub fn create_channel(&self, raw_name: &str) -> Result<String> {
        let name = protocol::channel(raw_name)?;
        let db = self.lock()?;
        if db
            .query_row(
                "SELECT 1 FROM channels WHERE name = ?1",
                [&name],
                |_| Ok(()),
            )
            .optional()?
            .is_some()
        {
            bail!("channel already exists");
        }
        db.execute(
            "INSERT INTO channels (name, created_at) VALUES (?1, ?2)",
            params![name, now()],
        )?;
        Ok(name)
    }

    pub fn has_channel(&self, name: &str) -> Result<bool> {
        let db = self.lock()?;
        Ok(db
            .query_row(
                "SELECT 1 FROM channels WHERE name = ?1",
                [name],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    pub fn record_channel_message(
        &self,
        channel: &str,
        author: &str,
        raw_text: &str,
    ) -> Result<ChatMessage> {
        let channel = protocol::channel(channel)?;
        let text = protocol::message_text(raw_text)?;
        let db = self.lock()?;
        if db
            .query_row(
                "SELECT 1 FROM channels WHERE name = ?1",
                [&channel],
                |_| Ok(()),
            )
            .optional()?
            .is_none()
        {
            bail!("channel not found");
        }
        let created_at = now();
        db.execute(
            "INSERT INTO channel_messages (channel, author, text, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![channel, author, text, created_at],
        )?;
        Ok(ChatMessage {
            id: db.last_insert_rowid(),
            channel: Some(channel),
            author: author.to_owned(),
            recipient: None,
            text,
            created_at,
        })
    }

    pub fn channel_history(&self, channel: &str, before: Option<i64>) -> Result<Vec<ChatMessage>> {
        let channel = protocol::channel(channel)?;
        if !self.has_channel(&channel)? {
            bail!("channel not found");
        }
        let db = self.lock()?;
        let mut stmt = db.prepare(
            "SELECT id, channel, author, text, created_at
             FROM channel_messages
             WHERE channel = ?1 AND (?2 IS NULL OR id < ?2)
             ORDER BY id DESC LIMIT ?3",
        )?;
        let mut rows = stmt.query(params![channel, before, HISTORY_PAGE_SIZE])?;
        let mut messages = Vec::new();
        while let Some(row) = rows.next()? {
            messages.push(ChatMessage {
                id: row.get(0)?,
                channel: Some(row.get(1)?),
                author: row.get(2)?,
                recipient: None,
                text: row.get(3)?,
                created_at: row.get(4)?,
            });
        }
        messages.reverse();
        Ok(messages)
    }

    pub fn record_dm(&self, author: &str, raw_recipient: &str, raw_text: &str) -> Result<ChatMessage> {
        let recipient = protocol::username(raw_recipient)?;
        let text = protocol::message_text(raw_text)?;
        if author == recipient {
            bail!("you cannot message yourself");
        }
        let db = self.lock()?;
        if db
            .query_row(
                "SELECT 1 FROM users WHERE username = ?1",
                [&recipient],
                |_| Ok(()),
            )
            .optional()?
            .is_none()
        {
            bail!("recipient does not exist");
        }
        let created_at = now();
        db.execute(
            "INSERT INTO direct_messages (author, recipient, text, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![author, recipient, text, created_at],
        )?;
        Ok(ChatMessage {
            id: db.last_insert_rowid(),
            channel: None,
            author: author.to_owned(),
            recipient: Some(recipient),
            text,
            created_at,
        })
    }

    pub fn direct_history(
        &self,
        raw_me: &str,
        raw_peer: &str,
        before: Option<i64>,
    ) -> Result<Vec<ChatMessage>> {
        let me = protocol::username(raw_me)?;
        let peer = protocol::username(raw_peer)?;
        let db = self.lock()?;
        if db
            .query_row(
                "SELECT 1 FROM users WHERE username = ?1",
                [&peer],
                |_| Ok(()),
            )
            .optional()?
            .is_none()
        {
            bail!("recipient does not exist");
        }
        let mut stmt = db.prepare(
            "SELECT id, author, recipient, text, created_at
             FROM direct_messages
             WHERE ((author = ?1 AND recipient = ?2) OR (author = ?2 AND recipient = ?1))
               AND (?3 IS NULL OR id < ?3)
             ORDER BY id DESC LIMIT ?4",
        )?;
        let mut rows = stmt.query(params![me, peer, before, HISTORY_PAGE_SIZE])?;
        let mut messages = Vec::new();
        while let Some(row) = rows.next()? {
            messages.push(ChatMessage {
                id: row.get(0)?,
                channel: None,
                author: row.get(1)?,
                recipient: Some(row.get(2)?),
                text: row.get(3)?,
                created_at: row.get(4)?,
            });
        }
        messages.reverse();
        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_channels_and_history_survive_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chat.sqlite3");
        let path = path.to_str().unwrap();
        {
            let store = Store::open(path).unwrap();
            store.register("ALICE", "correct horse battery").unwrap();
            store.register("BOB", "another good password").unwrap();
            assert_eq!(store.authenticate("alice", "correct horse battery").unwrap(), "alice");
            assert!(store.authenticate("alice", "wrong").is_err());
            assert!(store.register("Alice", "12345678").is_err());
            store.create_channel("movies").unwrap();
            store.record_channel_message("movies", "alice", "hello").unwrap();
            store.record_dm("alice", "bob", "private hello").unwrap();
        }
        let store = Store::open(path).unwrap();
        let history = store.channel_history("movies", None).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].text, "hello");
        assert_eq!(store.direct_history("bob", "alice", None).unwrap().len(), 1);
        assert!(store.direct_history("bob", "unknown", None).is_err());
    }

    #[test]
    fn history_is_ordered_and_paged() {
        let store = Store::open(":memory:").unwrap();
        store.register("alice", "correct horse battery").unwrap();
        for idx in 0..110 {
            store.record_channel_message("general", "alice", &format!("message {idx}")).unwrap();
        }
        let recent = store.channel_history("general", None).unwrap();
        assert_eq!(recent.len(), 100);
        assert_eq!(recent[0].text, "message 10");
        assert_eq!(recent.last().unwrap().text, "message 109");
        let older = store.channel_history("general", Some(recent[0].id)).unwrap();
        assert_eq!(older.len(), 10);
        assert_eq!(older[0].text, "message 0");
    }
}
