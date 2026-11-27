//! End-to-end tests: a real server on a random port, real WebSocket clients.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use porch_server::{serve, Config, Limits};
use serde_json::{json, Value};
use tokio::{net::TcpListener, time::timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

const PASSWORD: &str = "porch-test-password";

struct Server {
    url: String,
    _dir: tempfile::TempDir,
}

fn fast_limits() -> Limits {
    Limits {
        send_burst: 100_000.0,
        send_per_sec: 100_000.0,
        hash_concurrency: 4,
        ..Limits::default()
    }
}

async fn start(limits: Limits, invite: Option<&str>) -> Server {
    let dir = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let cfg = Config {
        db_path: dir
            .path()
            .join("test.sqlite3")
            .to_string_lossy()
            .into_owned(),
        invite: invite.map(str::to_owned),
        web_dir: None,
        realtime: None,
        limits,
    };
    tokio::spawn(serve(listener, cfg, std::future::pending()));
    Server {
        url: format!("ws://{addr}/ws"),
        _dir: dir,
    }
}

struct Client {
    ws: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    token: String,
}

impl Client {
    async fn connect(server: &Server) -> Self {
        let (ws, _) = connect_async(&server.url).await.unwrap();
        Self {
            ws,
            token: String::new(),
        }
    }

    async fn send(&mut self, v: Value) {
        self.ws.send(Message::text(v.to_string())).await.unwrap();
    }

    /// Next JSON frame, or None if the socket closed or stayed silent.
    async fn next(&mut self) -> Option<Value> {
        loop {
            match timeout(Duration::from_secs(5), self.ws.next()).await {
                Ok(Some(Ok(Message::Text(t)))) => {
                    return Some(serde_json::from_str(t.as_str()).unwrap())
                }
                Ok(Some(Ok(Message::Close(_)))) | Ok(None) | Ok(Some(Err(_))) | Err(_) => {
                    return None
                }
                Ok(Some(Ok(_))) => continue,
            }
        }
    }

    /// Skip frames until one of type `kind` (or an error) arrives.
    async fn until(&mut self, kind: &str) -> Value {
        loop {
            let v = self
                .next()
                .await
                .unwrap_or_else(|| panic!("closed waiting for {kind}"));
            if v["type"] == kind || v["type"] == "error" {
                return v;
            }
        }
    }

    async fn expect(&mut self, kind: &str) -> Value {
        let v = self.until(kind).await;
        assert_eq!(v["type"], kind, "expected {kind}, got {v}");
        v
    }

    /// Prove nothing of `kind` is queued: a ping's pong must come first.
    async fn assert_nothing(&mut self, kind: &str) {
        self.send(json!({"type": "ping"})).await;
        loop {
            let v = self.next().await.expect("closed");
            assert_ne!(v["type"], kind, "unexpected {v}");
            if v["type"] == "pong" {
                return;
            }
        }
    }

    async fn register(server: &Server, name: &str) -> Self {
        let mut c = Self::connect(server).await;
        c.send(json!({"type": "register", "username": name, "password": PASSWORD, "invite": null}))
            .await;
        let w = c.expect("welcome").await;
        c.token = w["token"].as_str().unwrap().to_owned();
        c
    }

    async fn login(server: &Server, name: &str, password: &str) -> (Self, Value) {
        let mut c = Self::connect(server).await;
        c.send(json!({"type": "login", "username": name, "password": password}))
            .await;
        let v = c.until("welcome").await;
        (c, v)
    }
}

#[tokio::test]
async fn accounts_tokens_and_logout() {
    let s = start(fast_limits(), None).await;
    let alice = Client::register(&s, "Alice").await;

    let (_, w) = Client::login(&s, "alice", PASSWORD).await;
    assert_eq!(w["type"], "welcome");
    assert_eq!(w["user"], "alice");
    assert_eq!(w["channels"], json!(["general"]));

    let (_, bad) = Client::login(&s, "alice", "not the password").await;
    assert_eq!(bad["code"], "auth_failed");
    let (_, nobody) = Client::login(&s, "nobody", PASSWORD).await;
    assert_eq!(
        nobody["message"], bad["message"],
        "unknown users look like wrong passwords"
    );

    let mut dup = Client::connect(&s).await;
    dup.send(
        json!({"type": "register", "username": "alice", "password": PASSWORD, "invite": null}),
    )
    .await;
    assert_eq!(dup.until("welcome").await["code"], "taken");

    // A token resumes, then stops working after logout.
    let mut again = Client::connect(&s).await;
    again
        .send(json!({"type": "resume", "token": alice.token}))
        .await;
    assert_eq!(again.expect("welcome").await["user"], "alice");
    again.send(json!({"type": "logout"})).await;
    assert!(again.next().await.is_none(), "logout closes the socket");

    let mut stale = Client::connect(&s).await;
    stale
        .send(json!({"type": "resume", "token": alice.token}))
        .await;
    assert_eq!(stale.until("welcome").await["code"], "token_invalid");

    let mut anon = Client::connect(&s).await;
    anon.send(json!({"type": "send", "to": "#general", "text": "hi", "nonce": null}))
        .await;
    assert_eq!(anon.until("message").await["code"], "not_signed_in");
}

#[tokio::test]
async fn channel_messages_reach_everyone_with_nonce() {
    let s = start(fast_limits(), None).await;
    let mut alice = Client::register(&s, "alice").await;
    let mut bob = Client::register(&s, "bob").await;
    alice.until("user_joined").await;

    alice
        .send(
            json!({"type": "send", "to": "#general", "text": "  hi\u{1b}[2J bob  ", "nonce": "n1"}),
        )
        .await;
    let mine = alice.expect("message").await;
    let theirs = bob.expect("message").await;
    assert_eq!(mine["nonce"], "n1");
    assert_eq!(mine["message"], theirs["message"]);
    assert_eq!(
        theirs["message"]["text"], "hi[2J bob",
        "control chars stripped, trimmed"
    );
    assert_eq!(theirs["message"]["author"], "alice");
    assert_eq!(theirs["message"]["to"], "#general");

    alice
        .send(json!({"type": "create_channel", "name": "#Movies"}))
        .await;
    assert_eq!(bob.expect("channel_created").await["name"], "movies");
    bob.send(json!({"type": "send", "to": "#nope", "text": "x", "nonce": null}))
        .await;
    assert_eq!(bob.until("message").await["code"], "not_found");

    alice
        .send(json!({"type": "send", "to": "#general", "text": "x".repeat(2001), "nonce": null}))
        .await;
    assert_eq!(alice.until("message").await["code"], "bad_request");
}

#[tokio::test]
async fn dms_stay_between_two_people() {
    let s = start(fast_limits(), None).await;
    let mut alice = Client::register(&s, "alice").await;
    let mut bob = Client::register(&s, "bob").await;
    let mut carol = Client::register(&s, "carol").await;

    alice
        .send(json!({"type": "send", "to": "@bob", "text": "psst", "nonce": null}))
        .await;
    let got = bob.expect("message").await;
    assert_eq!(got["message"]["to"], "@bob");
    assert_eq!(got["message"]["author"], "alice");
    alice.expect("message").await;
    carol.assert_nothing("message").await;

    alice.send(json!({"type": "typing", "to": "@bob"})).await;
    let typing = bob.expect("typing").await;
    assert_eq!(typing["to"], "@alice", "addressed from bob's side");
    carol.assert_nothing("typing").await;

    // Bob's history and unread state, from his side of the conversation.
    bob.send(json!({"type": "history", "to": "@alice", "before": null}))
        .await;
    let h = bob.expect("history").await;
    assert_eq!(h["messages"][0]["text"], "psst");
    let (_, w) = Client::login(&s, "bob", PASSWORD).await;
    let id = got["message"]["id"].clone();
    assert_eq!(
        w["convs"],
        json!([{"to": "@alice", "last_id": id, "last_read": 0}])
    );
    let (_, w) = Client::login(&s, "carol", PASSWORD).await;
    assert_eq!(w["convs"], json!([]));

    // Reading on one device tells the user's other devices.
    let (mut bob2, _) = Client::login(&s, "bob", PASSWORD).await;
    bob.send(json!({"type": "mark_read", "to": "@alice", "id": id}))
        .await;
    assert_eq!(
        bob2.expect("read").await,
        json!({"type": "read", "to": "@alice", "id": id})
    );

    alice
        .send(json!({"type": "send", "to": "@alice", "text": "me", "nonce": null}))
        .await;
    assert_eq!(alice.until("message").await["code"], "not_found");
}

#[tokio::test]
async fn history_pages() {
    let s = start(fast_limits(), None).await;
    let mut alice = Client::register(&s, "alice").await;
    for i in 0..60 {
        alice
            .send(json!({"type": "send", "to": "#general", "text": format!("m{i}"), "nonce": null}))
            .await;
        alice.expect("message").await;
    }
    alice
        .send(json!({"type": "history", "to": "#general", "before": null}))
        .await;
    let page = alice.expect("history").await;
    let msgs = page["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 50);
    assert_eq!(page["has_more"], true);
    assert_eq!(msgs[49]["text"], "m59");
    alice
        .send(json!({"type": "history", "to": "#general", "before": msgs[0]["id"]}))
        .await;
    let older = alice.expect("history").await;
    assert_eq!(older["messages"].as_array().unwrap().len(), 10);
    assert_eq!(older["has_more"], false);
}

#[tokio::test]
async fn only_authors_edit_and_delete() {
    let s = start(fast_limits(), None).await;
    let mut alice = Client::register(&s, "alice").await;
    let mut bob = Client::register(&s, "bob").await;
    alice
        .send(json!({"type": "send", "to": "#general", "text": "frist", "nonce": null}))
        .await;
    let id = alice.expect("message").await["message"]["id"].clone();
    bob.expect("message").await;

    bob.send(json!({"type": "edit", "id": id, "text": "lol"}))
        .await;
    assert_eq!(bob.until("edited").await["code"], "forbidden");
    alice
        .send(json!({"type": "edit", "id": id, "text": "first"}))
        .await;
    let e = bob.expect("edited").await;
    assert_eq!(e["message"]["text"], "first");
    assert!(e["message"]["edited_at"].is_number());

    bob.send(json!({"type": "delete", "id": id})).await;
    assert_eq!(bob.until("deleted").await["code"], "forbidden");
    alice.send(json!({"type": "delete", "id": id})).await;
    assert_eq!(
        bob.expect("deleted").await,
        json!({"type": "deleted", "id": id, "to": "#general", "author": "alice"})
    );
}

#[tokio::test]
async fn presence_follows_connections() {
    let s = start(fast_limits(), None).await;
    let mut alice = Client::register(&s, "alice").await;
    let bob = Client::register(&s, "bob").await;
    assert_eq!(
        alice.expect("presence").await,
        json!({"type": "presence", "user": "bob", "online": true})
    );
    let (_, w) = Client::login(&s, "carol", PASSWORD).await;
    assert_eq!(w["code"], "auth_failed");
    drop(bob);
    assert_eq!(
        alice.expect("presence").await,
        json!({"type": "presence", "user": "bob", "online": false})
    );
}

#[tokio::test]
async fn wrong_passwords_lock_the_name_across_connections() {
    let s = start(fast_limits(), None).await;
    Client::register(&s, "alice").await;
    for _ in 0..5 {
        let (_, v) = Client::login(&s, "alice", "guessing-wrong").await;
        assert_eq!(v["code"], "auth_failed");
    }
    let (_, v) = Client::login(&s, "alice", PASSWORD).await;
    assert_eq!(v["code"], "rate_limited", "even the right password waits");

    // One connection that keeps failing gets closed.
    let mut c = Client::connect(&s).await;
    for _ in 0..5 {
        c.send(json!({"type": "resume", "token": "bogus"})).await;
    }
    for _ in 0..5 {
        if c.next().await.is_none() {
            return;
        }
    }
    assert!(
        c.next().await.is_none(),
        "connection should close after repeated failures"
    );
}

#[tokio::test]
async fn invite_is_required_when_set() {
    let s = start(fast_limits(), Some("front-porch")).await;
    let mut c = Client::connect(&s).await;
    c.send(
        json!({"type": "register", "username": "alice", "password": PASSWORD, "invite": "nope"}),
    )
    .await;
    assert_eq!(c.until("welcome").await["code"], "auth_failed");
    c.send(json!({"type": "register", "username": "alice", "password": PASSWORD, "invite": " front-porch "}))
        .await;
    assert_eq!(c.expect("welcome").await["user"], "alice");
}

#[tokio::test]
async fn send_rate_is_limited() {
    let s = start(
        Limits {
            send_burst: 3.0,
            send_per_sec: 0.01,
            ..fast_limits()
        },
        None,
    )
    .await;
    let mut alice = Client::register(&s, "alice").await;
    for _ in 0..3 {
        alice
            .send(json!({"type": "send", "to": "#general", "text": "x", "nonce": null}))
            .await;
        alice.expect("message").await;
    }
    alice
        .send(json!({"type": "send", "to": "#general", "text": "x", "nonce": null}))
        .await;
    assert_eq!(alice.until("message").await["code"], "rate_limited");
}

#[tokio::test]
async fn a_stalled_client_is_dropped_without_freezing_anyone() {
    let s = start(
        Limits {
            queue: 8,
            // Out of reach, so only the full queue can drop the stalled client,
            // however fast or slow the machine is.
            write_timeout: Duration::from_secs(600),
            ..fast_limits()
        },
        None,
    )
    .await;
    let mut alice = Client::register(&s, "alice").await;
    let mut bob = Client::register(&s, "bob").await;
    let _stalled = Client::register(&s, "stalled").await; // never reads again
    let big = "y".repeat(2000);

    let mut dropped = false;
    for i in 0..20_000 {
        alice
            .send(json!({"type": "send", "to": "#general", "text": big, "nonce": null}))
            .await;
        // Both readers keep getting every message in order; nothing blocks.
        loop {
            let v = alice.next().await.expect("alice lost her connection");
            if v["type"] == "presence" && v["user"] == "stalled" && v["online"] == false {
                dropped = true;
            }
            if v["type"] == "message" {
                break;
            }
        }
        assert_eq!(
            bob.until("message").await["type"],
            "message",
            "bob stalled at {i}"
        );
        if dropped {
            break;
        }
    }
    assert!(dropped, "the stalled client was never dropped");
}

#[tokio::test]
async fn oversized_frames_close_the_connection() {
    let s = start(fast_limits(), None).await;
    let mut alice = Client::register(&s, "alice").await;
    let huge = "z".repeat(200_000);
    let _ = alice
        .ws
        .send(Message::text(
            json!({"type": "send", "to": "#general", "text": huge, "nonce": null}).to_string(),
        ))
        .await;
    assert!(alice.next().await.is_none());
}

#[tokio::test]
async fn garbage_gets_a_useful_error() {
    let s = start(fast_limits(), None).await;
    let mut c = Client::connect(&s).await;
    c.ws.send(Message::text("{not json")).await.unwrap();
    let v = c.next().await.unwrap();
    assert_eq!(v["code"], "bad_request");
    c.send(json!({"type": "ping"})).await;
    assert_eq!(c.next().await.unwrap()["type"], "pong");
}

// ---- voice ----------------------------------------------------------------

/// A stand-in for Cloudflare Realtime that answers like the real API and
/// checks the bearer token, so the proxy logic can be tested offline.
async fn fake_realtime() -> String {
    use axum::{
        extract::Path,
        http::{HeaderMap, StatusCode},
        routing::{post, put},
        Json, Router,
    };
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);
    fn authed(h: &HeaderMap) -> bool {
        h.get("authorization").and_then(|v| v.to_str().ok()) == Some("Bearer test-token")
    }
    let app = Router::new()
        .route(
            "/v1/apps/test-app/sessions/new",
            post(|h: HeaderMap| async move {
                if !authed(&h) {
                    return (StatusCode::UNAUTHORIZED, Json(json!({})));
                }
                let n = NEXT.fetch_add(1, Ordering::SeqCst);
                (
                    StatusCode::CREATED,
                    Json(json!({ "sessionId": format!("s{n}") })),
                )
            }),
        )
        .route(
            "/v1/apps/test-app/sessions/{sid}/tracks/new",
            post(
                |Path(sid): Path<String>, h: HeaderMap, Json(body): Json<Value>| async move {
                    if !authed(&h) {
                        return (StatusCode::UNAUTHORIZED, Json(json!({})));
                    }
                    let tracks = body["tracks"].as_array().cloned().unwrap_or_default();
                    if body.get("sessionDescription").is_some() {
                        let reply = json!({
                            "sessionDescription": {"type": "answer", "sdp": format!("answer:{sid}")},
                            "tracks": tracks,
                        });
                        return (StatusCode::OK, Json(reply));
                    }
                    let pulled: Vec<Value> = tracks
                        .iter()
                        .enumerate()
                        .map(|(i, t)| {
                            json!({"sessionId": t["sessionId"], "trackName": "mic", "mid": i.to_string()})
                        })
                        .collect();
                    let reply = json!({
                        "requiresImmediateRenegotiation": true,
                        "sessionDescription": {"type": "offer", "sdp": format!("offer:{sid}")},
                        "tracks": pulled,
                    });
                    (StatusCode::OK, Json(reply))
                },
            ),
        )
        .route(
            "/v1/apps/test-app/sessions/{sid}/renegotiate",
            put(|Json(body): Json<Value>| async move {
                assert_eq!(body["sessionDescription"]["type"], "answer");
                Json(json!({}))
            }),
        );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}/v1")
}

async fn start_voice() -> Server {
    let base_url = fake_realtime().await;
    let dir = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let cfg = Config {
        db_path: dir.path().join("v.sqlite3").to_string_lossy().into_owned(),
        invite: None,
        web_dir: None,
        realtime: Some(porch_server::voice::RealtimeConfig {
            base_url,
            app_id: "test-app".into(),
            app_token: "test-token".into(),
        }),
        limits: fast_limits(),
    };
    tokio::spawn(serve(listener, cfg, std::future::pending()));
    Server {
        url: format!("ws://{addr}/ws"),
        _dir: dir,
    }
}

impl Client {
    async fn voice(&mut self, rid: u32, mut req: Value) -> Value {
        req["rid"] = json!(rid);
        self.send(req).await;
        loop {
            let v = self.until("voice_reply").await;
            if v["rid"] == rid {
                return v;
            }
        }
    }
}

#[tokio::test]
async fn voice_is_off_without_realtime() {
    let s = start(fast_limits(), None).await;
    let mut alice = Client::connect(&s).await;
    alice
        .send(
            json!({"type": "register", "username": "alice", "password": PASSWORD, "invite": null}),
        )
        .await;
    let w = alice.expect("welcome").await;
    assert_eq!(w["voice_enabled"], false);
    let r = alice
        .voice(1, json!({"type": "voice_join", "room": "#general"}))
        .await;
    assert_eq!(r["error"], "Voice isn't set up on this server.");
}

#[tokio::test]
async fn voice_join_publish_pull_and_leave() {
    let s = start_voice().await;
    let mut alice = Client::register(&s, "alice").await;
    let mut bob = Client::register(&s, "bob").await;

    // DMs aren't rooms, and you can't publish before joining.
    let r = alice
        .voice(1, json!({"type": "voice_join", "room": "@bob"}))
        .await;
    assert_eq!(r["error"], "Voice rooms belong to channels.");
    let r = alice
        .voice(
            2,
            json!({"type": "voice_publish", "sdp": "v=0", "mid": "0"}),
        )
        .await;
    assert_eq!(r["error"], "Join a voice room first.");

    let r = alice
        .voice(3, json!({"type": "voice_join", "room": "#general"}))
        .await;
    assert!(r["error"].is_null(), "{r}");
    let st = bob.expect("voice_state").await;
    assert_eq!(
        st["member"],
        json!({"user": "alice", "room": "#general", "live": false, "muted": false})
    );

    let r = alice
        .voice(
            4,
            json!({"type": "voice_publish", "sdp": "v=0 offer", "mid": "0"}),
        )
        .await;
    assert!(r["sdp"].as_str().unwrap().starts_with("answer:"), "{r}");
    let st = bob.expect("voice_state").await;
    assert_eq!(st["member"]["live"], true);

    // Bob joins and pulls Alice; himself and strangers are skipped.
    bob.voice(1, json!({"type": "voice_join", "room": "#general"}))
        .await;
    let r = bob
        .voice(
            2,
            json!({"type": "voice_pull", "users": ["alice", "bob", "nobody"]}),
        )
        .await;
    assert!(r["sdp"].as_str().unwrap().starts_with("offer:"), "{r}");
    assert_eq!(r["tracks"], json!([{"user": "alice", "mid": "0"}]));
    let r = bob
        .voice(3, json!({"type": "voice_answer", "sdp": "v=0 answer"}))
        .await;
    assert!(r["error"].is_null(), "{r}");

    // A late arrival sees the roster in its welcome.
    let mut carol = Client::register(&s, "carol").await;
    let (_, w) = Client::login(&s, "carol", PASSWORD).await;
    assert_eq!(w["voice_enabled"], true);
    assert_eq!(w["voice"].as_array().unwrap().len(), 2);

    // Muting is broadcast; leaving, and dropping the socket, both free the seat.
    alice
        .send(json!({"type": "voice_mute", "muted": true}))
        .await;
    let st = carol.expect("voice_state").await;
    assert_eq!(st["member"]["muted"], true);
    bob.send(json!({"type": "voice_leave"})).await;
    assert_eq!(carol.expect("voice_left").await["user"], "bob");
    drop(alice);
    assert_eq!(carol.expect("voice_left").await["user"], "alice");
}

#[tokio::test]
async fn voice_seat_moves_to_the_newest_device() {
    let s = start_voice().await;
    let mut phone = Client::register(&s, "alice").await;
    phone
        .voice(1, json!({"type": "voice_join", "room": "#general"}))
        .await;
    let (mut desk, _) = Client::login(&s, "alice", PASSWORD).await;
    desk.voice(1, json!({"type": "voice_join", "room": "#general"}))
        .await;
    assert_eq!(
        phone.expect("voice_ended").await["reason"],
        "You joined voice on another device."
    );
    // The old device can no longer act on the seat, and leaving from it is a no-op.
    let r = phone
        .voice(2, json!({"type": "voice_answer", "sdp": "v=0"}))
        .await;
    assert_eq!(r["error"], "Join a voice room first.");
    phone.send(json!({"type": "voice_leave"})).await;
    desk.assert_nothing("voice_left").await;
}
