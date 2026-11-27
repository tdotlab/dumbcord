// One WebSocket to the Porch server, kept alive and reconnected with backoff.
//
// Status goes "connecting" -> "open" -> "closed" (and round again). The
// handlers are { open(), message(msg), status(status) }; `open` fires when the
// socket opens, which is when the sign-in frame should go.

const PING_EVERY = 25_000;
const SILENCE_LIMIT = 60_000;
const BACKOFF_MIN = 500;
const BACKOFF_MAX = 15_000;

export class Connection {
  ws = null;
  stopped = false;
  attempts = 0;
  retryTimer;
  pingTimer;
  lastHeard = 0;

  constructor(url, handlers) {
    this.url = url;
    this.handlers = handlers;
  }

  start() {
    this.stopped = false;
    this.connect();
    window.addEventListener("online", this.retryNow);
    document.addEventListener("visibilitychange", this.onVisible);
  }

  stop() {
    this.stopped = true;
    clearTimeout(this.retryTimer);
    clearInterval(this.pingTimer);
    window.removeEventListener("online", this.retryNow);
    document.removeEventListener("visibilitychange", this.onVisible);
    const ws = this.ws;
    this.ws = null;
    ws?.close(1000);
  }

  /** Send a frame; false if the socket isn't open. */
  send(msg) {
    if (this.ws?.readyState !== WebSocket.OPEN) return false;
    this.ws.send(JSON.stringify(msg));
    return true;
  }

  connect() {
    clearTimeout(this.retryTimer);
    this.handlers.status("connecting");
    let ws;
    try {
      ws = new WebSocket(this.url);
    } catch {
      this.scheduleRetry();
      return;
    }
    this.ws = ws;

    ws.onopen = () => {
      if (this.ws !== ws) return;
      this.attempts = 0;
      this.lastHeard = Date.now();
      this.handlers.status("open");
      this.handlers.open();
      clearInterval(this.pingTimer);
      this.pingTimer = setInterval(() => this.keepAlive(), PING_EVERY);
    };
    ws.onmessage = (ev) => {
      if (this.ws !== ws) return;
      this.lastHeard = Date.now();
      try {
        this.handlers.message(JSON.parse(ev.data));
      } catch (err) {
        console.error("bad frame from server", err);
      }
    };
    ws.onclose = () => {
      if (this.ws !== ws) return;
      this.ws = null;
      clearInterval(this.pingTimer);
      this.handlers.status("closed");
      if (!this.stopped) this.scheduleRetry();
    };
  }

  keepAlive() {
    if (Date.now() - this.lastHeard > SILENCE_LIMIT) {
      // The socket looks open but nothing comes back (sleep, Wi-Fi change).
      this.ws?.close();
      return;
    }
    this.send({ type: "ping" });
  }

  scheduleRetry() {
    if (this.stopped) return;
    const base = Math.min(BACKOFF_MAX, BACKOFF_MIN * 2 ** this.attempts);
    this.attempts++;
    const delay = base / 2 + Math.random() * (base / 2);
    this.retryTimer = setTimeout(() => this.connect(), delay);
  }

  retryNow = () => {
    if (this.stopped || this.ws) return;
    this.attempts = 0;
    this.connect();
  };

  onVisible = () => {
    if (document.visibilityState === "visible") {
      if (this.ws && Date.now() - this.lastHeard > PING_EVERY) this.keepAlive();
      else this.retryNow();
    }
  };
}

/**
 * Turn what someone typed ("porch.example.com", "localhost:7878",
 * "https://x.y/") into a WebSocket URL. Blank means "this page's server".
 */
export function serverUrl(input) {
  let raw = input.trim();
  if (!raw) {
    const proto = location.protocol === "https:" ? "wss:" : "ws:";
    return `${proto}//${location.host}/ws`;
  }
  if (!/^[a-z]+:\/\//i.test(raw)) {
    const host = raw.split(/[/:]/)[0] ?? "";
    const local =
      host === "localhost" ||
      /^127\./.test(host) ||
      /^10\./.test(host) ||
      /^192\.168\./.test(host) ||
      /^172\.(1[6-9]|2\d|3[01])\./.test(host) ||
      host.endsWith(".local");
    raw = `${local ? "ws" : "wss"}://${raw}`;
  }
  const url = new URL(raw);
  if (url.protocol === "https:") url.protocol = "wss:";
  if (url.protocol === "http:") url.protocol = "ws:";
  if (url.pathname === "/" || url.pathname === "") url.pathname = "/ws";
  return url.toString();
}

/** The host part of a server address, for the status line. */
export function hostOf(input) {
  try {
    return new URL(serverUrl(input)).host;
  } catch {
    return input;
  }
}
