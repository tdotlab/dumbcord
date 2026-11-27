// Voice rooms, one per channel, carried by Cloudflare Realtime.
//
// The browser holds one RTCPeerConnection to Cloudflare. It publishes the
// microphone on it and pulls everyone else in the room onto the same
// connection. Porch's server makes the Realtime calls on our behalf (it holds
// the app token), so every negotiation step is a request over the WebSocket,
// answered by a `voice_reply` with the same `rid`.
//
// A member is the server's VoiceMember: { user, room, muted, live }.

import { SvelteSet } from "svelte/reactivity";

const ICE = [{ urls: "stun:stun.cloudflare.com:3478" }];
const REQUEST_TIMEOUT = 15_000;
/** RMS level, 0..1, above which someone counts as speaking. */
const SPEAKING_LEVEL = 0.02;
/** Keep the light on this long after the level drops, so it doesn't flicker. */
const SPEAKING_HOLD = 350;
/** A track can't be pulled until its publisher's media reaches Cloudflare. */
const PULL_RETRIES = [500, 1000, 2000, 4000];

class VoiceError extends Error {}

export class Voice {
  /** The server has a Realtime app configured. */
  enabled = $state(false);
  /** Everyone sitting in any voice room, by name. */
  members = $state({});
  /** The room this device is in (or joining), `#name`. */
  room = $state(null);
  /** "idle", "joining" or "live". */
  phase = $state("idle");
  muted = $state(false);
  /** People whose audio is above the speaking level right now (me included). */
  speaking = new SvelteSet();

  me = "";
  rid = 0;
  waiting = new Map();
  /** Every negotiation step runs in order on this chain. */
  chain = Promise.resolve();
  /** Bumped on every join and leave, so stale steps can tell they are stale. */
  gen = 0;
  pc = null;
  mic = null;
  heard = new Map();
  audioCtx = null;
  meters = new Map();
  meterTimer;
  /** Rejoin this room once the socket is back (we lose the seat when it drops). */
  rejoin = null;

  constructor(send, toast) {
    this.send = send;
    this.toast = toast;
  }

  /** Who is in `room`, in a stable order. */
  inRoom(room) {
    return Object.values(this.members)
      .filter((m) => m.room === room)
      .sort((a, b) => a.user.localeCompare(b.user));
  }

  // ---- what the user does ---------------------------------------------

  /** `again` forces a fresh join even if we think we're already there. */
  async join(room, again = false) {
    if (!this.enabled) return;
    if (!again && this.room === room && this.phase !== "idle") return;
    this.teardown();
    const gen = ++this.gen;
    this.room = room;
    this.phase = "joining";
    try {
      const mic = await this.openMic();
      if (gen !== this.gen) {
        mic.getTracks().forEach((t) => t.stop());
        return;
      }
      this.mic = mic;
      for (const t of mic.getAudioTracks()) t.enabled = !this.muted;
      this.meter(this.me, mic);
      await this.step(gen, () => this.connect(gen, room));
    } catch (e) {
      if (gen !== this.gen) return;
      this.teardown();
      this.room = null;
      this.phase = "idle";
      this.toast(e instanceof VoiceError ? e.message : "Couldn't start voice. Try again in a moment.");
      if (!(e instanceof VoiceError)) console.error("voice join failed", e);
    }
  }

  leave() {
    if (this.phase === "idle" && !this.room) return;
    this.gen++;
    this.rejoin = null;
    this.send({ type: "voice_leave" });
    this.teardown();
    if (this.members[this.me]) delete this.members[this.me];
    this.room = null;
    this.phase = "idle";
  }

  toggleMute() {
    this.muted = !this.muted;
    for (const t of this.mic?.getAudioTracks() ?? []) t.enabled = !this.muted;
    if (this.muted) this.speaking.delete(this.me);
    if (this.phase !== "idle") this.send({ type: "voice_mute", muted: this.muted });
  }

  // ---- what the server says -------------------------------------------

  onWelcome(me, enabled, roster) {
    this.me = me;
    this.enabled = enabled;
    this.members = Object.fromEntries(roster.map((m) => [m.user, m]));
    const room = this.rejoin;
    this.rejoin = null;
    if (room && enabled) void this.join(room, true);
  }

  /** The socket dropped: the server has already let go of our seat. */
  onDisconnect() {
    for (const [rid, w] of this.waiting) {
      clearTimeout(w.timer);
      w.reject(new VoiceError("Lost the connection to Porch."));
      this.waiting.delete(rid);
    }
    if (this.room && this.phase !== "idle") {
      // Keep the audio going through the blip; rejoin when the socket is back.
      this.rejoin = this.room;
    }
  }

  /** Signing out or switching accounts. */
  reset() {
    this.leave();
    this.members = {};
    this.enabled = false;
    this.muted = false;
  }

  onMessage(msg) {
    switch (msg.type) {
      case "voice_reply": {
        const w = this.waiting.get(msg.rid);
        if (!w) return;
        this.waiting.delete(msg.rid);
        clearTimeout(w.timer);
        if (msg.error) w.reject(new VoiceError(msg.error));
        else w.resolve(msg);
        return;
      }
      case "voice_state": {
        const m = msg.member;
        // Copy before overwriting: `members` is a deep proxy.
        const old = this.members[m.user];
        const before = old ? { room: old.room, live: old.live } : undefined;
        this.members[m.user] = m;
        if (m.user === this.me) return;
        const wasHere = before && before.room === this.room && before.live;
        const isHere = m.room === this.room && m.live;
        if (wasHere && !isHere) this.drop(m.user);
        // A fresh `live` means a fresh Realtime session: pull it, even if we
        // heard an older one from this person.
        if (isHere && (!before?.live || before.room !== m.room)) {
          this.drop(m.user);
          if (this.phase === "live") {
            const gen = this.gen;
            void this.step(gen, () => this.pull(gen, [m.user])).catch((e) => this.pullFailed(e));
          }
        }
        return;
      }
      case "voice_left":
        if (this.members[msg.user]) delete this.members[msg.user];
        if (msg.user !== this.me) this.drop(msg.user);
        return;
      case "voice_ended":
        this.gen++;
        this.rejoin = null;
        this.teardown();
        this.room = null;
        this.phase = "idle";
        this.toast(msg.reason);
        return;
    }
  }

  // ---- negotiation ----------------------------------------------------

  request(req) {
    const rid = ++this.rid;
    return new Promise((resolve, reject) => {
      if (!this.send({ ...req, rid })) {
        reject(new VoiceError("Not connected to Porch right now."));
        return;
      }
      const timer = setTimeout(() => {
        this.waiting.delete(rid);
        reject(new VoiceError("Voice took too long to answer. Try again."));
      }, REQUEST_TIMEOUT);
      this.waiting.set(rid, { resolve, reject, timer });
    });
  }

  /** Run `fn` after every earlier step, unless the session it belongs to is gone. */
  step(gen, fn) {
    const run = this.chain.then(() => (gen === this.gen ? fn() : undefined));
    this.chain = run.catch(() => {});
    return run;
  }

  async openMic() {
    if (!navigator.mediaDevices?.getUserMedia) {
      throw new VoiceError("Voice needs a secure (https) connection to the server.");
    }
    try {
      return await navigator.mediaDevices.getUserMedia({
        audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
      });
    } catch (e) {
      const name = e instanceof DOMException ? e.name : "";
      if (name === "NotAllowedError") throw new VoiceError("Porch isn't allowed to use your microphone.");
      if (name === "NotFoundError") throw new VoiceError("No microphone found.");
      throw new VoiceError("Couldn't open your microphone.");
    }
  }

  async connect(gen, room) {
    await this.request({ type: "voice_join", room });
    if (gen !== this.gen) return;

    const pc = new RTCPeerConnection({ iceServers: ICE, bundlePolicy: "max-bundle" });
    this.pc = pc;
    pc.onconnectionstatechange = () => {
      if (pc !== this.pc || pc.connectionState !== "failed") return;
      // ICE gave up (network change, sleep): start over in the same room.
      this.toast("Voice dropped. Reconnecting...");
      if (this.room) void this.join(this.room, true);
    };

    const track = this.mic.getAudioTracks()[0];
    const sender = pc.addTransceiver(track, { direction: "sendonly" });
    await pc.setLocalDescription(await pc.createOffer());
    const mid = sender.mid;
    if (!mid || !pc.localDescription) throw new Error("no mid after setLocalDescription");
    const reply = await this.request({ type: "voice_publish", sdp: pc.localDescription.sdp, mid });
    if (gen !== this.gen) return;
    await pc.setRemoteDescription({ type: "answer", sdp: reply.sdp });
    this.phase = "live";
    if (this.muted) this.send({ type: "voice_mute", muted: true });

    const others = this.inRoom(room)
      .filter((m) => m.user !== this.me && m.live)
      .map((m) => m.user);
    if (others.length) await this.pull(gen, others);
  }

  async pull(gen, users, attempt = 0) {
    const pc = this.pc;
    const wanted = users.filter((u) => !this.heard.has(u));
    if (!pc || !wanted.length) return;
    const reply = await this.request({ type: "voice_pull", users: wanted });
    if (gen !== this.gen) return;

    const got = new Set(reply.tracks.map((t) => t.user));
    const missing = wanted.filter((u) => !got.has(u));
    const delay = PULL_RETRIES[attempt];
    if (missing.length && delay !== undefined) {
      setTimeout(() => {
        const still = missing.filter((u) => {
          const m = this.members[u];
          return m?.live && m.room === this.room && !this.heard.has(u);
        });
        if (still.length) {
          void this.step(gen, () => this.pull(gen, still, attempt + 1)).catch((e) => this.pullFailed(e));
        }
      }, delay);
    }

    if (!reply.sdp) return;
    await pc.setRemoteDescription({ type: "offer", sdp: reply.sdp });
    await pc.setLocalDescription(await pc.createAnswer());
    await this.request({ type: "voice_answer", sdp: pc.localDescription.sdp });
    if (gen !== this.gen) return;
    for (const { user, mid } of reply.tracks) {
      const t = pc.getTransceivers().find((x) => x.mid === mid);
      if (t) this.hear(user, t.receiver.track);
    }
  }

  pullFailed(e) {
    if (e instanceof VoiceError) this.toast(e.message);
    else console.error("voice pull failed", e);
  }

  // ---- audio ----------------------------------------------------------

  hear(user, track) {
    this.drop(user);
    const stream = new MediaStream([track]);
    // Chromium only plays remote WebRTC audio through a media element.
    const audio = new Audio();
    audio.autoplay = true;
    audio.srcObject = stream;
    void audio.play().catch(() => {});
    this.heard.set(user, { audio, track });
    this.meter(user, stream);
  }

  drop(user) {
    const h = this.heard.get(user);
    if (h) {
      h.audio.pause();
      h.audio.srcObject = null;
      this.heard.delete(user);
    }
    this.unmeter(user);
  }

  meter(user, stream) {
    this.unmeter(user);
    this.audioCtx ??= new AudioContext();
    void this.audioCtx.resume().catch(() => {});
    const source = this.audioCtx.createMediaStreamSource(stream);
    const analyser = this.audioCtx.createAnalyser();
    analyser.fftSize = 2048;
    source.connect(analyser);
    this.meters.set(user, { source, analyser, data: new Float32Array(analyser.fftSize), lastLoud: 0 });
    this.meterTimer ??= setInterval(() => this.measure(), 100);
  }

  unmeter(user) {
    const m = this.meters.get(user);
    if (m) {
      m.source.disconnect();
      this.meters.delete(user);
    }
    this.speaking.delete(user);
  }

  measure() {
    const now = performance.now();
    for (const [user, m] of this.meters) {
      m.analyser.getFloatTimeDomainData(m.data);
      let sum = 0;
      for (const v of m.data) sum += v * v;
      const rms = Math.sqrt(sum / m.data.length);
      const silent = (user === this.me && this.muted) || this.members[user]?.muted;
      if (rms > SPEAKING_LEVEL && !silent) m.lastLoud = now;
      const on = now - m.lastLoud < SPEAKING_HOLD;
      if (on !== this.speaking.has(user)) {
        if (on) this.speaking.add(user);
        else this.speaking.delete(user);
      }
    }
  }

  /** Close everything local: connection, microphone, playback, meters. */
  teardown() {
    for (const user of [...this.heard.keys()]) this.drop(user);
    for (const user of [...this.meters.keys()]) this.unmeter(user);
    clearInterval(this.meterTimer);
    this.meterTimer = undefined;
    if (this.pc) {
      this.pc.onconnectionstatechange = null;
      this.pc.close();
      this.pc = null;
    }
    this.mic?.getTracks().forEach((t) => t.stop());
    this.mic = null;
    this.speaking.clear();
  }
}
