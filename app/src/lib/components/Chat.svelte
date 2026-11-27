<script lang="ts">
  import { porch, label } from "../porch.svelte";
  import Avatar from "./Avatar.svelte";
  import Composer from "./Composer.svelte";
  import Icon from "./Icon.svelte";
  import MessageList from "./MessageList.svelte";

  let {
    onmenu,
    onpeople,
    peopleOpen,
  }: { onmenu: () => void; onpeople: () => void; peopleOpen: boolean } = $props();

  const key = $derived(porch.current);
  const conv = $derived(porch.convs[key]);
  const isDm = $derived(key.startsWith("@"));
  const peer = $derived(label(key));
  const voice = porch.voice;
  const inVoiceHere = $derived(voice.room === key && voice.phase !== "idle");
  const elsewhere = $derived(
    Object.values(porch.convs).filter((c) => c.key !== key && c.attention > 0).length,
  );
</script>

<section class="chat" aria-label={isDm ? `Conversation with ${peer}` : `#${peer}`}>
  <header class="bar">
    <button class="icon menu" type="button" aria-label="Conversations" onclick={onmenu}>
      <Icon name="menu" size={20} />
      {#if elsewhere}<span class="pip" aria-label="{elsewhere} waiting"></span>{/if}
    </button>

    <div class="title">
      {#if isDm}
        <Avatar name={peer} size={26} lit={porch.online.has(peer)} />
        <h1>{peer}</h1>
        <span class="sub">{porch.online.has(peer) ? "On the porch" : "Inside"}</span>
      {:else}
        <span class="hash"><Icon name="hash" size={20} /></span>
        <h1>{peer}</h1>
      {/if}
    </div>

    {#if !isDm && voice.enabled}
      <button
        class="icon voice"
        class:on={inVoiceHere}
        type="button"
        title={inVoiceHere ? "Leave voice" : `Join voice in #${peer}`}
        aria-label={inVoiceHere ? "Leave voice" : "Join voice"}
        aria-pressed={inVoiceHere}
        onclick={() => (inVoiceHere ? voice.leave() : voice.join(key))}
      >
        <Icon name="headphones" size={20} />
        {#if voice.inRoom(key).length && !inVoiceHere}
          <span class="count">{voice.inRoom(key).length}</span>
        {/if}
      </button>
    {/if}

    <button
      class="icon people"
      class:on={peopleOpen}
      type="button"
      aria-label="People"
      aria-pressed={peopleOpen}
      onclick={onpeople}
    >
      <Icon name="people" size={20} />
    </button>
  </header>

  {#if porch.status !== "open"}
    <div class="banner" role="status">
      <span class="pulse" aria-hidden="true"></span>
      Lost the connection. Reconnecting…
    </div>
  {/if}

  <div class="stage">
    {#if conv}
      {#key key}
        <MessageList {conv} />
      {/key}
    {/if}
  </div>

  {#key key}
    <Composer {key} />
  {/key}
</section>

<style>
  .chat {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-width: 0;
    background: var(--night);
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    height: 3.5rem;
    flex: none;
    padding: 0 0.75rem 0 1.25rem;
    border-bottom: 1px solid var(--line);
  }

  .title {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex: 1;
    min-width: 0;
  }

  h1 {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .hash {
    display: grid;
    color: var(--faint);
  }

  .sub {
    font-size: var(--t-small);
    color: var(--faint);
  }

  .icon {
    position: relative;
    display: grid;
    place-items: center;
    width: 2.25rem;
    height: 2.25rem;
    flex: none;
    border: none;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
  }

  .icon:hover {
    background: var(--raise);
    color: var(--text);
  }

  .icon.on {
    color: var(--haint);
  }

  .menu {
    display: none;
    margin-left: -0.6rem;
  }

  .count {
    position: absolute;
    top: 0.2rem;
    right: 0.1rem;
    min-width: 1rem;
    padding: 0 0.2rem;
    border-radius: 999px;
    background: var(--porchlight);
    color: #2a2112;
    font-size: 0.65rem;
    font-weight: 700;
    line-height: 1rem;
    box-shadow: 0 0 0 2px var(--night);
  }

  .pip {
    position: absolute;
    top: 0.35rem;
    right: 0.35rem;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--porchlight);
    box-shadow: 0 0 0 2px var(--night);
  }

  .banner {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.45rem 1.25rem;
    background: var(--raise);
    color: var(--muted);
    font-size: var(--t-small);
  }

  .pulse {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--muted);
    animation: pulse 1.4s ease-in-out infinite;
  }

  @keyframes pulse {
    50% { opacity: 0.25; }
  }

  .stage {
    position: relative;
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }

  @media (max-width: 760px) {
    .menu {
      display: grid;
    }

    .bar {
      padding-left: 1rem;
    }
  }
</style>
