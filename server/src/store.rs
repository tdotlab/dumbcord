//! SQLite persistence. All methods are blocking; the server calls them from
//! `spawn_blocking`. One connection behind a mutex is plenty for a friend
//! group, and WAL keeps reads cheap.

use std::{
    fs,
    path::Path,
    sync::{Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
};

use rand::RngCore;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::protocol::{ChatMessage, ConvState, ErrorCode, HISTORY_PAGE};
use crate::validate::{dm_members, view_of};

/// Sessions unused for this long stop working.
const SESSION_TTL_MS: i64 = 90 * 24 * 60 * 60 * 1000;

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Each entry moves the schema forward by one version. Never edit an entry
/// that has shipped; append a new one.
const MIGRATIONS: &[&str] = &[
    // 1: initial schema
    "CREATE TABLE users (
        username TEXT PRIMARY KEY,
        password_hash TEXT NOT NULL,
        created_at INTEGER NOT NULL
    );
    CREATE TABLE sessions (
        token_hash TEXT PRIMARY KEY,
        username TEXT NOT NULL REFERENCES users(username) ON DELETE CASCADE,
        created_at INTEGER NOT NULL,
        last_seen INTEGER NOT NULL
    );
    CREATE INDEX sessions_by_user ON sessions(username);
    CREATE TABLE channels (
        name TEXT PRIMARY KEY,
        created_by TEXT,
        created_at INTEGER NOT NULL
    );
    CREATE TABLE messages (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        conv TEXT NOT NULL,
        author TEXT NOT NULL REFERENCES users(username) ON DELETE CASCADE,
        text TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        edited_at INTEGER
    );
    CREATE INDEX messages_by_conv ON messages(conv, id);
    CREATE TABLE reads (
        username TEXT NOT NULL REFERENCES users(username) ON DELETE CASCADE,
        conv TEXT NOT NULL,
        last_read INTEGER NOT NULL,
        PRIMARY KEY (username, conv)
    ) WITHOUT ROWID;",
];

pub struct Store {
    db: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        if path != ":memory:" {
            if let Some(parent) = Path::new(path).parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)?;
                }
            }
        }
        let mut db = Connection::open(path)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "synchronous", "NORMAL")?;
        db.pragma_update(None, "foreign_keys", "ON")?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;

        let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let version = usize::try_from(version).unwrap_or(usize::MAX);
        if version > MIGRATIONS.len() {
            anyhow::bail!(
                "database schema v{version} is newer than this server (v{}); upgrade porch-server",
                MIGRATIONS.len()
            );
        }
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version) {
            let tx = db.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
            tx.commit()?;
        }
        db.execute(
            "INSERT OR IGNORE INTO channels (name, created_by, created_at) VALUES ('general', NULL, ?1)",
            [now_ms()],
        )?;
        db.execute(
            "DELETE FROM sessions WHERE last_seen < ?1",
            [now_ms() - SESSION_TTL_MS],
        )?;
        Ok(Self { db: Mutex::new(db) })
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        // A panic while holding the lock leaves SQLite itself consistent, so
        // keep serving rather than poisoning every later request.
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }

    // ---- accounts -------------------------------------------------------

    pub fn create_user(&self, username: &str, password_hash: &str) -> Result<()> {
        let db = self.lock();
        let inserted = db.execute(
            "INSERT OR IGNORE INTO users (username, password_hash, created_at) VALUES (?1, ?2, ?3)",
            params![username, password_hash, now_ms()],
        )?;
        if inserted == 0 {
            return Err(Error::user(ErrorCode::Taken, "That username is taken."));
        }
        Ok(())
    }

    pub fn password_hash(&self, username: &str) -> Result<Option<String>> {
        Ok(self
            .lock()
            .query_row(
                "SELECT password_hash FROM users WHERE username = ?1",
                [username],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Replace a password and sign the user out everywhere.
    pub fn set_password(&self, username: &str, password_hash: &str) -> Result<bool> {
        let mut db = self.lock();
        let tx = db.transaction()?;
        let changed = tx.execute(
            "UPDATE users SET password_hash = ?2 WHERE username = ?1",
            params![username, password_hash],
        )?;
        tx.execute("DELETE FROM sessions WHERE username = ?1", [username])?;
        tx.commit()?;
        Ok(changed > 0)
    }

    pub fn user_exists(&self, username: &str) -> Result<bool> {
        Ok(self
            .lock()
            .query_row(
                "SELECT 1 FROM users WHERE username = ?1",
                [username],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    pub fn users(&self) -> Result<Vec<String>> {
        let db = self.lock();
        let mut stmt = db.prepare_cached("SELECT username FROM users ORDER BY username")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // ---- sessions -------------------------------------------------------

    /// Issue a new session token. Only its hash is stored.
    pub fn create_session(&self, username: &str) -> Result<String> {
        let mut raw = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut raw);
        let token = hex::encode(raw);
        let now = now_ms();
        self.lock().execute(
            "INSERT INTO sessions (token_hash, username, created_at, last_seen) VALUES (?1, ?2, ?3, ?3)",
            params![token_hash(&token), username, now],
        )?;
        Ok(token)
    }

    /// The user a live token belongs to, refreshing its expiry.
    pub fn resume(&self, token: &str) -> Result<Option<String>> {
        let hash = token_hash(token);
        let now = now_ms();
        let db = self.lock();
        let user: Option<String> = db
            .query_row(
                "SELECT username FROM sessions WHERE token_hash = ?1 AND last_seen >= ?2",
                params![hash, now - SESSION_TTL_MS],
                |r| r.get(0),
            )
            .optional()?;
        if user.is_some() {
            db.execute(
                "UPDATE sessions SET last_seen = ?2 WHERE token_hash = ?1",
                params![hash, now],
            )?;
        }
        Ok(user)
    }

    pub fn revoke(&self, token: &str) -> Result<()> {
        self.lock().execute(
            "DELETE FROM sessions WHERE token_hash = ?1",
            [token_hash(token)],
        )?;
        Ok(())
    }

    // ---- channels -------------------------------------------------------

    pub fn channels(&self) -> Result<Vec<String>> {
        let db = self.lock();
        let mut stmt = db.prepare_cached("SELECT name FROM channels ORDER BY name")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn channel_exists(&self, name: &str) -> Result<bool> {
        Ok(self
            .lock()
            .query_row("SELECT 1 FROM channels WHERE name = ?1", [name], |_| Ok(()))
            .optional()?
            .is_some())
    }

    pub fn create_channel(&self, name: &str, by: &str) -> Result<()> {
        let inserted = self.lock().execute(
            "INSERT OR IGNORE INTO channels (name, created_by, created_at) VALUES (?1, ?2, ?3)",
            params![name, by, now_ms()],
        )?;
        if inserted == 0 {
            return Err(Error::user(
                ErrorCode::Taken,
                format!("#{name} already exists."),
            ));
        }
        Ok(())
    }

    // ---- messages -------------------------------------------------------

    fn row_to_message(conv: &str, r: &rusqlite::Row<'_>) -> rusqlite::Result<ChatMessage> {
        let author: String = r.get("author")?;
        Ok(ChatMessage {
            id: r.get("id")?,
            to: view_of(conv, &author),
            author,
            text: r.get("text")?,
            created_at: r.get("created_at")?,
            edited_at: r.get("edited_at")?,
        })
    }

    pub fn insert_message(&self, conv: &str, author: &str, text: &str) -> Result<ChatMessage> {
        let created_at = now_ms();
        let db = self.lock();
        db.execute(
            "INSERT INTO messages (conv, author, text, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![conv, author, text, created_at],
        )?;
        Ok(ChatMessage {
            id: db.last_insert_rowid(),
            to: view_of(conv, author),
            author: author.to_owned(),
            text: text.to_owned(),
            created_at,
            edited_at: None,
        })
    }

    /// One page of history, oldest first, plus whether older messages exist.
    pub fn history(&self, conv: &str, before: Option<i64>) -> Result<(Vec<ChatMessage>, bool)> {
        let db = self.lock();
        let mut stmt = db.prepare_cached(
            "SELECT id, author, text, created_at, edited_at FROM messages
             WHERE conv = ?1 AND id < ?2 ORDER BY id DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![conv, before.unwrap_or(i64::MAX), HISTORY_PAGE as i64 + 1],
            |r| Self::row_to_message(conv, r),
        )?;
        let mut messages: Vec<ChatMessage> = rows.collect::<rusqlite::Result<_>>()?;
        let has_more = messages.len() > HISTORY_PAGE;
        messages.truncate(HISTORY_PAGE);
        messages.reverse();
        Ok((messages, has_more))
    }

    /// The conversation key and author of a message.
    fn owner(db: &Connection, id: i64) -> Result<(String, String)> {
        db.query_row(
            "SELECT conv, author FROM messages WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| Error::user(ErrorCode::NotFound, "That message is gone."))
    }

    fn require_author(actual: &str, user: &str) -> Result<()> {
        if actual != user {
            return Err(Error::user(
                ErrorCode::Forbidden,
                "You can only change your own messages.",
            ));
        }
        Ok(())
    }

    pub fn edit_message(&self, id: i64, user: &str, text: &str) -> Result<(String, ChatMessage)> {
        let db = self.lock();
        let (conv, author) = Self::owner(&db, id)?;
        Self::require_author(&author, user)?;
        db.execute(
            "UPDATE messages SET text = ?2, edited_at = ?3 WHERE id = ?1",
            params![id, text, now_ms()],
        )?;
        let message = db.query_row(
            "SELECT id, author, text, created_at, edited_at FROM messages WHERE id = ?1",
            [id],
            |r| Self::row_to_message(&conv, r),
        )?;
        Ok((conv, message))
    }

    pub fn delete_message(&self, id: i64, user: &str) -> Result<String> {
        let db = self.lock();
        let (conv, author) = Self::owner(&db, id)?;
        Self::require_author(&author, user)?;
        db.execute("DELETE FROM messages WHERE id = ?1", [id])?;
        Ok(conv)
    }

    // ---- read state -----------------------------------------------------

    /// Latest message and read position for every conversation `user` is in.
    pub fn conv_states(&self, user: &str) -> Result<Vec<ConvState>> {
        let db = self.lock();
        let mut latest = db.prepare_cached(
            "SELECT g.conv, g.last_id, COALESCE(r.last_read, 0)
             FROM (SELECT conv, MAX(id) AS last_id FROM messages GROUP BY conv) g
             LEFT JOIN reads r ON r.conv = g.conv AND r.username = ?1",
        )?;
        let rows = latest.query_map([user], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (conv, last_id, last_read) = row?;
            if let Some((a, b)) = dm_members(&conv) {
                if a != user && b != user {
                    continue;
                }
            }
            out.push(ConvState {
                to: view_of(&conv, user),
                last_id,
                last_read,
            });
        }
        Ok(out)
    }

    /// Move a read marker forward. Returns false if it was already there.
    pub fn mark_read(&self, user: &str, conv: &str, id: i64) -> Result<bool> {
        let changed = self.lock().execute(
            "INSERT INTO reads (username, conv, last_read) VALUES (?1, ?2, ?3)
             ON CONFLICT (username, conv) DO UPDATE SET last_read = excluded.last_read
             WHERE excluded.last_read > reads.last_read",
            params![user, conv, id],
        )?;
        Ok(changed > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        let s = Store::open(":memory:").unwrap();
        s.create_user("alice", "x").unwrap();
        s.create_user("bob", "x").unwrap();
        s.create_user("carol", "x").unwrap();
        s
    }

    #[test]
    fn data_survives_reopening_and_migrations_are_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("porch.sqlite3");
        let path = path.to_str().unwrap();
        let token;
        {
            let s = Store::open(path).unwrap();
            s.create_user("alice", "hash").unwrap();
            s.insert_message("#general", "alice", "hello").unwrap();
            token = s.create_session("alice").unwrap();
        }
        let s = Store::open(path).unwrap();
        assert_eq!(s.history("#general", None).unwrap().0[0].text, "hello");
        assert_eq!(s.resume(&token).unwrap().as_deref(), Some("alice"));
        assert!(matches!(
            s.create_user("alice", "other"),
            Err(Error::User(ErrorCode::Taken, _))
        ));
    }

    #[test]
    fn history_pages_with_has_more() {
        let s = store();
        for i in 0..(HISTORY_PAGE + 5) {
            s.insert_message("#general", "alice", &format!("m{i}"))
                .unwrap();
        }
        let (page, more) = s.history("#general", None).unwrap();
        assert_eq!(page.len(), HISTORY_PAGE);
        assert!(more);
        assert_eq!(page.last().unwrap().text, format!("m{}", HISTORY_PAGE + 4));
        let (older, more) = s.history("#general", Some(page[0].id)).unwrap();
        assert_eq!(older.len(), 5);
        assert!(!more);
        assert_eq!(older[0].text, "m0");
    }

    #[test]
    fn dm_messages_read_from_each_side() {
        let s = store();
        let m = s.insert_message("@alice:bob", "bob", "psst").unwrap();
        assert_eq!(m.to, "@alice");
        let alice = s.conv_states("alice").unwrap();
        assert_eq!(
            alice,
            vec![ConvState {
                to: "@bob".into(),
                last_id: m.id,
                last_read: 0
            }]
        );
        assert!(s.conv_states("carol").unwrap().is_empty());
        assert!(s.mark_read("alice", "@alice:bob", m.id).unwrap());
        assert!(!s.mark_read("alice", "@alice:bob", m.id - 1).unwrap());
        assert_eq!(s.conv_states("alice").unwrap()[0].last_read, m.id);
    }

    #[test]
    fn only_authors_edit_and_delete() {
        let s = store();
        let m = s.insert_message("#general", "alice", "frist").unwrap();
        assert!(matches!(
            s.edit_message(m.id, "bob", "lol"),
            Err(Error::User(ErrorCode::Forbidden, _))
        ));
        let (conv, edited) = s.edit_message(m.id, "alice", "first").unwrap();
        assert_eq!(conv, "#general");
        assert_eq!(edited.text, "first");
        assert!(edited.edited_at.is_some());
        assert!(s.delete_message(m.id, "bob").is_err());
        s.delete_message(m.id, "alice").unwrap();
        assert!(s.history("#general", None).unwrap().0.is_empty());
    }

    #[test]
    fn password_change_revokes_sessions() {
        let s = store();
        let t = s.create_session("alice").unwrap();
        assert!(s.set_password("alice", "new").unwrap());
        assert_eq!(s.resume(&t).unwrap(), None);
        s.revoke("nonsense").unwrap();
    }
}
