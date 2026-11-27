// Client state: one instance, shared by every component.
//
// A conversation ("conv") is keyed "#channel" or "@user" and looks like
//   { key, messages, loaded, loading, older, hasMore, lastId, lastRead, attention }
// where `older` means the request in flight is for an older page and
// `attention` counts DMs and @mentions that arrived since the last read.
// Messages are the server's ChatMessage ({ id, to, author, text, created_at,
// edited_at }) plus, while unconfirmed, `nonce` and `state` ("pending" or
// "failed"). The wire format lives in server/src/protocol.rs.

import { SvelteSet } from "svelte/reactivity";
import { Connection, serverUrl } from "./conn.js";
import { mentions, setKnownUsers, setMe } from "./markdown.js";
import { appIsVisible, notify } from "./platform.js";
import { Voice } from "./voice.svelte.js";

const KEY = {
  server: "porch.server",
  token: "porch.token",
  current: "porch.current",
};

const TYPING_TTL = 6000;
const PENDING_TIMEOUT = 15_000;
const NOTICE_FOR = 6000;

/** The conversation a message belongs to, from `me`'s side. */
export function convOf(m, me) {
  if (m.to.startsWith("#") || m.author === me) return m.to;
  return `@${m.author}`;
}

/** "#general" -> "general", "@amy" -> "amy". */
export function label(key) {
  return key.slice(1);
}

class Porch {
  /** "signed-out", "resuming" (have a token, waiting for welcome) or "ready". */
  phase = $state("signed-out");
  /** The socket: "connecting", "open" or "closed". */
  status = $state("closed");
  busy = $state(false);
  authError = $state("");
  /** A passing message for the status line; cleared after a few seconds. */
  notice = $state("");

  server = $state(localStorage.getItem(KEY.server) ?? "");
  me = $state("");
  channels = $state([]);
  users = $state([]);
  online = new SvelteSet();
  convs = $state({});
  current = $state(localStorage.getItem(KEY.current) ?? "#general");
  /** typing[key][user] = time the indicator expires. */
  typing = $state({});
  drafts = $state({});
  /** Id of the message being edited, or null. */
  editing = $state(null);
  voice = new Voice(
    (msg) => this.send(msg),
    (message) => this.toast(message),
  );

  conn = null;
  token = localStorage.getItem(KEY.token);
  pendingAuth = null;
  seq = 0;
  lastTypingSent = 0;
  noticeTimer;

  constructor() {
    setInterval(() => this.pruneTyping(), 1000);
    if (this.token) {
      this.phase = "resuming";
      this.startConnection();
    }
  }

  // ---- signing in and out ---------------------------------------------

  /** `mode` is "login" or "register". */
  signIn(mode, server, username, password, invite) {
    this.authError = "";
    this.server = server.trim();
    localStorage.setItem(KEY.server, this.server);
    this.token = null;
    this.pendingAuth =
      mode === "login"
        ? { type: "login", username, password }
        : { type: "register", username, password, invite: invite.trim() || null };
    this.busy = true;
    this.startConnection();
  }

  signOut() {
    this.conn?.send({ type: "logout" });
    this.endSession("");
  }

  endSession(why) {
    const conn = this.conn;
    this.conn = null;
    // Give the logout frame a moment to leave before the socket closes.
    setTimeout(() => conn?.stop(), 150);
    this.token = null;
    localStorage.removeItem(KEY.token);
    this.pendingAuth = null;
    this.me = "";
    this.convs = {};
    this.channels = [];
    this.users = [];
    this.online.clear();
    this.typing = {};
    this.drafts = {};
    this.editing = null;
    this.voice.reset();
    this.busy = false;
    this.authError = why;
    this.phase = "signed-out";
  }

  startConnection() {
    this.conn?.stop();
    let url;
    try {
      url = serverUrl(this.server);
    } catch {
      this.busy = false;
      this.authError = "That server address doesn't look right.";
      return;
    }
    this.conn = new Connection(url, {
      open: () => this.onOpen(),
      message: (m) => this.onMessage(m),
      status: (s) => this.onStatus(s),
    });
    this.conn.start();
  }

  onOpen() {
    if (this.token) this.conn?.send({ type: "resume", token: this.token });
    else if (this.pendingAuth) this.conn?.send(this.pendingAuth);
  }

  onStatus(s) {
    this.status = s;
    if (s !== "closed") return;
    this.voice.onDisconnect();
    for (const conv of Object.values(this.convs)) {
      for (const m of conv.messages) if (m.state === "pending") m.state = "failed";
    }
    if (this.busy && !this.token) {
      // Signing in and the server went away or was never there.
      this.busy = false;
      this.authError ||= "Couldn't reach that server. Check the address and try again.";
      this.conn?.stop();
      this.conn = null;
    }
  }

  // ---- incoming -------------------------------------------------------

  onMessage(msg) {
    switch (msg.type) {
      case "welcome":
        return this.onWelcome(msg);
      case "error":
        return this.onError(msg.code, msg.message);
      case "history": {
        const conv = this.conv(msg.to);
        const page = msg.messages;
        if (conv.older) {
          const known = new Set(conv.messages.map((m) => m.id));
          conv.messages = [...page.filter((m) => !known.has(m.id)), ...conv.messages];
        } else {
          // Keep live messages that raced the page, and anything unsent.
          const newest = page.at(-1)?.id ?? 0;
          const live = conv.messages.filter((m) => !m.state && m.id > newest);
          const unsent = conv.messages.filter((m) => m.state);
          conv.messages = [...page, ...live, ...unsent];
          if (newest) conv.lastId = Math.max(conv.lastId, newest);
        }
        conv.hasMore = msg.has_more;
        conv.loaded = true;
        conv.loading = false;
        conv.older = false;
        return;
      }
      case "message":
        return this.onChat(msg.message, msg.nonce);
      case "edited": {
        const conv = this.convs[convOf(msg.message, this.me)];
        const m = conv?.messages.find((x) => x.id === msg.message.id);
        if (m) {
          m.text = msg.message.text;
          m.edited_at = msg.message.edited_at;
        }
        return;
      }
      case "deleted": {
        const conv = this.convs[convOf(msg, this.me)];
        if (conv) conv.messages = conv.messages.filter((m) => m.id !== msg.id);
        return;
      }
      case "channel_created":
        if (!this.channels.includes(msg.name)) {
          this.channels = [...this.channels, msg.name].sort();
        }
        return;
      case "user_joined":
        if (!this.users.includes(msg.user)) {
          this.users = [...this.users, msg.user].sort();
          setKnownUsers(this.users);
        }
        return;
      case "presence":
        if (msg.online) this.online.add(msg.user);
        else this.online.delete(msg.user);
        return;
      case "typing":
        this.typing[msg.to] = { ...this.typing[msg.to], [msg.user]: Date.now() + TYPING_TTL };
        return;
      case "read": {
        const conv = this.conv(msg.to);
        conv.lastRead = Math.max(conv.lastRead, msg.id);
        if (conv.lastRead >= conv.lastId) conv.attention = 0;
        return;
      }
      case "pong":
        return;
      case "voice_reply":
      case "voice_state":
      case "voice_left":
      case "voice_ended":
        return this.voice.onMessage(msg);
    }
  }

  onWelcome(msg) {
    this.token = msg.token;
    localStorage.setItem(KEY.token, msg.token);
    this.pendingAuth = null;
    this.me = msg.user;
    setMe(msg.user);
    this.channels = msg.channels;
    this.users = msg.users;
    setKnownUsers(msg.users);
    this.online.clear();
    for (const u of msg.online) this.online.add(u);
    this.voice.onWelcome(msg.user, msg.voice_enabled, msg.voice);

    // Anything loaded before a reconnect may have gaps: reload on demand.
    for (const conv of Object.values(this.convs)) {
      conv.loaded = false;
      conv.loading = false;
      conv.older = false;
    }
    for (const s of msg.convs) {
      const conv = this.conv(s.to);
      conv.lastId = s.last_id;
      conv.lastRead = s.last_read;
      if (conv.lastRead >= conv.lastId) conv.attention = 0;
    }
    if (!this.exists(this.current)) this.current = "#general";
    this.busy = false;
    this.authError = "";
    this.phase = "ready";
    this.load(this.current);
  }

  onError(code, message) {
    if (code === "token_invalid") {
      this.endSession("Your session ended. Sign in again.");
      return;
    }
    if (this.phase !== "ready") {
      this.busy = false;
      this.authError = message;
      if (!this.token) {
        this.conn?.stop();
        this.conn = null;
        if (this.phase === "resuming") this.phase = "signed-out";
      }
      return;
    }
    // Requests are answered in order, so the oldest unconfirmed send is the
    // likeliest culprit when a send-type error comes back.
    if (code === "rate_limited" || code === "bad_request" || code === "not_found") {
      for (const conv of Object.values(this.convs)) {
        const m = conv.messages.find((x) => x.state === "pending");
        if (m) {
          m.state = "failed";
          break;
        }
      }
    }
    this.toast(message);
  }

  onChat(message, nonce) {
    const key = convOf(message, this.me);
    const conv = this.conv(key);
    if (conv.loaded || conv.loading) {
      const mine = nonce ? conv.messages.findIndex((m) => m.nonce === nonce) : -1;
      if (mine >= 0) conv.messages[mine] = message;
      else if (!conv.messages.some((m) => m.id === message.id)) {
        // Keep unsent messages at the bottom.
        const firstUnsent = conv.messages.findIndex((m) => m.state);
        if (firstUnsent < 0) conv.messages.push(message);
        else conv.messages.splice(firstUnsent, 0, message);
      }
    }
    conv.lastId = Math.max(conv.lastId, message.id);
    const typing = this.typing[key];
    if (typing?.[message.author]) delete typing[message.author];

    if (message.author === this.me) {
      conv.lastRead = Math.max(conv.lastRead, message.id);
      return;
    }
    const looking = key === this.current && appIsVisible();
    const forMe = key.startsWith("@") || mentions(message.text, this.me);
    if (!looking && forMe) {
      conv.attention++;
      const where = key.startsWith("#") ? ` in ${key}` : "";
      const body = message.text.length > 160 ? `${message.text.slice(0, 157)}...` : message.text;
      void notify(`${message.author}${where}`, body);
    }
  }

  // ---- conversations --------------------------------------------------

  conv(key) {
    this.convs[key] ??= {
      key,
      messages: [],
      loaded: false,
      loading: false,
      older: false,
      hasMore: false,
      lastId: 0,
      lastRead: 0,
      attention: 0,
    };
    return this.convs[key];
  }

  exists(key) {
    if (key.startsWith("#")) return this.channels.includes(key.slice(1));
    return key !== `@${this.me}` && this.users.includes(key.slice(1));
  }

  open(key) {
    if (!this.exists(key)) return;
    this.current = key;
    this.editing = null;
    localStorage.setItem(KEY.current, key);
    this.load(key);
  }

  load(key) {
    const conv = this.conv(key);
    if (conv.loaded || conv.loading) return;
    if (this.send({ type: "history", to: key, before: null })) conv.loading = true;
  }

  loadOlder(key) {
    const conv = this.conv(key);
    if (!conv.loaded || conv.loading || !conv.hasMore) return;
    const oldest = conv.messages.find((m) => !m.state);
    if (!oldest) return;
    if (this.send({ type: "history", to: key, before: oldest.id })) {
      conv.loading = true;
      conv.older = true;
    }
  }

  /** The user is looking at the bottom of `key`: everything there is read. */
  markSeen(key) {
    const conv = this.convs[key];
    if (!conv) return;
    if (conv.lastId <= conv.lastRead) {
      conv.attention = 0;
      return;
    }
    conv.lastRead = conv.lastId;
    conv.attention = 0;
    this.send({ type: "mark_read", to: key, id: conv.lastId });
  }

  unread(key) {
    const conv = this.convs[key];
    return !!conv && conv.lastId > conv.lastRead;
  }

  say(key, text) {
    const trimmed = text.trim();
    if (!trimmed) return;
    const conv = this.conv(key);
    const nonce = `${Date.now().toString(36)}.${++this.seq}`;
    conv.messages.push({
      id: -this.seq,
      to: key,
      author: this.me,
      text: trimmed,
      created_at: Date.now(),
      edited_at: null,
      nonce,
      state: "pending",
    });
    this.transmit(key, nonce);
  }

  transmit(key, nonce) {
    const m = this.convs[key]?.messages.find((x) => x.nonce === nonce);
    if (!m) return;
    const sent = this.send({ type: "send", to: key, text: m.text, nonce });
    m.state = sent ? "pending" : "failed";
    if (sent) {
      setTimeout(() => {
        const still = this.convs[key]?.messages.find((x) => x.nonce === nonce);
        if (still?.state === "pending") still.state = "failed";
      }, PENDING_TIMEOUT);
    }
  }

  retry(key, nonce) {
    this.transmit(key, nonce);
  }

  discard(key, nonce) {
    const conv = this.convs[key];
    if (conv) conv.messages = conv.messages.filter((m) => m.nonce !== nonce);
  }

  edit(id, text) {
    this.editing = null;
    const trimmed = text.trim();
    if (trimmed) this.send({ type: "edit", id, text: trimmed });
  }

  remove(id) {
    this.send({ type: "delete", id });
  }

  lastOwn(key) {
    return this.convs[key]?.messages.findLast((m) => m.author === this.me && !m.state);
  }

  typingIn(key) {
    const now = Date.now();
    return Object.entries(this.typing[key] ?? {})
      .filter(([, until]) => until > now)
      .map(([user]) => user);
  }

  /** Tell others we're typing, at most every few seconds. */
  nudgeTyping(key) {
    const now = Date.now();
    if (now - this.lastTypingSent < 3000) return;
    this.lastTypingSent = now;
    this.send({ type: "typing", to: key });
  }

  createChannel(name) {
    this.send({ type: "create_channel", name });
  }

  toast(message) {
    this.notice = message;
    clearTimeout(this.noticeTimer);
    this.noticeTimer = setTimeout(() => (this.notice = ""), NOTICE_FOR);
  }

  send(msg) {
    return this.conn?.send(msg) ?? false;
  }

  pruneTyping() {
    const now = Date.now();
    for (const [key, who] of Object.entries(this.typing)) {
      for (const [user, until] of Object.entries(who)) {
        if (until <= now) delete this.typing[key][user];
      }
    }
  }
}

export const porch = new Porch();
