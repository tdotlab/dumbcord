use std::{
    collections::{HashMap, HashSet},
    env,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, Context, Result};

use crate::{
    protocol::{self, ClientMessage, ServerMessage, MAX_FRAME_BYTES},
    store::Store,
};

type Writer = Arc<Mutex<TcpStream>>;

#[derive(Clone)]
struct Session {
    username: String,
    channel: String,
    writer: Writer,
}

struct State {
    store: Store,
    sessions: Mutex<HashMap<u64, Session>>,
    invite: Option<String>,
    next_id: AtomicU64,
}

impl State {
    fn new(store: Store, invite: Option<String>) -> Self {
        Self {
            store,
            sessions: Mutex::new(HashMap::new()),
            invite,
            next_id: AtomicU64::new(1),
        }
    }

    fn session_writers(&self) -> Result<Vec<Writer>> {
        Ok(self
            .sessions
            .lock()
            .map_err(|_| anyhow!("sessions lock poisoned"))?
            .values()
            .map(|session| session.writer.clone())
            .collect())
    }

    fn broadcast(&self, event: &ServerMessage) -> Result<()> {
        for writer in self.session_writers()? {
            let _ = send_event(&writer, event);
        }
        Ok(())
    }

    fn broadcast_users(&self) -> Result<()> {
        let members = self.store.users()?;
        let mut online: Vec<String> = self
            .sessions
            .lock()
            .map_err(|_| anyhow!("sessions lock poisoned"))?
            .values()
            .map(|session| session.username.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        online.sort();
        self.broadcast(&ServerMessage::Users { members, online })
    }

    fn send_to_pair(&self, a: &str, b: &str, event: &ServerMessage) -> Result<()> {
        let targets: Vec<Writer> = self
            .sessions
            .lock()
            .map_err(|_| anyhow!("sessions lock poisoned"))?
            .values()
            .filter(|session| session.username == a || session.username == b)
            .map(|session| session.writer.clone())
            .collect();
        for writer in targets {
            let _ = send_event(&writer, event);
        }
        Ok(())
    }

    fn authenticate(&self, request: ClientMessage) -> Result<String> {
        match request {
            ClientMessage::Register {
                username,
                password,
                invite,
            } => {
                if let Some(required) = &self.invite {
                    if invite.as_deref() != Some(required.as_str()) {
                        bail!("valid invite code required");
                    }
                }
                self.store.register(&username, &password)
            }
            ClientMessage::Login { username, password } => {
                self.store.authenticate(&username, &password)
            }
            _ => bail!("log in or register before using chat"),
        }
    }

    fn join(&self, id: u64, channel: &str) -> Result<()> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| anyhow!("sessions lock poisoned"))?;
        let session = sessions
            .get_mut(&id)
            .ok_or_else(|| anyhow!("session closed"))?;
        session.channel = channel.to_owned();
        Ok(())
    }

    fn typing(&self, id: u64, name: &str, channel: &str) -> Result<()> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| anyhow!("sessions lock poisoned"))?;
        let current = sessions
            .get(&id)
            .ok_or_else(|| anyhow!("session closed"))?;
        if current.channel != channel {
            bail!("join the channel before sending typing indicators");
        }
        let recipients: Vec<Writer> = sessions
            .iter()
            .filter(|(other_id, session)| **other_id != id && session.channel == channel)
            .map(|(_, session)| session.writer.clone())
            .collect();
        drop(sessions);
        let event = ServerMessage::Typing {
            channel: channel.to_owned(),
            username: name.to_owned(),
        };
        for writer in recipients {
            let _ = send_event(&writer, &event);
        }
        Ok(())
    }

    fn handle(
        &self,
        id: u64,
        name: &str,
        request: ClientMessage,
        writer: &Writer,
        last_typing: &mut Instant,
    ) -> Result<()> {
        match request {
            ClientMessage::Channels => {
                send_event(
                    writer,
                    &ServerMessage::Channels {
                        channels: self.store.channels()?,
                    },
                )?;
            }
            ClientMessage::Users => self.broadcast_users()?,
            ClientMessage::CreateChannel { name } => {
                self.store.create_channel(&name)?;
                self.broadcast(&ServerMessage::Channels {
                    channels: self.store.channels()?,
                })?;
            }
            ClientMessage::Join { channel } => {
                let channel = protocol::channel(&channel)?;
                if !self.store.has_channel(&channel)? {
                    bail!("channel not found");
                }
                self.join(id, &channel)?;
                let messages = self.store.channel_history(&channel, None)?;
                send_event(writer, &ServerMessage::History { channel, messages })?;
            }
            ClientMessage::History { channel, before } => {
                let channel = protocol::channel(&channel)?;
                let messages = self.store.channel_history(&channel, before)?;
                send_event(writer, &ServerMessage::History { channel, messages })?;
            }
            ClientMessage::Send { channel, text } => {
                let message = self.store.record_channel_message(&channel, name, &text)?;
                self.broadcast(&ServerMessage::Message { message })?;
            }
            ClientMessage::DirectMessage { to, text } => {
                let message = self.store.record_dm(name, &to, &text)?;
                let recipient = message
                    .recipient
                    .as_deref()
                    .ok_or_else(|| anyhow!("missing recipient"))?;
                self.send_to_pair(
                    name,
                    recipient,
                    &ServerMessage::DirectMessage {
                        message: message.clone(),
                    },
                )?;
            }
            ClientMessage::DirectHistory { peer, before } => {
                let peer = protocol::username(&peer)?;
                let messages = self.store.direct_history(name, &peer, before)?;
                send_event(writer, &ServerMessage::DirectHistory { peer, messages })?;
            }
            ClientMessage::Typing { channel } => {
                let channel = protocol::channel(&channel)?;
                if last_typing.elapsed() >= Duration::from_secs(2) {
                    self.typing(id, name, &channel)?;
                    *last_typing = Instant::now();
                }
            }
            ClientMessage::Ping => send_event(writer, &ServerMessage::Pong)?,
            ClientMessage::Register { .. } | ClientMessage::Login { .. } => {
                bail!("already authenticated");
            }
        }
        Ok(())
    }

    fn remove(&self, id: u64) -> Result<()> {
        let removed = self
            .sessions
            .lock()
            .map_err(|_| anyhow!("sessions lock poisoned"))?
            .remove(&id);
        if removed.is_some() {
            self.broadcast_users()?;
        }
        Ok(())
    }
}

fn send_event(writer: &Writer, event: &ServerMessage) -> Result<()> {
    let mut socket = writer
        .lock()
        .map_err(|_| anyhow!("socket lock poisoned"))?;
    serde_json::to_writer(&mut *socket, event)?;
    socket.write_all(b"\n")?;
    socket.flush()?;
    Ok(())
}

fn serve_client(stream: TcpStream, id: u64, shared: &Arc<State>) -> Result<()> {
    stream.set_nodelay(true)?;
    let reader_socket = stream.try_clone()?;
    let writer: Writer = Arc::new(Mutex::new(stream));
    let mut reader = BufReader::new(reader_socket);
    let mut signed_in: Option<String> = None;
    let mut failed_auth = 0;
    let mut last_typing = Instant::now() - Duration::from_secs(3);

    let result = (|| -> Result<()> {
        loop {
            let mut frame = Vec::new();
            let bytes = {
                let mut bounded = (&mut reader).take((MAX_FRAME_BYTES + 1) as u64);
                bounded.read_until(b'\n', &mut frame)?
            };

            if bytes == 0 {
                break;
            }
            if bytes > MAX_FRAME_BYTES || !frame.ends_with(b"\n") {
                let _ = send_event(
                    &writer,
                    &ServerMessage::Error {
                        message: "request exceeds frame limit".to_owned(),
                    },
                );
                break;
            }

            let request = match serde_json::from_slice::<ClientMessage>(&frame) {
                Ok(value) => value,
                Err(_) => {
                    let _ = send_event(
                        &writer,
                        &ServerMessage::Error {
                            message: "invalid JSON protocol message".to_owned(),
                        },
                    );
                    continue;
                }
            };

            let outcome = if let Some(username) = &signed_in {
                shared.handle(id, username, request, &writer, &mut last_typing)
            } else {
                match shared.authenticate(request) {
                    Ok(username) => {
                        send_event(
                            &writer,
                            &ServerMessage::Authenticated {
                                username: username.clone(),
                            },
                        )?;
                        shared
                            .sessions
                            .lock()
                            .map_err(|_| anyhow!("sessions lock poisoned"))?
                            .insert(
                                id,
                                Session {
                                    username: username.clone(),
                                    channel: "general".to_owned(),
                                    writer: writer.clone(),
                                },
                            );
                        signed_in = Some(username);
                        send_event(
                            &writer,
                            &ServerMessage::Channels {
                                channels: shared.store.channels()?,
                            },
                        )?;
                        send_event(
                            &writer,
                            &ServerMessage::History {
                                channel: "general".to_owned(),
                                messages: shared.store.channel_history("general", None)?,
                            },
                        )?;
                        shared.broadcast_users()
                    }
                    Err(error) => Err(error),
                }
            };

            if let Err(error) = outcome {
                let _ = send_event(
                    &writer,
                    &ServerMessage::Error {
                        message: error.to_string(),
                    },
                );
                if signed_in.is_none() {
                    failed_auth += 1;
                    if failed_auth >= 5 {
                        break;
                    }
                }
            }
        }
        Ok(())
    })();

    let _ = shared.remove(id);
    result
}

pub fn run(bind: &str, db_path: &str) -> Result<()> {
    let store = Store::open(db_path).context("open SQLite database")?;
    let invite = env::var("DUMBCORD_INVITE")
        .ok()
        .filter(|value| !value.is_empty());
    let shared = Arc::new(State::new(store, invite.clone()));
    let listener = TcpListener::bind(bind).with_context(|| format!("bind server to {bind}"))?;

    println!("Dumbcord server listening on {bind}; database: {db_path}");
    if invite.is_none() {
        println!("Registration is open. Set DUMBCORD_INVITE to require an invite.");
    }
    println!("WARNING: unencrypted TCP. Use a private LAN or VPN; never public-forward this port.");

    for accepted in listener.incoming() {
        match accepted {
            Ok(stream) => {
                let id = shared.next_id.fetch_add(1, Ordering::Relaxed);
                let shared = Arc::clone(&shared);
                thread::spawn(move || {
                    if let Err(error) = serve_client(stream, id, &shared) {
                        eprintln!("connection {id} ended: {error:#}");
                    }
                });
            }
            Err(error) => eprintln!("accept failed: {error}"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;

    #[test]
    fn register_and_exchange_messages_over_tcp() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let shared = Arc::new(State::new(Store::open(":memory:").unwrap(), Some("secret".into())));
        let host = shared.clone();

        let join_handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            serve_client(stream, 1, &host).unwrap();
        });

        let mut socket = TcpStream::connect(address).unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let mut reader = BufReader::new(socket.try_clone().unwrap());

        let register = serde_json::to_string(&ClientMessage::Register {
            username: "alice".into(),
            password: "eight characters".into(),
            invite: Some("secret".into()),
        }).unwrap();
        socket.write_all(format!("{register}\n").as_bytes()).unwrap();

        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert!(matches!(
            serde_json::from_str::<ServerMessage>(&line).unwrap(),
            ServerMessage::Authenticated { .. }
        ));

        // Initial channel list, general history, and online snapshot.
        for _ in 0..3 {
            line.clear();
            reader.read_line(&mut line).unwrap();
        }

        let send = serde_json::to_string(&ClientMessage::Send {
            channel: "general".into(),
            text: "hello from integration test".into(),
        }).unwrap();
        socket.write_all(format!("{send}\n").as_bytes()).unwrap();
        line.clear();
        reader.read_line(&mut line).unwrap();
        match serde_json::from_str::<ServerMessage>(&line).unwrap() {
            ServerMessage::Message { message } => assert_eq!(message.text, "hello from integration test"),
            other => panic!("unexpected response: {other:?}"),
        }

        drop(reader);
        drop(socket);
        join_handle.join().unwrap();
    }
}
