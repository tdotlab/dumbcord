<script>
  import { porch } from "./lib/porch.svelte.js";
  import { hostOf } from "./lib/conn.js";
  import Chat from "./lib/components/Chat.svelte";
  import Login from "./lib/components/Login.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";

  let menuOpen = $state(false);

  const host = $derived(hostOf(porch.server));
  // The status line: a passing notice wins, then trouble, then the usual.
  const status = $derived.by(() => {
    if (porch.notice) return { text: porch.notice, tone: "notice" };
    if (porch.phase === "signed-out") return { text: "Not signed in", tone: "" };
    if (porch.phase === "resuming") return { text: `Connecting to ${host}...`, tone: "" };
    if (porch.status !== "open") return { text: `Reconnecting to ${host}...`, tone: "warn" };
    return { text: `Connected to ${host} as ${porch.me}`, tone: "" };
  });

  $effect(() => {
    const waiting = Object.values(porch.convs).reduce((n, c) => n + c.attention, 0);
    document.title = waiting ? `(${waiting}) Porch` : "Porch";
  });

  $effect(() => {
    const mq = window.matchMedia("(max-width: 700px)");
    const shut = () => (menuOpen = false);
    mq.addEventListener("change", shut);
    return () => mq.removeEventListener("change", shut);
  });
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && menuOpen && (menuOpen = false)} />

<div class="window">
  <header class="bar">
    {#if porch.phase === "ready"}
      <button class="btn menu" type="button" aria-expanded={menuOpen} onclick={() => (menuOpen = !menuOpen)}>
        Menu
      </button>
    {/if}
    <span class="wordmark">PORCH</span>
    <span class="rule" aria-hidden="true"></span>
    <span class="status {status.tone}" role="status" title={status.text}>{status.text}</span>
    {#if porch.phase !== "signed-out"}
      <button class="btn" type="button" onclick={() => porch.signOut()}>Sign out</button>
    {/if}
  </header>

  {#if porch.phase === "signed-out"}
    <Login />
  {:else if porch.phase === "resuming"}
    <main class="waiting" aria-busy="true">Connecting...</main>
  {:else}
    <div class="body" class:menu-open={menuOpen}>
      <div class="side">
        <Sidebar onnavigate={() => (menuOpen = false)} />
      </div>
      <main class="main">
        <Chat />
      </main>
      {#if menuOpen}
        <button class="scrim" type="button" aria-label="Close menu" onclick={() => (menuOpen = false)}></button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: none;
    min-height: 40px;
    padding: 4px var(--pad);
    padding-top: max(4px, env(safe-area-inset-top));
    border-bottom: 1px solid var(--line);
  }

  .wordmark {
    font-size: 21px;
    font-weight: 300;
    letter-spacing: 0.04em;
    line-height: 1;
    color: var(--strong);
  }

  .rule {
    align-self: stretch;
    width: 1px;
    margin: 2px 0;
    background: var(--line);
  }

  .status {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    color: var(--weak);
  }

  .status.warn {
    color: var(--attention);
  }

  .status.notice {
    color: var(--strong);
  }

  .menu {
    display: none;
  }

  .waiting {
    display: grid;
    place-items: center;
    flex: 1;
    color: var(--weak);
  }

  .body {
    position: relative;
    display: grid;
    grid-template-columns: var(--sidebar-w) minmax(0, 1fr);
    flex: 1;
    min-height: 0;
  }

  .side {
    min-height: 0;
    border-right: 1px solid var(--line);
    background: var(--bg);
  }

  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .scrim {
    display: none;
  }

  @media (max-width: 700px) {
    .menu {
      display: inline-flex;
    }

    .wordmark {
      font-size: 19px;
    }

    .body {
      grid-template-columns: minmax(0, 1fr);
    }

    .side {
      position: absolute;
      inset: 0 auto 0 0;
      z-index: 10;
      width: min(85vw, 300px);
      transform: translateX(-101%);
      transition: transform 160ms ease-out;
    }

    .menu-open .side {
      transform: none;
    }

    .scrim {
      display: block;
      position: absolute;
      inset: 0;
      z-index: 9;
      border: 0;
      background: rgba(0, 0, 0, 0.55);
    }
  }
</style>
