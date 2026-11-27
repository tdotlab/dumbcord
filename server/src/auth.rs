//! Password hashing and login throttling.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use crate::error::{Error, Result};
use crate::protocol::ErrorCode;

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("password hashing failed: {e}"))
}

/// Check a password. With no stored hash, burn the same time against a dummy
/// so response timing doesn't reveal which usernames exist.
pub fn verify_password(stored: Option<&str>, password: &str) -> bool {
    static DUMMY: OnceLock<String> = OnceLock::new();
    let dummy = DUMMY.get_or_init(|| hash_password("porch-dummy-password").unwrap_or_default());
    let (hash, real) = match stored {
        Some(h) => (h, true),
        None => (dummy.as_str(), false),
    };
    let ok = PasswordHash::new(hash)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false);
    ok && real
}

/// Per-username failed-login counter. Five misses lock the name for the rest
/// of a fifteen-minute window, across every connection.
pub struct Throttle {
    max_failures: u32,
    window: Duration,
    entries: Mutex<HashMap<String, (u32, Instant)>>,
}

impl Throttle {
    pub fn new(max_failures: u32, window: Duration) -> Self {
        Self {
            max_failures,
            window,
            entries: Mutex::new(HashMap::new()),
        }
    }

    fn entries(&self) -> std::sync::MutexGuard<'_, HashMap<String, (u32, Instant)>> {
        self.entries.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn check(&self, user: &str) -> Result<()> {
        let entries = self.entries();
        if let Some(&(count, since)) = entries.get(user) {
            let elapsed = since.elapsed();
            if count >= self.max_failures && elapsed < self.window {
                let minutes = (self.window - elapsed).as_secs().div_ceil(60).max(1);
                return Err(Error::user(
                    ErrorCode::RateLimited,
                    format!(
                        "Too many wrong passwords for {user}. Try again in {minutes} minute{}.",
                        if minutes == 1 { "" } else { "s" }
                    ),
                ));
            }
        }
        Ok(())
    }

    pub fn fail(&self, user: &str) {
        let mut entries = self.entries();
        if entries.len() > 10_000 {
            let window = self.window;
            entries.retain(|_, (_, since)| since.elapsed() < window);
        }
        let entry = entries
            .entry(user.to_owned())
            .or_insert((0, Instant::now()));
        if entry.1.elapsed() >= self.window {
            *entry = (0, Instant::now());
        }
        entry.0 += 1;
    }

    pub fn clear(&self, user: &str) {
        self.entries().remove(user);
    }
}

/// A token bucket for per-connection request limits.
pub struct Bucket {
    capacity: f64,
    per_sec: f64,
    tokens: f64,
    last: Instant,
}

impl Bucket {
    pub fn new(capacity: f64, per_sec: f64) -> Self {
        Self {
            capacity,
            per_sec,
            tokens: capacity,
            last: Instant::now(),
        }
    }

    pub fn take(&mut self) -> bool {
        let now = Instant::now();
        self.tokens = (self.tokens + now.duration_since(self.last).as_secs_f64() * self.per_sec)
            .min(self.capacity);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_verify() {
        let h = hash_password("correct horse").unwrap();
        assert!(verify_password(Some(&h), "correct horse"));
        assert!(!verify_password(Some(&h), "wrong horse"));
        assert!(!verify_password(None, "porch-dummy-password"));
        assert!(!verify_password(Some("garbage"), "x"));
    }

    #[test]
    fn throttle_locks_after_max_failures() {
        let t = Throttle::new(3, Duration::from_secs(60));
        for _ in 0..3 {
            t.check("alice").unwrap();
            t.fail("alice");
        }
        assert!(t.check("alice").is_err());
        assert!(t.check("bob").is_ok());
        t.clear("alice");
        assert!(t.check("alice").is_ok());
    }

    #[test]
    fn bucket_refills() {
        let mut b = Bucket::new(2.0, 1000.0);
        assert!(b.take() && b.take());
        std::thread::sleep(Duration::from_millis(5));
        assert!(b.take());
    }
}
