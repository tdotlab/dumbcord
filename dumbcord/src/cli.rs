use std::{
    io::{self, Write},
    sync::{mpsc::Sender, Arc, Mutex},
    thread,
    time::Duration,
};

use anyhow::{anyhow, bail, Result};

use crate::{
    client,
    protocol::{ChatMessage, ClientMessage, ServerMessage},
};

#[derive(Clone)]
enum View {
    Channel(String),
    Direct(String),
}

impl View {
    fn label(&self) -> String {
        match self {
            Self::Channel(channel) => format!("#{channel}"),
            Self::Direct(peer) => format!("@{peer}"),
        }
    }
}

fn input(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut buffer = String::new();
    if io::stdin().read_line(&mut buffer)? == 0 {
        bail!("end of input");
    }
    Ok(buffer.trim().to_owned())
}

fn timestamp(seconds: i64) -> String {
    let seconds = seconds.rem_euclid(86_400);
    format!("{:02}:{:02} UTC", seconds / 3600, seconds % 3600 / 60)
}

fn print_message(message: &ChatMessage) {
    println!(
        "[{}] {}: {}",
        timestamp(message.created_at),
        message.author,
        message.text
    );
}

fn help() {
    println!(
        "Commands:
  /channels       List channels
  /create NAME    Create and join a channel
  /join NAME      Switch to a channel
  /users          List members and who's online
  /dm NAME        Enter direct-message conversation
  /back           Return to the last channel
  /history        Reload recent messages
  /help           Show these commands
  /quit           Disconnect
Anything else sends a message to your active channel or DM."
    );
}

fn reader_loop(
    incoming: std::sync::mpsc::Receiver<ServerMessage>,
    view: Arc<Mutex<View>>,
    my_name: String,
) {
    while let Ok(event) = incoming.recv() {
        match event {
            ServerMessage::Message { message } => {
                let active = view.lock().ok().map(|v| v.label()).unwrap_or_default();
                let room = message.channel.as_deref().unwrap_or("");
                if active == format!("#{room}") {
                    print_message(&message);
                } else {
                    println!("* New message in #{room} from {}", message.author);
                }
            }
            ServerMessage::DirectMessage { message } => {
                let peer = if message.author == my_name {
                    message.recipient.as_deref().unwrap_or("")
                } else {
                    &message.author
                };
                let active = view.lock().ok().map(|v| v.label()).unwrap_or_default();
                if active == format!("@{peer}") {
                    print_message(&message);
                } else {
                    println!("* Direct message from {}", message.author);
                }
            }
            ServerMessage::History { channel, messages } => {
                let active = view.lock().ok().map(|v| v.label()).unwrap_or_default();
                if active == format!("#{channel}") {
                    println!("\n--- #{channel} ({} recent messages) ---", messages.len());
                    for message in &messages {
                        print_message(message);
                    }
                }
            }
            ServerMessage::DirectHistory { peer, messages } => {
                let active = view.lock().ok().map(|v| v.label()).unwrap_or_default();
                if active == format!("@{peer}") {
                    println!("\n--- DM with {peer} ---");
                    for message in &messages {
                        print_message(message);
                    }
                }
            }
            ServerMessage::Channels { channels } => {
                println!("Channels: {}", channels.iter().map(|ch| format!("#{ch}")).collect::<Vec<_>>().join(", "));
            }
            ServerMessage::Users { members, online } => {
                println!(
                    "Members: {}",
                    members
                        .iter()
                        .map(|user| {
                            if online.contains(user) {
                                format!("{user} (online)")
                            } else {
                                user.to_string()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            ServerMessage::Error { message } => eprintln!("! {message}"),
            ServerMessage::Typing { channel, username } => {
                let active = view.lock().ok().map(|v| v.label()).unwrap_or_default();
                if active == format!("#{channel}") {
                    println!("* {username} is typing...");
                }
            }
            ServerMessage::Disconnected { message } => {
                eprintln!("! {message}");
                break;
            }
            ServerMessage::Authenticated { .. } | ServerMessage::Pong => {}
        }
    }
}

fn send(outgoing: &Sender<ClientMessage>, request: ClientMessage) -> Result<()> {
    outgoing.send(request).map_err(|_| anyhow!("connection closed"))
}

pub fn run(address: &str, preset_user: Option<String>) -> Result<()> {
    println!("Dumbcord CLI — {address}");
    println!("Use a private LAN or VPN; this transport is not encrypted.");
    let connection = client::connect(address)?;

    let create = input("Create account? [y/N]: ")?.eq_ignore_ascii_case("y");
    let username = match preset_user {
        Some(name) => name,
        None => input("Username: ")?,
    };
    let password = rpassword::prompt_password("Password: ")?;

    let auth = if create {
        let invite = input("Invite code (blank if not required): ")?;
        ClientMessage::Register {
            username,
            password,
            invite: Some(invite),
        }
    } else {
        ClientMessage::Login { username, password }
    };
    send(&connection.outgoing, auth)?;

    let me = match connection.incoming.recv_timeout(Duration::from_secs(15))? {
        ServerMessage::Authenticated { username } => username,
        ServerMessage::Error { message } => bail!("authentication failed: {message}"),
        event => bail!("unexpected authentication response: {event:?}"),
    };

    println!("Signed in as {me}. Type /help for commands.");
    let view = Arc::new(Mutex::new(View::Channel("general".to_owned())));
    let last_channel = "general".to_owned();
    let incoming = connection.incoming;
    let outgoing = connection.outgoing;
    let printer_view = Arc::clone(&view);
    thread::spawn(move || reader_loop(incoming, printer_view, me));

    let mut last_channel = last_channel;
    loop {
        let label = view.lock().map_err(|_| anyhow!("view lock poisoned"))?.label();
        let line = match input(&format!("{label}> ")) {
            Ok(value) => value,
            Err(_) => break,
        };
        if line.is_empty() {
            continue;
        }
        if !line.starts_with('/') {
            let selected = view.lock().map_err(|_| anyhow!("view lock poisoned"))?.clone();
            match selected {
                View::Channel(channel) => send(&outgoing, ClientMessage::Send { channel, text: line })?,
                View::Direct(to) => send(&outgoing, ClientMessage::DirectMessage { to, text: line })?,
            }
            continue;
        }
        let (command, arg) = line.split_once(' ').unwrap_or((&line, ""));
        let arg = arg.trim();
        match command {
            "/help" => help(),
            "/quit" | "/exit" | "/q" => break,
            "/channels" => send(&outgoing, ClientMessage::Channels)?,
            "/users" => send(&outgoing, ClientMessage::Users)?,
            "/create" if !arg.is_empty() => {
                send(&outgoing, ClientMessage::CreateChannel { name: arg.to_owned() })?;
                let channel = crate::protocol::channel(arg)?;
                last_channel.clone_from(&channel);
                *view.lock().map_err(|_| anyhow!("view lock poisoned"))? = View::Channel(channel.clone());
                send(&outgoing, ClientMessage::Join { channel })?;
            }
            "/join" if !arg.is_empty() => {
                let channel = crate::protocol::channel(arg)?;
                last_channel.clone_from(&channel);
                *view.lock().map_err(|_| anyhow!("view lock poisoned"))? = View::Channel(channel.clone());
                send(&outgoing, ClientMessage::Join { channel })?;
            }
            "/dm" if !arg.is_empty() => {
                let peer = crate::protocol::username(arg)?;
                *view.lock().map_err(|_| anyhow!("view lock poisoned"))? = View::Direct(peer.clone());
                send(&outgoing, ClientMessage::DirectHistory { peer, before: None })?;
            }
            "/back" => {
                *view.lock().map_err(|_| anyhow!("view lock poisoned"))? = View::Channel(last_channel.clone());
                send(&outgoing, ClientMessage::Join { channel: last_channel.clone() })?;
            }
            "/history" => {
                let selected = view.lock().map_err(|_| anyhow!("view lock poisoned"))?.clone();
                match selected {
                    View::Channel(channel) => send(&outgoing, ClientMessage::History { channel, before: None })?,
                    View::Direct(peer) => send(&outgoing, ClientMessage::DirectHistory { peer, before: None })?,
                }
            }
            _ => println!("Unknown or incomplete command. Type /help."),
        }
    }
    println!("Goodbye.");
    Ok(())
}
