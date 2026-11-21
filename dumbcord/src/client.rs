use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{Shutdown, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use anyhow::{Context, Result};

use crate::protocol::{ClientMessage, ServerMessage, MAX_FRAME_BYTES};

/// Client I/O runs entirely on worker threads. GUI and terminal code only use
/// these channels, so incoming chat never blocks on a user input operation.
pub struct Connection {
    pub outgoing: Sender<ClientMessage>,
    pub incoming: Receiver<ServerMessage>,
}

pub fn connect(address: &str) -> Result<Connection> {
    let socket = TcpStream::connect(address)
        .with_context(|| format!("connect to Dumbcord at {address}"))?;
    socket.set_nodelay(true)?;

    let mut write_socket = socket.try_clone()?;
    let (outgoing, requests) = mpsc::channel::<ClientMessage>();
    let (events, incoming) = mpsc::channel::<ServerMessage>();
    let writer_events = events.clone();

    thread::spawn(move || {
        while let Ok(request) = requests.recv() {
            let sent = serde_json::to_writer(&mut write_socket, &request)
                .and_then(|_| {
                    write_socket
                        .write_all(b"\n")
                        .map_err(serde_json::Error::io)
                });
            if let Err(error) = sent {
                let _ = writer_events.send(ServerMessage::Error {
                    message: format!("could not send: {error}"),
                });
                break;
            }
        }
        let _ = write_socket.shutdown(Shutdown::Both);
    });

    thread::spawn(move || {
        let mut reader = BufReader::new(socket);
        loop {
            let mut frame = Vec::new();
            let outcome = {
                let mut bounded = (&mut reader).take((MAX_FRAME_BYTES * 32 + 1) as u64);
                bounded.read_until(b'\n', &mut frame)
            };
            match outcome {
                Ok(0) => break,
                Ok(bytes) if bytes > MAX_FRAME_BYTES * 32 || !frame.ends_with(b"\n") => {
                    let _ = events.send(ServerMessage::Error {
                        message: "incoming frame exceeds limit".into(),
                    });
                    break;
                }
                Ok(_) => match serde_json::from_slice::<ServerMessage>(&frame) {
                    Ok(event) => {
                        if events.send(event).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = events.send(ServerMessage::Error {
                            message: format!("unrecognized server response: {error}"),
                        });
                    }
                },
                Err(error) => {
                    let _ = events.send(ServerMessage::Error {
                        message: format!("connection read error: {error}"),
                    });
                    break;
                }
            }
        }
        let _ = events.send(ServerMessage::Disconnected {
            message: "disconnected from server".to_owned(),
        });
    });

    Ok(Connection { outgoing, incoming })
}
