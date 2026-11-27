//! Voice through Cloudflare Realtime (an SFU).
//!
//! Media never touches this server. Each client in a voice room has one
//! `RTCPeerConnection` to Cloudflare and one Realtime session behind it. The
//! client can't hold the app token, so it asks this server to make the four
//! Realtime calls it needs, and the server checks each one against who is in
//! which room before passing it on.

use std::time::Duration;

use anyhow::{anyhow, Context};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{Error, Result};

/// Where to reach Realtime and with which app.
#[derive(Debug, Clone)]
pub struct RealtimeConfig {
    /// `https://rtc.live.cloudflare.com/v1` in production; tests point it at a fake.
    pub base_url: String,
    pub app_id: String,
    pub app_token: String,
}

impl RealtimeConfig {
    pub const BASE_URL: &'static str = "https://rtc.live.cloudflare.com/v1";
}

/// Every published microphone uses this track name; the session tells them apart.
pub const MIC_TRACK: &str = "mic";

pub struct Realtime {
    http: reqwest::Client,
    apps: String,
    token: String,
}

/// One pulled track: which publisher session it came from and the `mid` it got
/// on the puller's connection.
#[derive(Debug, Clone)]
pub struct Pulled {
    pub session: String,
    pub mid: String,
}

pub struct PullResult {
    /// An SFU offer the client must answer, when there is one.
    pub offer: Option<String>,
    pub tracks: Vec<Pulled>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Sdp {
    sdp: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackResult {
    mid: Option<String>,
    session_id: Option<String>,
    error_code: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TracksResponse {
    session_description: Option<Sdp>,
    #[serde(default)]
    tracks: Vec<TrackResult>,
    #[serde(default)]
    requires_immediate_renegotiation: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewSession {
    session_id: String,
}

fn unavailable() -> Error {
    Error::user(
        crate::protocol::ErrorCode::Internal,
        "Voice couldn't reach Cloudflare. Try again in a moment.",
    )
}

impl Realtime {
    pub fn new(cfg: &RealtimeConfig) -> anyhow::Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()?,
            apps: format!("{}/apps/{}", cfg.base_url.trim_end_matches('/'), cfg.app_id),
            token: cfg.app_token.clone(),
        })
    }

    async fn call(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        let mut req = self
            .http
            .request(method, format!("{}{path}", self.apps))
            .bearer_auth(&self.token);
        if let Some(body) = body {
            req = req.json(&body);
        }
        let resp = req.send().await.map_err(|e| {
            tracing::warn!("realtime request failed: {e}");
            unavailable()
        })?;
        let status = resp.status();
        let value: Value = resp.json().await.unwrap_or(Value::Null);
        if !status.is_success() || value.get("errorCode").is_some_and(|c| !c.is_null()) {
            tracing::warn!(%status, path, body = %value, "realtime refused a request");
            return Err(unavailable());
        }
        Ok(value)
    }

    fn parse<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T> {
        Ok(serde_json::from_value(value).context("unexpected Realtime response")?)
    }

    pub async fn new_session(&self) -> Result<String> {
        let v = self
            .call(reqwest::Method::POST, "/sessions/new", None)
            .await?;
        Ok(Self::parse::<NewSession>(v)?.session_id)
    }

    /// Publish the client's microphone. Returns the SDP answer.
    pub async fn publish(&self, session: &str, offer: &str, mid: &str) -> Result<String> {
        let v = self
            .call(
                reqwest::Method::POST,
                &format!("/sessions/{session}/tracks/new"),
                Some(json!({
                    "sessionDescription": {"type": "offer", "sdp": offer},
                    "tracks": [{"location": "local", "mid": mid, "trackName": MIC_TRACK}],
                })),
            )
            .await?;
        let r: TracksResponse = Self::parse(v)?;
        if r.tracks.iter().any(|t| t.error_code.is_some()) {
            return Err(unavailable());
        }
        r.session_description
            .map(|d| d.sdp)
            .ok_or_else(|| anyhow!("publish returned no answer").into())
    }

    /// Subscribe one session to other sessions' microphones.
    pub async fn pull(&self, session: &str, publishers: &[String]) -> Result<PullResult> {
        let tracks: Vec<Value> = publishers
            .iter()
            .map(|p| json!({"location": "remote", "sessionId": p, "trackName": MIC_TRACK}))
            .collect();
        let v = self
            .call(
                reqwest::Method::POST,
                &format!("/sessions/{session}/tracks/new"),
                Some(json!({ "tracks": tracks })),
            )
            .await?;
        let r: TracksResponse = Self::parse(v)?;
        // Results come back in request order; fall back on that when a result
        // doesn't name its publisher.
        let tracks = r
            .tracks
            .into_iter()
            .zip(publishers)
            .filter(|(t, p)| {
                if let Some(code) = &t.error_code {
                    // Usually the publisher's media isn't flowing yet; the
                    // client tries again shortly.
                    tracing::info!(
                        publisher = %p,
                        code,
                        description = t.error_description.as_deref().unwrap_or(""),
                        "realtime couldn't pull a track"
                    );
                }
                t.error_code.is_none()
            })
            .filter_map(|(t, p)| {
                Some(Pulled {
                    session: t.session_id.unwrap_or_else(|| p.clone()),
                    mid: t.mid?,
                })
            })
            .collect();
        let offer = r
            .session_description
            .filter(|_| r.requires_immediate_renegotiation)
            .map(|d| d.sdp);
        Ok(PullResult { offer, tracks })
    }

    /// Hand the client's answer to an SFU offer back to Realtime.
    pub async fn renegotiate(&self, session: &str, answer: &str) -> Result<()> {
        self.call(
            reqwest::Method::PUT,
            &format!("/sessions/{session}/renegotiate"),
            Some(json!({"sessionDescription": {"type": "answer", "sdp": answer}})),
        )
        .await?;
        Ok(())
    }
}
