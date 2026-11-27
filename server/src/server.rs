//! HTTP routes and the per-connection WebSocket session.

use std::{path::PathBuf, sync::Arc, time::Duration};

use axum::{
    extract::{
        ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use tokio::{
    net::TcpListener,
    sync::{mpsc, Notify, Semaphore},
    time::timeout,
};
use tower_http::services::{ServeDir, ServeFile};

use crate::{
    auth::{self, Bucket, Throttle},
    error::{Error, Result},
    hub::{ConnId, Hub, Outbox},
    protocol::{ClientMessage, ErrorCode, ServerMessage, MAX_FRAME_BYTES, PROTOCOL_VERSION},
    store::Store,
    validate::{self, Target},
};

/// Tunables. Defaults suit a small group on a small VPS; tests shrink them.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Outbound frames a connection may have queued before it is dropped.
    pub queue: usize,
    /// Messages, edits and deletes: burst size and sustained rate.
    pub send_burst: f64,
    pub send_per_sec: f64,
    /// Close a connection that sends nothing (not even a ping) for this long.
    pub idle_timeout: Duration,
    /// Close a connection whose socket won't take a frame for this long.
    pub write_timeout: Duration,
    pub login_failures: u32,
    pub login_window: Duration,
    /// Failed sign-ins on one connection before it is closed.
    pub conn_auth_failures: u32,
    /// Concurrent Argon2 hashes (about 19 MiB each).
    pub hash_concurrency: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            queue: 512,
            send_burst: 10.0,
            send_per_sec: 3.0,
            idle_timeout: Duration::from_secs(75),
            write_timeout: Duration::from_secs(15),
            login_failures: 5,
            login_window: Duration::from_secs(15 * 60),
            conn_auth_failures: 5,
            hash_concurrency: 2,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub db_path: String,
    pub invite: Option<String>,
    /// Serve the built web client from here, if set.
    pub web_dir: Option<PathBuf>,
    pub limits: Limits,
}

pub struct App {
    store: Store,
    hub: Hub,
    invite_hash: Option<[u8; 32]>,
    throttle: Throttle,
    hashing: Semaphore,
    limits: Limits,
}

impl App {
    pub fn new(cfg: &Config) -> anyhow::Result<Arc<Self>> {
        Ok(Arc::new(Self {
            store: Store::open(&cfg.db_path)?,
            hub: Hub::default(),
            invite_hash: cfg
                .invite
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(|s| Sha256::digest(s.as_bytes()).into()),
            throttle: Throttle::new(cfg.limits.login_failures, cfg.limits.login_window),
            hashing: Semaphore::new(cfg.limits.hash_concurrency.max(1)),
            limits: cfg.limits.clone(),
        }))
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Run a blocking database call off the async threads.
    async fn db<T, F>(self: &Arc<Self>, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&Store) -> Result<T> + Send + 'static,
    {
        let app = self.clone();
        tokio::task::spawn_blocking(move || f(&app.store)).await?
    }

    async fn hash(&self, password: String) -> Result<String> {
        let _permit = self.hashing.acquire().await.map_err(anyhow::Error::from)?;
        Ok(tokio::task::spawn_blocking(move || auth::hash_password(&password)).await??)
    }

    async fn verify(&self, stored: Option<String>, password: String) -> Result<bool> {
        let _permit = self.hashing.acquire().await.map_err(anyhow::Error::from)?;
        Ok(
            tokio::task::spawn_blocking(move || {
                auth::verify_password(stored.as_deref(), &password)
            })
            .await?,
        )
    }
}

pub fn router(app: Arc<App>, web_dir: Option<PathBuf>) -> Router {
    let router = Router::new()
        .route("/ws", get(ws_upgrade))
        .route("/healthz", get(|| async { "ok" }))
        .with_state(app);
    match web_dir {
        Some(dir) => {
            let index = dir.join("index.html");
            router.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)))
        }
        None => router,
    }
}

/// Serve until ctrl-c (or the future resolves), then close every socket.
pub async fn serve(
    listener: TcpListener,
    cfg: Config,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    let app = App::new(&cfg)?;
    let closer = app.clone();
    let router = router(app, cfg.web_dir.clone());
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            shutdown.await;
            closer.hub.close_all();
        })
        .await?;
    Ok(())
}

async fn ws_upgrade(State(app): State<Arc<App>>, ws: WebSocketUpgrade) -> Response {
    ws.max_message_size(MAX_FRAME_BYTES)
        .max_frame_size(MAX_FRAME_BYTES)
        .on_upgrade(move |socket| session(app, socket))
}

fn frame(msg: &ServerMessage) -> Utf8Bytes {
    serde_json::to_string(msg)
        .expect("server messages always serialize")
        .into()
}

fn error_frame(code: ErrorCode, message: impl Into<String>) -> Utf8Bytes {
    frame(&ServerMessage::Error {
        code,
        message: message.into(),
    })
}

/// What the read loop should do after handling a frame.
enum Flow {
    Continue,
    Close,
}

struct Signed {
    id: ConnId,
    user: String,
    token: String,
}

struct Session {
    app: Arc<App>,
    out: Outbox,
    signed: Option<Signed>,
    auth_failures: u32,
    sends: Bucket,
    typing: Bucket,
}

async fn session(app: Arc<App>, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::channel::<Utf8Bytes>(app.limits.queue);
    let kill = Arc::new(Notify::new());
    let write_timeout = app.limits.write_timeout;

    let writer_kill = kill.clone();
    let mut writer = tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                _ = writer_kill.notified() => break,
                next = rx.recv() => {
                    let Some(text) = next else { break };
                    match timeout(write_timeout, sink.send(Message::Text(text))).await {
                        Ok(Ok(())) => {}
                        _ => break,
                    }
                }
            }
        }
        let _ = timeout(Duration::from_secs(2), sink.close()).await;
    });

    let mut s = Session {
        out: Outbox { tx, kill },
        signed: None,
        auth_failures: 0,
        sends: Bucket::new(app.limits.send_burst, app.limits.send_per_sec),
        typing: Bucket::new(4.0, 1.0),
        app,
    };
    let idle = s.app.limits.idle_timeout;
    let mut writer_done = false;

    loop {
        let next = tokio::select! {
            _ = &mut writer => { writer_done = true; break; }
            next = timeout(idle, stream.next()) => next,
        };
        let msg = match next {
            Ok(Some(Ok(msg))) => msg,
            _ => break, // idle timeout, closed, or protocol error
        };
        let flow = match msg {
            Message::Text(text) => s.handle_text(text.as_str()).await,
            Message::Binary(_) => {
                s.out.push(error_frame(
                    ErrorCode::BadRequest,
                    "Porch speaks JSON text frames.",
                ));
                Flow::Continue
            }
            Message::Close(_) => Flow::Close,
            Message::Ping(_) | Message::Pong(_) => Flow::Continue,
        };
        if let Flow::Close = flow {
            break;
        }
    }

    s.sign_out_locally();
    // Dropping the last sender lets the writer flush what's queued and close.
    drop(s);
    if !writer_done {
        let _ = timeout(Duration::from_secs(3), writer).await;
    }
}

impl Session {
    fn reply(&self, msg: &ServerMessage) {
        self.out.push(frame(msg));
    }

    async fn handle_text(&mut self, text: &str) -> Flow {
        let msg: ClientMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(err) => {
                self.out.push(error_frame(
                    ErrorCode::BadRequest,
                    format!("Couldn't read that request: {err}"),
                ));
                return Flow::Continue;
            }
        };
        match self.handle(msg).await {
            Ok(flow) => flow,
            Err(err) => {
                if let Error::Internal(e) = &err {
                    tracing::error!(user = self.user().unwrap_or("-"), "request failed: {e:#}");
                }
                let (code, message) = err.public();
                self.out.push(error_frame(code, message));
                if matches!(code, ErrorCode::AuthFailed | ErrorCode::TokenInvalid) {
                    self.auth_failures += 1;
                    if self.auth_failures >= self.app.limits.conn_auth_failures {
                        return Flow::Close;
                    }
                }
                Flow::Continue
            }
        }
    }

    fn user(&self) -> Option<&str> {
        self.signed.as_ref().map(|s| s.user.as_str())
    }

    fn require_user(&self) -> Result<&Signed> {
        self.signed
            .as_ref()
            .ok_or_else(|| Error::user(ErrorCode::NotSignedIn, "Sign in first."))
    }

    fn rate_limit(&mut self) -> Result<()> {
        if self.sends.take() {
            Ok(())
        } else {
            Err(Error::user(ErrorCode::RateLimited, "Slow down a little."))
        }
    }

    async fn handle(&mut self, msg: ClientMessage) -> Result<Flow> {
        use ClientMessage as C;
        let auth_msg = matches!(msg, C::Register { .. } | C::Login { .. } | C::Resume { .. });
        if auth_msg && self.signed.is_some() {
            return Err(Error::bad("Already signed in."));
        }
        match msg {
            C::Ping => self.reply(&ServerMessage::Pong),
            C::Register {
                username,
                password,
                invite,
            } => self.register(username, password, invite).await?,
            C::Login { username, password } => self.login(username, password).await?,
            C::Resume { token } => self.resume(token).await?,
            C::Logout => {
                if let Some(s) = &self.signed {
                    let token = s.token.clone();
                    self.app.db(move |st| st.revoke(&token)).await?;
                }
                return Ok(Flow::Close);
            }
            C::CreateChannel { name } => self.create_channel(name).await?,
            C::History { to, before } => self.history(to, before).await?,
            C::Send { to, text, nonce } => self.send(to, text, nonce).await?,
            C::Edit { id, text } => self.edit(id, text).await?,
            C::Delete { id } => self.delete(id).await?,
            C::Typing { to } => self.typing(to)?,
            C::MarkRead { to, id } => self.mark_read(to, id).await?,
        }
        Ok(Flow::Continue)
    }

    // ---- signing in -----------------------------------------------------

    async fn register(
        &mut self,
        username: String,
        password: String,
        invite: Option<String>,
    ) -> Result<()> {
        if let Some(expected) = self.app.invite_hash {
            let given: [u8; 32] =
                Sha256::digest(invite.unwrap_or_default().trim().as_bytes()).into();
            if given != expected {
                return Err(Error::user(
                    ErrorCode::AuthFailed,
                    "That invite code isn't right.",
                ));
            }
        }
        let user = validate::username(&username)?;
        validate::password(&password)?;
        if self
            .app
            .db({
                let u = user.clone();
                move |st| st.user_exists(&u)
            })
            .await?
        {
            return Err(Error::user(ErrorCode::Taken, "That username is taken."));
        }
        let hash = self.app.hash(password).await?;
        let token = self
            .app
            .db({
                let u = user.clone();
                move |st| {
                    st.create_user(&u, &hash)?;
                    st.create_session(&u)
                }
            })
            .await?;
        tracing::info!(user, "registered");
        self.app.hub.to_all(
            &frame(&ServerMessage::UserJoined { user: user.clone() }),
            None,
        );
        self.welcome(user, token).await
    }

    async fn login(&mut self, username: String, password: String) -> Result<()> {
        let wrong = || Error::user(ErrorCode::AuthFailed, "Wrong username or password.");
        let user = validate::username(&username).map_err(|_| wrong())?;
        self.app.throttle.check(&user)?;
        let stored = self
            .app
            .db({
                let u = user.clone();
                move |st| st.password_hash(&u)
            })
            .await?;
        if !self.app.verify(stored, password).await? {
            self.app.throttle.fail(&user);
            tracing::info!(user, "failed login");
            return Err(wrong());
        }
        self.app.throttle.clear(&user);
        let token = self
            .app
            .db({
                let u = user.clone();
                move |st| st.create_session(&u)
            })
            .await?;
        self.welcome(user, token).await
    }

    async fn resume(&mut self, token: String) -> Result<()> {
        let user = self
            .app
            .db({
                let t = token.clone();
                move |st| st.resume(&t)
            })
            .await?
            .ok_or_else(|| {
                Error::user(
                    ErrorCode::TokenInvalid,
                    "Your session ended. Sign in again.",
                )
            })?;
        self.welcome(user, token).await
    }

    async fn welcome(&mut self, user: String, token: String) -> Result<()> {
        let (channels, users, convs) = self
            .app
            .db({
                let u = user.clone();
                move |st| Ok((st.channels()?, st.users()?, st.conv_states(&u)?))
            })
            .await?;
        // `online` is computed before attaching, so add ourselves explicitly.
        let mut online = self.app.hub.online();
        if !online.contains(&user) {
            online.push(user.clone());
            online.sort();
        }
        let welcome = frame(&ServerMessage::Welcome {
            protocol: PROTOCOL_VERSION,
            user: user.clone(),
            token: token.clone(),
            channels,
            users,
            online,
            convs,
        });
        let (id, first) = self.app.hub.attach(&user, self.out.clone(), welcome);
        if first {
            let presence = frame(&ServerMessage::Presence {
                user: user.clone(),
                online: true,
            });
            self.app.hub.to_all(&presence, Some(&user));
        }
        tracing::info!(user, conn = id, "signed in");
        self.signed = Some(Signed { id, user, token });
        Ok(())
    }

    /// Leave the hub and announce going offline if this was the last device.
    fn sign_out_locally(&mut self) {
        let Some(signed) = self.signed.take() else {
            return;
        };
        if let Some((user, offline)) = self.app.hub.detach(signed.id) {
            if offline {
                let presence = frame(&ServerMessage::Presence {
                    user: user.clone(),
                    online: false,
                });
                self.app.hub.to_all(&presence, Some(&user));
            }
        }
    }

    // ---- conversations --------------------------------------------------

    /// Parse a target and check it exists. Returns it with its storage key.
    async fn conv(&self, to: &str) -> Result<(Target, String)> {
        let me = self.require_user()?.user.clone();
        let target = Target::parse(to)?;
        let key = target.key(&me);
        let check = target.clone();
        let exists = self
            .app
            .db(move |st| match &check {
                Target::Channel(name) => st.channel_exists(name),
                Target::Dm(peer) if *peer == me => Ok(false),
                Target::Dm(peer) => st.user_exists(peer),
            })
            .await?;
        if !exists {
            return Err(Error::user(
                ErrorCode::NotFound,
                match target {
                    Target::Channel(name) => format!("There's no #{name}."),
                    Target::Dm(peer) => format!("There's nobody called {peer} here."),
                },
            ));
        }
        Ok((target, key))
    }

    async fn create_channel(&mut self, name: String) -> Result<()> {
        let me = self.require_user()?.user.clone();
        self.rate_limit()?;
        let name = validate::channel(&name)?;
        self.app
            .db({
                let n = name.clone();
                move |st| st.create_channel(&n, &me)
            })
            .await?;
        self.app
            .hub
            .to_all(&frame(&ServerMessage::ChannelCreated { name }), None);
        Ok(())
    }

    async fn history(&mut self, to: String, before: Option<i64>) -> Result<()> {
        let (_, key) = self.conv(&to).await?;
        let (messages, has_more) = self.app.db(move |st| st.history(&key, before)).await?;
        self.reply(&ServerMessage::History {
            to,
            messages,
            has_more,
        });
        Ok(())
    }

    async fn send(&mut self, to: String, text: String, nonce: Option<String>) -> Result<()> {
        self.require_user()?;
        self.rate_limit()?;
        let text = validate::text(&text)?;
        let nonce = nonce.filter(|n| n.len() <= 64);
        let (_, key) = self.conv(&to).await?;
        let me = self.require_user()?.user.clone();
        let (conv, message) = self
            .app
            .db(move |st| {
                let m = st.insert_message(&key, &me, &text)?;
                st.mark_read(&me, &key, m.id)?;
                Ok((key, m))
            })
            .await?;
        self.app
            .hub
            .to_conv(&conv, &frame(&ServerMessage::Message { message, nonce }));
        Ok(())
    }

    async fn edit(&mut self, id: i64, text: String) -> Result<()> {
        let me = self.require_user()?.user.clone();
        self.rate_limit()?;
        let text = validate::text(&text)?;
        let (conv, message) = self
            .app
            .db(move |st| st.edit_message(id, &me, &text))
            .await?;
        self.app
            .hub
            .to_conv(&conv, &frame(&ServerMessage::Edited { message }));
        Ok(())
    }

    async fn delete(&mut self, id: i64) -> Result<()> {
        let me = self.require_user()?.user.clone();
        self.rate_limit()?;
        let author = me.clone();
        let conv = self.app.db(move |st| st.delete_message(id, &me)).await?;
        let to = validate::view_of(&conv, &author);
        self.app
            .hub
            .to_conv(&conv, &frame(&ServerMessage::Deleted { id, to, author }));
        Ok(())
    }

    fn typing(&mut self, to: String) -> Result<()> {
        let me = self.require_user()?.user.clone();
        if !self.typing.take() {
            return Ok(()); // excess typing pings are dropped quietly
        }
        match Target::parse(&to)? {
            Target::Channel(name) => {
                let f = frame(&ServerMessage::Typing {
                    user: me.clone(),
                    to: format!("#{name}"),
                });
                self.app.hub.to_all(&f, Some(&me));
            }
            Target::Dm(peer) if peer != me => {
                let f = frame(&ServerMessage::Typing {
                    user: me.clone(),
                    to: format!("@{me}"),
                });
                self.app.hub.to_users(&[&peer], &f, None);
            }
            Target::Dm(_) => {}
        }
        Ok(())
    }

    async fn mark_read(&mut self, to: String, id: i64) -> Result<()> {
        let signed = self.require_user()?;
        let (me, conn) = (signed.user.clone(), signed.id);
        let key = Target::parse(&to)?.key(&me);
        let user = me.clone();
        if self.app.db(move |st| st.mark_read(&user, &key, id)).await? {
            self.app
                .hub
                .to_users(&[&me], &frame(&ServerMessage::Read { to, id }), Some(conn));
        }
        Ok(())
    }
}
