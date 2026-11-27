<script>
  import { onMount, untrack } from "svelte";
  import { porch, label } from "../porch.svelte.js";
  import { dayLabel, sameDay } from "../time.js";
  import Message from "./Message.svelte";

  let { conv } = $props();

  const GROUP_MS = 5 * 60_000;
  const NEAR_BOTTOM = 48;
  const NEAR_TOP = 400;

  let scroller = $state();
  let content = $state();
  let stuck = $state(true);

  // Where "new" started when this conversation was opened.
  const readMark = untrack(() => conv.lastRead);

  const rows = $derived.by(() => {
    const out = [];
    let freshShown = false;
    conv.messages.forEach((msg, i) => {
      const prev = conv.messages[i - 1];
      const newDay = !prev || !sameDay(prev.created_at, msg.created_at);
      const fresh = !freshShown && readMark > 0 && msg.id > readMark && msg.author !== porch.me && !msg.state;
      if (fresh) freshShown = true;
      const continued =
        !!prev && !newDay && !fresh && prev.author === msg.author && msg.created_at - prev.created_at < GROUP_MS;
      out.push({ msg, continued, day: newDay ? dayLabel(msg.created_at) : null, fresh });
    });
    return out;
  });

  const isDm = $derived(conv.key.startsWith("@"));
  const hasNew = $derived(conv.lastId > conv.lastRead);

  function nearBottom(el) {
    return el.scrollHeight - el.scrollTop - el.clientHeight < NEAR_BOTTOM;
  }

  function toBottom() {
    scroller?.scrollTo({ top: scroller.scrollHeight, behavior: "instant" });
  }

  function onScroll() {
    if (!scroller) return;
    stuck = nearBottom(scroller);
    if (scroller.scrollTop < NEAR_TOP) porch.loadOlder(conv.key);
  }

  // Keep the reader's place when an older page lands above them.
  let snapHeight = 0;
  let snapTop = 0;
  let renderedFirst;

  $effect.pre(() => {
    void conv.messages.length;
    void conv.messages[0]?.id;
    if (scroller) {
      snapHeight = scroller.scrollHeight;
      snapTop = scroller.scrollTop;
    }
  });

  $effect(() => {
    void conv.messages.length;
    const first = conv.messages[0]?.id;
    if (!scroller) return;
    const prepended = renderedFirst !== undefined && first !== renderedFirst;
    renderedFirst = first;
    if (untrack(() => stuck)) toBottom();
    else if (prepended) scroller.scrollTop = snapTop + (scroller.scrollHeight - snapHeight);
  });

  // A short page that doesn't fill the screen can't be scrolled up: fetch more.
  $effect(() => {
    if (!scroller || !conv.loaded || conv.loading || !conv.hasMore) return;
    void conv.messages.length;
    if (scroller.scrollHeight <= scroller.clientHeight + NEAR_TOP) porch.loadOlder(conv.key);
  });

  // Reading the bottom of a conversation marks it read.
  $effect(() => {
    void conv.lastId;
    if (stuck && conv.loaded && document.visibilityState === "visible") {
      porch.markSeen(conv.key);
    }
  });

  onMount(() => {
    toBottom();
    // Late font loads and edits change heights; stay pinned if pinned.
    const ro = new ResizeObserver(() => {
      if (stuck) toBottom();
    });
    if (content) ro.observe(content);
    const seen = () => {
      if (stuck && document.visibilityState === "visible") porch.markSeen(conv.key);
    };
    window.addEventListener("focus", seen);
    document.addEventListener("visibilitychange", seen);
    return () => {
      ro.disconnect();
      window.removeEventListener("focus", seen);
      document.removeEventListener("visibilitychange", seen);
    };
  });
</script>

<div class="scroller" bind:this={scroller} onscroll={onScroll}>
  <div class="content" bind:this={content}>
    {#if conv.loaded && !conv.hasMore}
      <p class="start">
        {#if isDm}
          Start of your conversation with {label(conv.key)}.
        {:else if conv.messages.length === 0}
          Nothing in #{label(conv.key)} yet. Say something.
        {:else}
          Start of #{label(conv.key)}.
        {/if}
      </p>
    {:else if conv.loading || conv.hasMore}
      <p class="start" aria-live="polite">{conv.loaded ? "Loading older messages..." : "Loading messages..."}</p>
    {/if}

    {#each rows as row (row.msg.nonce ?? row.msg.id)}
      {#if row.day}
        <div class="divider" role="separator"><span>{row.day}</span></div>
      {/if}
      {#if row.fresh}
        <div class="divider fresh" role="separator"><span>New</span></div>
      {/if}
      <Message message={row.msg} key={conv.key} continued={row.continued} />
    {/each}
  </div>
</div>

{#if !stuck}
  <div class="behind" class:new={hasNew}>
    <span>{hasNew ? "New messages below" : "Viewing older messages"}</span>
    <button class="btn" type="button" onclick={toBottom}>Jump to latest</button>
  </div>
{/if}

<style>
  .scroller {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-anchor: none;
    overscroll-behavior: contain;
  }

  .content {
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    min-height: 100%;
    padding: 12px 0 8px;
  }

  .start {
    margin: 12px 18px 4px;
    font-size: 13px;
    color: var(--weak);
  }

  /* Left-labelled rule: "Today ----------". */
  .divider {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 16px 18px 2px;
    font-size: 12px;
    color: var(--weak);
  }

  .divider::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--line);
  }

  .divider.fresh {
    color: var(--attention);
  }

  .divider.fresh::after {
    background: var(--attention);
    opacity: 0.6;
  }

  .divider + .divider.fresh {
    margin-top: 4px;
  }

  .behind {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: none;
    min-height: 36px;
    padding: 4px var(--pad) 4px 18px;
    border-top: 1px solid var(--line);
    font-size: 13px;
    color: var(--weak);
  }

  .behind span {
    flex: 1;
  }

  .behind.new {
    color: var(--attention);
  }
</style>
