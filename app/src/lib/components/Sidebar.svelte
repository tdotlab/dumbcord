<script>
  import { porch, label } from "../porch.svelte.js";

  let { onnavigate } = $props();

  const voice = porch.voice;
  let newChannel = $state("");

  // Everyone but me, online first, then by name.
  const people = $derived(
    porch.users
      .filter((u) => u !== porch.me)
      .sort((a, b) => Number(porch.online.has(b)) - Number(porch.online.has(a)) || a.localeCompare(b)),
  );

  function go(key) {
    porch.open(key);
    onnavigate?.();
  }

  function addChannel(e) {
    e.preventDefault();
    const name = newChannel.trim().replace(/^#/, "").toLowerCase().replace(/\s+/g, "-");
    newChannel = "";
    if (!name) return;
    if (porch.channels.includes(name)) return go(`#${name}`);
    porch.createChannel(name);
    setTimeout(() => porch.channels.includes(name) && go(`#${name}`), 300);
  }
</script>

<nav class="sidebar" aria-label="Conversations">
  <div class="lists">
    <section aria-labelledby="channels-head">
      <h2 class="head" id="channels-head">Channels</h2>
      <ul>
        {#each porch.channels as name (name)}
          {@const key = `#${name}`}
          {@const conv = porch.convs[key]}
          <li>
            <button
              type="button"
              class="row"
              class:current={porch.current === key}
              class:unread={porch.unread(key)}
              aria-current={porch.current === key ? "page" : undefined}
              onclick={() => go(key)}
            >
              <span class="name">#{name}</span>
              {#if conv?.attention}<span class="count" aria-label="{conv.attention} for you">{conv.attention}</span>{/if}
            </button>
            {#if voice.inRoom(key).length}
              <ul class="sitters" aria-label="In voice in #{name}">
                {#each voice.inRoom(key) as m (m.user)}
                  <li class="sitter">
                    <span
                      class="light"
                      class:speaking={voice.speaking.has(m.user)}
                      class:live={m.live}
                      aria-hidden="true"
                    ></span>
                    <span class="who">{m.user}</span>
                    {#if m.muted}<span class="muted">muted</span>{/if}
                  </li>
                {/each}
              </ul>
            {/if}
          </li>
        {/each}
      </ul>
      <form class="new" onsubmit={addChannel}>
        <input class="field" bind:value={newChannel} placeholder="new-channel" maxlength="32" aria-label="New channel name" />
        <button class="btn" type="submit" title="Create channel" aria-label="Create channel" disabled={!newChannel.trim()}>+</button>
      </form>
    </section>

    <hr class="sep" />

    <section aria-labelledby="people-head">
      <h2 class="head" id="people-head">People</h2>
      {#if people.length === 0}
        <p class="empty">Nobody else yet. Give a friend the invite code.</p>
      {:else}
        <ul>
          {#each people as user (user)}
            {@const key = `@${user}`}
            {@const conv = porch.convs[key]}
            {@const on = porch.online.has(user)}
            <li>
              <button
                type="button"
                class="row"
                class:current={porch.current === key}
                class:unread={porch.unread(key)}
                class:away={!on}
                aria-current={porch.current === key ? "page" : undefined}
                onclick={() => go(key)}
              >
                <span class="dot" class:on aria-hidden="true"></span>
                <span class="name">{user}</span>
                <span class="visually-hidden">{on ? "online" : "offline"}</span>
                {#if conv?.attention}<span class="count" aria-label="{conv.attention} unread">{conv.attention}</span>{/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  </div>

  {#if voice.phase !== "idle" && voice.room}
    <section class="voice" aria-label="Voice">
      <div class="voice-line">
        <span class="state" class:live={voice.phase === "live"}>
          {voice.phase === "live" ? "Voice connected" : "Joining voice..."}
        </span>
        <button type="button" class="where" onclick={() => go(voice.room)}>#{label(voice.room)}</button>
      </div>
      <div class="voice-buttons">
        <button type="button" class="btn" class:on={voice.muted} aria-pressed={voice.muted} onclick={() => voice.toggleMute()}>
          {voice.muted ? "Unmute" : "Mute"}
        </button>
        <button type="button" class="btn" onclick={() => voice.leave()}>Leave</button>
      </div>
    </section>
  {/if}
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .lists {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 12px 8px 12px;
  }

  section > .head {
    padding: 0 4px 4px;
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 100%;
    min-height: 24px;
    margin: 1px 0;
    padding: 1px 5px;
    border: 0;
    border-radius: var(--radius);
    background: none;
    color: var(--text);
    text-align: left;
  }

  .row:hover {
    background: var(--hover);
    color: var(--strong);
  }

  .row.unread {
    color: var(--strong);
    font-weight: 500;
  }

  .row.away {
    color: var(--weak);
  }

  .row.current {
    background: var(--select);
    color: var(--select-text);
  }

  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    min-width: 20px;
    padding: 0 5px;
    border-radius: var(--radius);
    background: var(--attention);
    color: #1b1b1b;
    font-size: 12px;
    font-weight: 500;
    line-height: 18px;
    text-align: center;
  }

  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border: 1px solid var(--weak);
    border-radius: 50%;
  }

  .dot.on {
    border-color: var(--live);
    background: var(--live);
  }

  .sitters {
    margin: 0 0 4px;
    padding-left: 18px;
  }

  .sitter {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 20px;
    font-size: 13px;
  }

  /* A small square that turns green while that person talks. */
  .light {
    flex: none;
    width: 8px;
    height: 8px;
    border: 1px solid var(--weak);
  }

  .light.live {
    border-color: var(--live);
  }

  .light.speaking {
    background: var(--live);
  }

  .who {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .muted {
    color: var(--weak);
    font-size: 12px;
  }

  .new {
    display: flex;
    gap: 6px;
    margin: 8px 0 0;
    padding: 0 2px;
  }

  .new .btn {
    flex: none;
    width: 28px;
    padding: 0;
    font-size: 16px;
  }

  .sep {
    margin: 12px 4px;
  }

  .empty {
    margin: 0;
    padding: 0 6px;
    font-size: 13px;
    color: var(--weak);
  }

  .voice {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px var(--pad);
    padding-bottom: max(10px, env(safe-area-inset-bottom));
    border-top: 1px solid var(--line);
  }

  .voice-line {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-size: 13px;
  }

  .state {
    color: var(--attention);
  }

  .state.live {
    color: var(--live);
  }

  .where {
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    color: var(--strong);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .where:hover {
    text-decoration: underline;
  }

  .voice-buttons {
    display: flex;
    gap: 6px;
  }

  .voice-buttons .btn {
    flex: 1;
  }
</style>
