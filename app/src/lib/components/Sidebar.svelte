<script lang="ts">
  import { porch, label } from "../porch.svelte";
  import Avatar from "./Avatar.svelte";
  import Icon from "./Icon.svelte";
  import PorchLight, { glowOf } from "./PorchLight.svelte";

  let { onnavigate }: { onnavigate?: () => void } = $props();

  let adding = $state(false);
  let newChannel = $state("");

  const voice = porch.voice;

  const company = $derived([...porch.online].filter((u) => u !== porch.me).length);
  const porchLine = $derived(
    company === 0
      ? "Just you out here"
      : `${company + 1} on the porch`,
  );

  function go(key: string) {
    porch.open(key);
    onnavigate?.();
  }

  function addChannel(e: SubmitEvent) {
    e.preventDefault();
    const name = newChannel.trim().replace(/^#/, "").toLowerCase().replace(/\s+/g, "-");
    if (name) {
      porch.createChannel(name);
      porch.channels.includes(name) || setTimeout(() => porch.open(`#${name}`), 300);
    }
    newChannel = "";
    adding = false;
  }

  function focus(node: HTMLInputElement) {
    node.focus();
  }
</script>

<nav class="sidebar" aria-label="Conversations">
  <header class="porch" style:--glow={glowOf(company)}>
    <PorchLight height={62} {company} />
    <div>
      <div class="wordmark">Porch</div>
      <div class="who" class:lit={company > 0}>{porchLine}</div>
    </div>
  </header>

  <div class="lists">
    <section>
      <div class="heading">
        <h2>Channels</h2>
        <button class="icon" title="New channel" aria-label="New channel" onclick={() => (adding = !adding)}>
          <Icon name="plus" size={16} />
        </button>
      </div>

      {#if adding}
        <form class="new" onsubmit={addChannel}>
          <input
            use:focus
            bind:value={newChannel}
            placeholder="new-channel"
            maxlength="32"
            aria-label="New channel name"
            onkeydown={(e) => e.key === "Escape" && (adding = false)}
            onblur={() => !newChannel && (adding = false)}
          />
        </form>
      {/if}

      <ul>
        {#each porch.channels as name (name)}
          {@const key = `#${name}`}
          {@const conv = porch.convs[key]}
          {@const sitting = voice.inRoom(key)}
          <li>
            <button
              class="row"
              class:current={porch.current === key}
              class:unread={conv && conv.lastId > conv.lastRead}
              aria-current={porch.current === key ? "page" : undefined}
              onclick={() => go(key)}
            >
              <span class="glyph"><Icon name="hash" size={16} /></span>
              <span class="name">{name}</span>
              {#if conv?.attention}
                <span class="badge">{conv.attention}</span>
              {:else if conv && conv.lastId > conv.lastRead}
                <span class="dot" aria-label="unread"></span>
              {/if}
            </button>
            {#if sitting.length}
              <button
                class="voice-room"
                class:mine={voice.room === key}
                title={voice.room === key ? undefined : `Join voice in #${name}`}
                disabled={voice.room === key}
                onclick={() => voice.join(key)}
              >
                {#each sitting as m (m.user)}
                  <span class="sitter" class:quiet={!m.live}>
                    <Avatar name={m.user} size={20} speaking={voice.speaking.has(m.user)} />
                    <span class="name">{m.user}</span>
                    {#if m.muted}<span class="muted" title="Muted"><Icon name="mic-off" size={14} /></span>{/if}
                  </span>
                {/each}
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    </section>

    <section>
      <div class="heading"><h2>Direct messages</h2></div>
      {#if porch.dms.length === 0}
        <p class="empty">Pick someone from the porch list to start one.</p>
      {/if}
      <ul>
        {#each porch.dms as key (key)}
          {@const conv = porch.convs[key]}
          {@const user = label(key)}
          <li>
            <button
              class="row person"
              class:current={porch.current === key}
              class:unread={conv && conv.lastId > conv.lastRead}
              aria-current={porch.current === key ? "page" : undefined}
              onclick={() => go(key)}
            >
              <Avatar name={user} size={24} lit={porch.online.has(user)} />
              <span class="name">{user}</span>
              {#if conv?.attention}
                <span class="badge">{conv.attention}</span>
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    </section>
  </div>

  {#if voice.phase !== "idle" && voice.room}
    <div class="voice-bar" role="status">
      <span class="voice-dot" class:live={voice.phase === "live"} aria-hidden="true"></span>
      <div class="voice-text">
        <div class="voice-state">{voice.phase === "live" ? "Voice connected" : "Joining voice…"}</div>
        <button class="voice-where" onclick={() => go(voice.room!)}>#{label(voice.room)}</button>
      </div>
      <button
        class="icon"
        class:off={voice.muted}
        title={voice.muted ? "Unmute" : "Mute"}
        aria-label={voice.muted ? "Unmute" : "Mute"}
        aria-pressed={voice.muted}
        onclick={() => voice.toggleMute()}
      >
        <Icon name={voice.muted ? "mic-off" : "mic"} size={18} />
      </button>
      <button class="icon leave" title="Leave voice" aria-label="Leave voice" onclick={() => voice.leave()}>
        <Icon name="hangup" size={18} />
      </button>
    </div>
  {/if}

  <footer class="me">
    <Avatar name={porch.me} size={32} lit={porch.status === "open"} />
    <div class="me-text">
      <div class="me-name">{porch.me}</div>
      <div class="status" class:down={porch.status !== "open"}>
        {porch.status === "open" ? "Connected" : "Reconnecting"}
      </div>
    </div>
    <button class="icon" title="Sign out" aria-label="Sign out" onclick={() => porch.signOut()}>
      <Icon name="out" size={18} />
    </button>
  </footer>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--boards);
    border-right: 1px solid var(--line);
  }

  .porch {
    position: relative;
    display: flex;
    align-items: center;
    gap: 0.8rem;
    padding: 0.9rem 1rem 0.9rem 1rem;
    border-bottom: 1px solid var(--line);
  }

  /* The lantern's light falls across the boards; more company, more light. */
  .porch::before {
    content: "";
    position: absolute;
    inset: 0 0 -3rem 0;
    background: radial-gradient(70% 90% at 2.1rem 45%, rgba(242, 196, 109, 0.16), transparent 70%);
    opacity: var(--glow);
    transition: opacity 900ms ease;
    pointer-events: none;
  }

  .porch > :global(*) {
    position: relative;
  }

  .wordmark {
    font-family: var(--serif);
    font-size: var(--t-title);
    line-height: 1.1;
  }

  .who {
    font-size: var(--t-small);
    color: var(--faint);
    transition: color 600ms;
  }

  .who.lit {
    color: var(--porchlight);
  }

  .lists {
    flex: 1;
    overflow-y: auto;
    padding: 0.75rem 0.5rem;
  }

  section + section {
    margin-top: 1.25rem;
  }

  .heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 0.25rem 0.35rem 0.6rem;
  }

  h2 {
    margin: 0;
    font-size: var(--t-small);
    font-weight: 600;
    color: var(--muted);
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    width: 100%;
    min-height: 2.1rem;
    padding: 0.3rem 0.6rem;
    border: none;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    text-align: left;
    transition: background 100ms, color 100ms;
  }

  .row:hover {
    background: var(--raise);
    color: var(--text);
  }

  .row.unread {
    color: var(--text);
    font-weight: 700;
  }

  .row.current {
    background: var(--haint-wash);
    color: var(--haint);
  }

  .glyph {
    display: grid;
    color: var(--faint);
  }

  .current .glyph {
    color: var(--haint);
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text);
  }

  .badge {
    min-width: 1.25rem;
    padding: 0 0.35rem;
    border-radius: 999px;
    background: var(--porchlight);
    color: #2a2112;
    font-size: 0.72rem;
    font-weight: 700;
    line-height: 1.25rem;
    text-align: center;
  }

  .empty {
    margin: 0;
    padding: 0.2rem 0.6rem;
    color: var(--faint);
    font-size: var(--t-small);
  }

  .new {
    padding: 0 0.25rem 0.4rem;
  }

  .new input {
    width: 100%;
    padding: 0.4rem 0.6rem;
    background: var(--sunk);
    border: 1px solid var(--haint);
    border-radius: var(--radius-s);
    outline: none;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 1.9rem;
    height: 1.9rem;
    border: none;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
  }

  .icon:hover {
    background: var(--raise);
    color: var(--text);
  }

  .voice-room {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    width: 100%;
    margin: 0.1rem 0 0.3rem;
    padding: 0.25rem 0.6rem 0.25rem 2.1rem;
    border: none;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    text-align: left;
  }

  .voice-room:not(:disabled):hover {
    background: var(--raise);
    color: var(--text);
  }

  .voice-room:disabled {
    cursor: default;
  }

  .voice-room.mine {
    color: var(--text);
  }

  .sitter {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-height: 1.6rem;
    font-size: var(--t-small);
  }

  .sitter.quiet {
    opacity: 0.55;
  }

  .muted {
    display: grid;
    color: var(--faint);
  }

  .voice-bar {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.6rem 0.5rem 0.6rem 1rem;
    border-top: 1px solid var(--line);
    background: color-mix(in srgb, var(--haint) 6%, var(--sunk));
  }

  .voice-dot {
    flex: none;
    width: 8px;
    height: 8px;
    margin-right: 0.3rem;
    border-radius: 50%;
    background: var(--faint);
  }

  .voice-dot.live {
    background: var(--haint);
    box-shadow: 0 0 8px var(--haint-deep);
  }

  .voice-text {
    flex: 1;
    min-width: 0;
  }

  .voice-state {
    font-size: var(--t-small);
    font-weight: 700;
    color: var(--haint);
  }

  .voice-where {
    display: block;
    max-width: 100%;
    padding: 0;
    border: none;
    background: none;
    color: var(--faint);
    font-size: var(--t-small);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .voice-where:hover {
    color: var(--text);
    text-decoration: underline;
  }

  .icon.off {
    color: var(--error);
  }

  .icon.leave:hover {
    background: var(--error-wash);
    color: var(--error);
  }

  .me {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    padding: 0.7rem 0.75rem 0.7rem 1rem;
    background: var(--sunk);
  }

  .me-text {
    flex: 1;
    min-width: 0;
  }

  .me-name {
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .status {
    font-size: var(--t-small);
    color: var(--faint);
  }

  .status.down {
    color: var(--porchlight);
  }
</style>
