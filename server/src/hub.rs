//! Live connections and fan-out.
//!
//! Every connection owns a bounded outbound queue. Fan-out never waits on a
//! socket: it does a `try_send`, and a connection whose queue is full is told
//! to close. One stalled client can no longer freeze everyone else.

use std::{
    collections::{BTreeSet, HashMap},
    sync::{Arc, Mutex, MutexGuard},
};

use axum::extract::ws::Utf8Bytes;
use tokio::sync::{mpsc, Notify};

use crate::validate::dm_members;

pub type ConnId = u64;

#[derive(Clone)]
pub struct Outbox {
    pub tx: mpsc::Sender<Utf8Bytes>,
    pub kill: Arc<Notify>,
}

impl Outbox {
    /// Queue a frame, or cut the connection loose if it has fallen behind.
    pub fn push(&self, frame: Utf8Bytes) {
        if let Err(mpsc::error::TrySendError::Full(_)) = self.tx.try_send(frame) {
            self.kill.notify_one();
        }
    }
}

struct Conn {
    user: String,
    out: Outbox,
}

#[derive(Default)]
struct Inner {
    next_id: ConnId,
    conns: HashMap<ConnId, Conn>,
    by_user: HashMap<String, Vec<ConnId>>,
}

#[derive(Default)]
pub struct Hub {
    inner: Mutex<Inner>,
}

impl Hub {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Register a signed-in connection and queue its first frame while the
    /// lock is held, so nothing broadcast afterwards can arrive before it.
    /// Returns the id and whether this is the user's first live connection.
    pub fn attach(&self, user: &str, out: Outbox, first_frame: Utf8Bytes) -> (ConnId, bool) {
        let mut inner = self.lock();
        inner.next_id += 1;
        let id = inner.next_id;
        out.push(first_frame);
        inner.conns.insert(
            id,
            Conn {
                user: user.to_owned(),
                out,
            },
        );
        let ids = inner.by_user.entry(user.to_owned()).or_default();
        ids.push(id);
        (id, ids.len() == 1)
    }

    /// Remove a connection. Returns its user and whether they are now offline.
    pub fn detach(&self, id: ConnId) -> Option<(String, bool)> {
        let mut inner = self.lock();
        let conn = inner.conns.remove(&id)?;
        let ids = inner.by_user.get_mut(&conn.user)?;
        ids.retain(|&c| c != id);
        let offline = ids.is_empty();
        if offline {
            inner.by_user.remove(&conn.user);
        }
        Some((conn.user, offline))
    }

    pub fn online(&self) -> Vec<String> {
        self.lock()
            .by_user
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn is_online(&self, user: &str) -> bool {
        self.lock().by_user.contains_key(user)
    }

    /// Every connection, optionally skipping one user's.
    pub fn to_all(&self, frame: &Utf8Bytes, except_user: Option<&str>) {
        let inner = self.lock();
        for conn in inner.conns.values() {
            if Some(conn.user.as_str()) != except_user {
                conn.out.push(frame.clone());
            }
        }
    }

    /// Every connection of the given users, optionally skipping one connection.
    pub fn to_users(&self, users: &[&str], frame: &Utf8Bytes, except_conn: Option<ConnId>) {
        let inner = self.lock();
        for user in users {
            for id in inner.by_user.get(*user).into_iter().flatten() {
                if Some(*id) != except_conn {
                    if let Some(conn) = inner.conns.get(id) {
                        conn.out.push(frame.clone());
                    }
                }
            }
        }
    }

    /// Everyone who can see a conversation: all users for a channel, the two
    /// members for a DM.
    pub fn to_conv(&self, conv: &str, frame: &Utf8Bytes) {
        match dm_members(conv) {
            Some((a, b)) => self.to_users(&[a, b], frame, None),
            None => self.to_all(frame, None),
        }
    }

    /// Ask every connection to close, for shutdown.
    pub fn close_all(&self) {
        for conn in self.lock().conns.values() {
            conn.out.kill.notify_one();
        }
    }
}
