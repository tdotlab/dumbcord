"""Voice end to end: two browsers, fake microphones, real Cloudflare Realtime.

Start a server with voice on and the built client, for example

    PORCH_CF_APP_ID=... PORCH_CF_APP_TOKEN=... \
      cargo run --release -p porch-server -- --db /tmp/vt.sqlite3 \
      --bind 127.0.0.1:7979 --web-dir app/dist

then run

    uv run --no-project --with playwright python scripts/voice_e2e.py [URL] [SHOT_DIR]

It registers two fresh users, puts both in voice in #general, and checks that
audio flows both ways, the speaking rings light, a dropped socket rejoins,
and mute and leave reach the other side. It uses the installed Edge by default
(PORCH_E2E_CHANNEL=chrome or chromium to change that), so nothing downloads.
Each run spends a little of the Realtime app's free allowance.
"""

import asyncio
import json
import math
import os
import random
import struct
import sys
import tempfile
import wave
from pathlib import Path

from playwright.async_api import async_playwright

URL = sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:7979/"
SHOTS = Path(sys.argv[2]) if len(sys.argv) > 2 else None
CHANNEL = os.environ.get("PORCH_E2E_CHANNEL", "msedge")
RUN = random.randint(100, 999)  # fresh names on a reused server


def speech_like(path: Path, seconds: int = 20, rate: int = 48_000) -> None:
    """Voiced bursts with gaps. A steady tone gets eaten by noise suppression."""
    frames = bytearray()
    for i in range(seconds * rate):
        t = i / rate
        syllable = (t % 0.4) < 0.25
        env = math.sin(math.pi * (t % 0.4) / 0.25) if syllable else 0.0
        pitch = 140 + 30 * math.sin(2 * math.pi * 0.7 * t)
        s = sum(math.sin(2 * math.pi * pitch * h * t) / h for h in range(1, 6))
        frames += struct.pack("<h", int(9000 * env * s / 2.3))
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(bytes(frames))


# Keep handles on the page's peer connections and sockets.
CAPTURE = """(() => {
  const P = window.RTCPeerConnection;
  window.__pcs = [];
  window.RTCPeerConnection = function (...a) { const pc = new P(...a); window.__pcs.push(pc); return pc; };
  window.RTCPeerConnection.prototype = P.prototype;
  const W = window.WebSocket;
  window.__sockets = [];
  window.WebSocket = function (...a) { const ws = new W(...a); window.__sockets.push(ws); return ws; };
  Object.assign(window.WebSocket, { CONNECTING: 0, OPEN: 1, CLOSING: 2, CLOSED: 3 });
  window.WebSocket.prototype = W.prototype;
})()"""

STATS = """async () => {
  const pc = (window.__pcs || []).filter((x) => x.connectionState !== "closed").at(-1);
  if (!pc) return null;
  const out = { state: pc.connectionState, inbound: [], outbound: 0 };
  (await pc.getStats()).forEach((r) => {
    if (r.type === "inbound-rtp" && r.kind === "audio") out.inbound.push(r.bytesReceived);
    if (r.type === "outbound-rtp" && r.kind === "audio") out.outbound += r.bytesSent;
  });
  return out;
}"""


def hearing(stats) -> bool:
    return bool(stats) and any(b > 1000 for b in stats["inbound"])


async def sign_up(page, name):
    await page.goto(URL)
    await page.get_by_role("button", name="Create an account").click()
    if await page.get_by_label("Server").count():
        await page.get_by_label("Server").fill(URL.split("//", 1)[-1].rstrip("/"))
    await page.get_by_label("Username").fill(f"{name}{RUN}")
    await page.get_by_label("Password").fill("correct horse battery")
    await page.get_by_role("button", name="Create account").click()
    await page.get_by_role("button", name="Join voice").wait_for(timeout=10_000)


async def run(tone: Path) -> bool:
    args = [
        "--use-fake-ui-for-media-stream",
        "--use-fake-device-for-media-stream",
        f"--use-file-for-fake-audio-capture={tone}",
        "--autoplay-policy=no-user-gesture-required",
    ]
    ok = True
    async with async_playwright() as p:
        browser = await p.chromium.launch(channel=CHANNEL, args=args)
        pages = {}
        for name in ("alice", "bob"):
            ctx = await browser.new_context(viewport={"width": 1100, "height": 720})
            await ctx.add_init_script(CAPTURE)
            page = await ctx.new_page()
            page.on("console", lambda m, n=name: m.type == "error" and print(f"[{n}] {m.text}"))
            await sign_up(page, name)
            pages[name] = page

        for name, page in pages.items():
            await page.get_by_role("button", name="Join voice").click()
            await page.get_by_text("Voice connected").wait_for(timeout=20_000)
            print(f"{name}: voice connected")

        await asyncio.sleep(6)
        for name, page in pages.items():
            stats = await page.evaluate(STATS)
            print(name, json.dumps(stats))
            ok &= hearing(stats) and stats["state"] == "connected"
            other = "bob" if name == "alice" else "alice"
            ring = page.locator(".sitter", has_text=f"{other}{RUN}").locator(".avatar.speaking")
            try:
                await ring.wait_for(timeout=4_000)
                print(f"{name} sees {other}'s speaking ring")
            except Exception:
                print(f"{name} never saw {other}'s speaking ring")
                ok = False
            if SHOTS:
                await page.screenshot(path=str(SHOTS / f"voice-{name}.png"))

        # A dropped socket costs the seat; the client should sit back down.
        alice, bob = pages["alice"], pages["bob"]
        await alice.evaluate("window.__sockets.forEach((w) => w.close())")
        await bob.locator(".sitter", has_text=f"alice{RUN}").wait_for(state="detached", timeout=5_000)
        print("bob saw alice drop")
        await alice.get_by_text("Voice connected").wait_for(timeout=30_000)
        await asyncio.sleep(4)
        for name, page in pages.items():
            stats = await page.evaluate(STATS)
            print("after rejoin", name, json.dumps(stats))
            ok &= hearing(stats)

        await alice.get_by_role("button", name="Mute", exact=True).click()
        await bob.locator(".sitter", has_text=f"alice{RUN}").locator(".muted").wait_for(timeout=5_000)
        print("bob sees alice muted")

        await bob.get_by_role("button", name="Leave voice").first.click()
        await alice.locator(".sitter", has_text=f"bob{RUN}").wait_for(state="detached", timeout=5_000)
        print("alice saw bob leave")

        await browser.close()
    return ok


def main() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        tone = Path(tmp) / "speech.wav"
        speech_like(tone)
        ok = asyncio.run(run(tone))
    print("PASS" if ok else "FAIL")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
