<script>
  import { porch, label } from "../porch.svelte.js";
  import Composer from "./Composer.svelte";
  import MessageList from "./MessageList.svelte";

  const voice = porch.voice;
  const key = $derived(porch.current);
  const conv = $derived(porch.convs[key]);
  const isDm = $derived(key.startsWith("@"));
  const peer = $derived(label(key));
  const here = $derived(voice.room === key && voice.phase !== "idle");
  const sitting = $derived(voice.inRoom(key).length);
</script>

<header class="top">
  {#if isDm}
    <h1>@{peer}</h1>
    <span class="sub" class:on={porch.online.has(peer)}>{porch.online.has(peer) ? "online" : "offline"}</span>
  {:else}
    <h1>#{peer}</h1>
    {#if voice.enabled}
      {#if sitting && !here}<span class="sub">{sitting} in voice</span>{/if}
      <span class="grow"></span>
      <button
        type="button"
        class="btn"
        class:on={here}
        onclick={() => (here ? voice.leave() : voice.join(key))}
      >
        {here ? "Leave voice" : "Join voice"}
      </button>
    {/if}
  {/if}
</header>

{#if conv}
  {#key key}
    <div class="feed">
      <MessageList {conv} />
    </div>
    <Composer {key} />
  {/key}
{/if}

<style>
  .top {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: none;
    min-height: 44px;
    padding: 4px var(--pad) 4px 18px;
    border-bottom: 1px solid var(--line);
  }

  h1 {
    min-width: 0;
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 20px;
    font-weight: 300;
    color: var(--strong);
  }

  .sub {
    flex: none;
    font-size: 13px;
    color: var(--weak);
  }

  .sub.on {
    color: var(--live);
  }

  .grow {
    flex: 1;
  }

  .feed {
    position: relative;
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
</style>
