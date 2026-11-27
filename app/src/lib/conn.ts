// One WebSocket to the Porch server, kept alive and reconnected with backoff.

import type { ClientMessage } from "./protocol/ClientMessage";
import type { ServerMessage } from "./protocol/ServerMessage";

export type ConnStatus = "connecting" | "open" | "closed";

export interface ConnHandlers {
  /** The socket opened; send the sign-in frame now. */
  open(): void;
  message(msg: ServerMessage): void;
  status(status: ConnStatus): void;
}

const PING_EVERY = 25_000;
const SILENCE_LIMIT = 60_000;
const BACKOFF_MIN = 500;
const BACKOFF_MAX = 15_000;

export class Connection {
  private ws: WebSocket | null = null;
  private stopped = false;
  private attempts = 0;
  private retryTimer: ReturnType<typeof setTimeout> | undefined;
  private pingTimer: ReturnType<typeof setInterval> | undefined;
  private lastHeard = 0;

  constructor(
    readonly url: string,
    private handlers: ConnHandlers,
  ) {}

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

  send(msg: ClientMessage): boolean {
    if (this.ws?.readyState !== WebSocket.OPEN) return false;
    this.ws.send(JSON.stringify(msg));
    return true;
  }

  private connect() {
    clearTimeout(this.retryTimer);
    this.handlers.status("connecting");
    let ws: WebSocket;
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
        this.handlers.message(JSON.parse(ev.data as string) as ServerMessage);
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

  private keepAlive() {
    if (Date.now() - this.lastHeard > SILENCE_LIMIT) {
      // The socket looks open but nothing comes back (sleep, Wi-Fi change).
      this.ws?.close();
      return;
    }
    this.send({ type: "ping" });
  }

  private scheduleRetry() {
    if (this.stopped) return;
    const base = Math.min(BACKOFF_MAX, BACKOFF_MIN * 2 ** this.attempts);
    this.attempts++;
    const delay = base / 2 + Math.random() * (base / 2);
    this.retryTimer = setTimeout(() => this.connect(), delay);
  }

  private retryNow = () => {
    if (this.stopped || this.ws) return;
    this.attempts = 0;
    this.connect();
  };

  private onVisible = () => {
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
export function serverUrl(input: string): string {
  let raw = input.trim();
  if (!raw) {
    const proto = location.protocol === "https:" ? "wss:" : "ws:";
    return `${proto}//${location.host}/ws`;
  }
  if (!/^[a-z]+:\/\//i.test(raw)) {
    const hostPart = raw.split(/[/:]/)[0] ?? "";
    const local =
      hostPart === "localhost" ||
      /^127\./.test(hostPart) ||
      /^10\./.test(hostPart) ||
      /^192\.168\./.test(hostPart) ||
      /^172\.(1[6-9]|2\d|3[01])\./.test(hostPart) ||
      hostPart.endsWith(".local");
    raw = `${local ? "ws" : "wss"}://${raw}`;
  }
  const url = new URL(raw);
  if (url.protocol === "https:") url.protocol = "wss:";
  if (url.protocol === "http:") url.protocol = "ws:";
  if (url.pathname === "/" || url.pathname === "") url.pathname = "/ws";
  return url.toString();
}
