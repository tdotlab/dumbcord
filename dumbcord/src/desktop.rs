use std::{
    collections::{HashMap, HashSet},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use eframe::egui;

use crate::{
    client::{self, Connection},
    protocol::{self, ChatMessage, ClientMessage, ServerMessage},
};

#[derive(Clone, PartialEq, Eq)]
enum View {
    Channel(String),
    Direct(String),
}

impl View {
    fn label(&self) -> String {
        match self {
            Self::Channel(name) => format!("#{name}"),
            Self::Direct(name) => format!("@{name}"),
        }
    }
}

struct Desktop {
    server: String,
    username: String,
    password: String,
    invite: String,
    register: bool,
    connecting: Option<Receiver<Result<Connection, String>>>,
    connection: Option<Connection>,
    authenticated: bool,
    me: String,
    status: String,
    channels: Vec<String>,
    members: Vec<String>,
    online: HashSet<String>,
    channels_history: HashMap<String, Vec<ChatMessage>>,
    direct_history: HashMap<String, Vec<ChatMessage>>,
    unread: HashSet<String>,
    paging: HashSet<String>,
    typing: HashMap<String, Instant>,
    last_typing_sent: Instant,
    view: View,
    draft: String,
    new_channel: String,
}

impl Desktop {
    fn new(server: String) -> Self {
        Self {
            server,
            username: String::new(),
            password: String::new(),
            invite: String::new(),
            register: false,
            connecting: None,
            connection: None,
            authenticated: false,
            me: String::new(),
            status: "Connect to your friends' server.".into(),
            channels: Vec::new(),
            members: Vec::new(),
            online: HashSet::new(),
            channels_history: HashMap::new(),
            direct_history: HashMap::new(),
            unread: HashSet::new(),
            paging: HashSet::new(),
            typing: HashMap::new(),
            last_typing_sent: Instant::now() - Duration::from_secs(5),
            view: View::Channel("general".into()),
            draft: String::new(),
            new_channel: String::new(),
        }
    }

    fn begin_connect(&mut self) {
        if self.connecting.is_some() {
            return;
        }
        self.connection = None;
        self.authenticated = false;
        self.status = format!("Connecting to {}...", self.server.trim());
        let address = self.server.trim().to_owned();
        let (tx, rx) = mpsc::channel();
        self.connecting = Some(rx);
        thread::spawn(move || {
            let result = client::connect(&address).map_err(|error| error.to_string());
            let _ = tx.send(result);
        });
    }

    fn send(&mut self, request: ClientMessage) {
        match &self.connection {
            Some(connection) => {
                if connection.outgoing.send(request).is_err() {
                    self.status = "Connection closed. Reconnect to continue.".into();
                }
            }
            None => self.status = "Not connected.".into(),
        }
    }

    fn poll(&mut self) {
        let connection_result = self.connecting.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Disconnected) => {
                Some(Err("connection worker terminated unexpectedly".into()))
            }
            Err(TryRecvError::Empty) => None,
        });

        if let Some(result) = connection_result {
            self.connecting = None;
            match result {
                Ok(connection) => {
                    self.connection = Some(connection);
                    let request = if self.register {
                        ClientMessage::Register {
                            username: self.username.clone(),
                            password: self.password.clone(),
                            invite: Some(self.invite.clone()),
                        }
                    } else {
                        ClientMessage::Login {
                            username: self.username.clone(),
                            password: self.password.clone(),
                        }
                    };
                    self.send(request);
                }
                Err(error) => self.status = format!("Could not connect: {error}"),
            }
        }

        let mut events = Vec::new();
        if let Some(connection) = &self.connection {
            while let Ok(event) = connection.incoming.try_recv() {
                events.push(event);
            }
        }
        for event in events {
            self.handle_event(event);
        }
        self.typing.retain(|_, since| since.elapsed() < Duration::from_secs(4));
    }

    fn handle_event(&mut self, event: ServerMessage) {
        match event {
            ServerMessage::Authenticated { username } => {
                self.authenticated = true;
                self.me = username;
                self.password.clear();
                self.view = View::Channel("general".into());
                self.status = format!("Connected to {} as {}", self.server, self.me);
            }
            ServerMessage::Channels { channels } => self.channels = channels,
            ServerMessage::Users { members, online } => {
                self.members = members;
                self.online = online.into_iter().collect();
            }
            ServerMessage::History { channel, messages } => {
                let key = format!("#{channel}");
                if self.paging.remove(&key) {
                    let buffer = self.channels_history.entry(channel).or_default();
                    let existing: HashSet<i64> = buffer.iter().map(|message| message.id).collect();
                    let mut earlier: Vec<ChatMessage> = messages
                        .into_iter()
                        .filter(|message| !existing.contains(&message.id))
                        .collect();
                    earlier.append(buffer);
                    *buffer = earlier;
                } else {
                    self.channels_history.insert(channel, messages);
                }
            }
            ServerMessage::DirectHistory { peer, messages } => {
                let key = format!("@{peer}");
                if self.paging.remove(&key) {
                    let buffer = self.direct_history.entry(peer).or_default();
                    let existing: HashSet<i64> = buffer.iter().map(|message| message.id).collect();
                    let mut earlier: Vec<ChatMessage> = messages
                        .into_iter()
                        .filter(|message| !existing.contains(&message.id))
                        .collect();
                    earlier.append(buffer);
                    *buffer = earlier;
                } else {
                    self.direct_history.insert(peer, messages);
                }
            }
            ServerMessage::Message { message } => {
                if let Some(channel) = message.channel.clone() {
                    let buffer = self.channels_history.entry(channel.clone()).or_default();
                    if !buffer.iter().any(|prior| prior.id == message.id) {
                        buffer.push(message);
                    }
                    if self.view != View::Channel(channel.clone()) {
                        self.unread.insert(format!("#{channel}"));
                    }
                }
            }
            ServerMessage::DirectMessage { message } => {
                let peer = if message.author == self.me {
                    message.recipient.clone().unwrap_or_default()
                } else {
                    message.author.clone()
                };
                let buffer = self.direct_history.entry(peer.clone()).or_default();
                if !buffer.iter().any(|prior| prior.id == message.id) {
                    buffer.push(message);
                }
                if self.view != View::Direct(peer.clone()) {
                    self.unread.insert(format!("@{peer}"));
                }
            }
            ServerMessage::Typing { channel, username } => {
                if username != self.me && self.view == View::Channel(channel) {
                    self.typing.insert(username, Instant::now());
                }
            }
            ServerMessage::Error { message } => self.status = format!("Error: {message}"),
            ServerMessage::Disconnected { message } => {
                self.connection = None;
                self.authenticated = false;
                self.status = message;
            }
            ServerMessage::Pong => {}
        }
    }

    fn open_channel(&mut self, name: String) {
        self.view = View::Channel(name.clone());
        self.unread.remove(&format!("#{name}"));
        self.typing.clear();
        self.send(ClientMessage::Join { channel: name });
    }

    fn open_dm(&mut self, peer: String) {
        if peer == self.me {
            return;
        }
        self.view = View::Direct(peer.clone());
        self.unread.remove(&format!("@{peer}"));
        self.typing.clear();
        self.send(ClientMessage::DirectHistory { peer, before: None });
    }

    fn create_channel(&mut self) {
        let name = match protocol::channel(&self.new_channel) {
            Ok(name) => name,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };
        self.new_channel.clear();
        self.send(ClientMessage::CreateChannel { name: name.clone() });
        self.open_channel(name);
    }

    fn send_message(&mut self) {
        let text = self.draft.trim().to_owned();
        if text.is_empty() {
            return;
        }
        let request = match &self.view {
            View::Channel(channel) => ClientMessage::Send {
                channel: channel.clone(),
                text,
            },
            View::Direct(to) => ClientMessage::DirectMessage {
                to: to.clone(),
                text,
            },
        };
        self.send(request);
        self.draft.clear();
    }

    fn load_older(&mut self) {
        let id = self.current_messages().first().map(|message| message.id);
        if let Some(before) = id {
            let key = self.view.label();
            self.paging.insert(key);
            match &self.view {
                View::Channel(channel) => self.send(ClientMessage::History {
                    channel: channel.clone(),
                    before: Some(before),
                }),
                View::Direct(peer) => self.send(ClientMessage::DirectHistory {
                    peer: peer.clone(),
                    before: Some(before),
                }),
            }
        }
    }

    fn current_messages(&self) -> &[ChatMessage] {
        match &self.view {
            View::Channel(channel) => self
                .channels_history
                .get(channel)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            View::Direct(peer) => self
                .direct_history
                .get(peer)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        }
    }
}

fn clock(seconds: i64) -> String {
    let seconds = seconds.rem_euclid(86_400);
    format!("{:02}:{:02} UTC", seconds / 3600, seconds % 3600 / 60)
}

impl eframe::App for Desktop {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        egui::TopBottomPanel::top("heading").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("DUMBCORD");
                ui.separator();
                ui.small(&self.status);
            });
        });

        if !self.authenticated {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(55.0);
                    ui.heading("Your friends. Your server.");
                    ui.label("Native Windows + Linux chat, in Rust.");
                    ui.add_space(20.0);
                    ui.set_max_width(420.0);
                    ui.label("Server address");
                    ui.text_edit_singleline(&mut self.server);
                    ui.label("Username");
                    ui.text_edit_singleline(&mut self.username);
                    ui.label("Password (8+ characters)");
                    ui.add(egui::TextEdit::singleline(&mut self.password).password(true));
                    ui.checkbox(&mut self.register, "Create account");
                    if self.register {
                        ui.label("Invite code (if required)");
                        ui.text_edit_singleline(&mut self.invite);
                    }
                    ui.add_space(10.0);
                    if ui
                        .add_enabled(self.connecting.is_none(), egui::Button::new("Connect"))
                        .clicked()
                    {
                        self.begin_connect();
                    }
                    ui.add_space(18.0);
                    ui.small("Use only on a private LAN or VPN; traffic is not encrypted.");
                });
            });
            ctx.request_repaint_after(Duration::from_millis(90));
            return;
        }

        let mut selected_channel = None;
        let mut selected_dm = None;
        let mut create_channel = false;

        egui::SidePanel::left("sidebar")
            .resizable(true)
            .default_width(230.0)
            .show(ctx, |ui| {
                ui.strong(format!("Signed in as {}", self.me));
                ui.add_space(10.0);
                ui.heading("CHANNELS");
                egui::ScrollArea::vertical().max_height(245.0).show(ui, |ui| {
                    for channel in &self.channels {
                        let label = format!("#{channel}");
                        let active = self.view == View::Channel(channel.clone());
                        let unread = self.unread.contains(&label);
                        let shown = if unread { format!("{label}  *") } else { label };
                        if ui.selectable_label(active, shown).clicked() {
                            selected_channel = Some(channel.clone());
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_channel)
                            .hint_text("new-channel")
                            .desired_width(120.0),
                    );
                    if ui.button("+").clicked() {
                        create_channel = true;
                    }
                });
                ui.separator();
                ui.heading("DIRECT MESSAGES");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for member in &self.members {
                        if member == &self.me {
                            continue;
                        }
                        let online = self.online.contains(member);
                        let label = format!("@{member}");
                        let shown = format!(
                            "{}{}",
                            if online { "● " } else { "○ " },
                            if self.unread.contains(&label) {
                                format!("{label} *")
                            } else {
                                label
                            }
                        );
                        if ui
                            .selectable_label(self.view == View::Direct(member.clone()), shown)
                            .clicked()
                        {
                            selected_dm = Some(member.clone());
                        }
                    }
                });
            });

        if let Some(channel) = selected_channel {
            self.open_channel(channel);
        }
        if let Some(peer) = selected_dm {
            self.open_dm(peer);
        }
        if create_channel {
            self.create_channel();
        }

        let mut submit = false;
        let mut draft_changed = false;
        egui::TopBottomPanel::bottom("composer").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.draft)
                        .hint_text(format!("Message {}", self.view.label()))
                        .desired_width(f32::INFINITY),
                );
                draft_changed = response.changed();
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
                if ui.button("Send").clicked() {
                    submit = true;
                }
            });
        });

        if draft_changed
            && !self.draft.is_empty()
            && self.last_typing_sent.elapsed() >= Duration::from_secs(3)
        {
            if let View::Channel(channel) = &self.view {
                self.send(ClientMessage::Typing {
                    channel: channel.clone(),
                });
                self.last_typing_sent = Instant::now();
            }
        }
        if submit {
            self.send_message();
        }

        let messages = self.current_messages().to_vec();
        let title = self.view.label();
        let show_older = messages.len() >= 100;
        let mut load_older = false;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(title);
            ui.separator();
            if show_older && ui.button("Load earlier messages").clicked() {
                load_older = true;
            }
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if messages.is_empty() {
                        ui.weak("No messages yet. Say something.");
                    }
                    for message in &messages {
                        ui.horizontal(|ui| {
                            ui.strong(&message.author);
                            ui.weak(clock(message.created_at));
                        });
                        ui.label(&message.text);
                        ui.add_space(9.0);
                    }
                });
            if !self.typing.is_empty() {
                let names: Vec<_> = self.typing.keys().cloned().collect();
                ui.weak(format!("{} typing...", names.join(", ")));
            }
        });
        if load_older {
            self.load_older();
        }
        ctx.request_repaint_after(Duration::from_millis(90));
    }
}

pub fn run(server: String) -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1080.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Dumbcord",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(Desktop::new(server)))
        }),
    )
    .map_err(|error| anyhow!("desktop launch failed: {error}"))
}
