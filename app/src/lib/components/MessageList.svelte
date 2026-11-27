<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { porch, label, type Conv } from "../porch.svelte";
  import { dayLabel, sameDay } from "../time";
  import Icon from "./Icon.svelte";
  import Message from "./Message.svelte";

  let { conv }: { conv: Conv } = $props();

  const GROUP_MS = 5 * 60_000;
  const NEAR_BOTTOM = 48;
  const NEAR_TOP = 400;

  let scroller: HTMLDivElement | undefined = $state();
  let content: HTMLDivElement | undefined = $state();
  let stuck = $state(true);

  // Where "new" started when this conversation was opened.
  const readMark = untrack(() => conv.lastRead);

  const rows = $derived.by(() => {
    const out: { msg: (typeof conv.messages)[number]; continued: boolean; day: string | null; fresh: boolean }[] = [];
    let freshShown = false;
    conv.messages.forEach((msg, i) => {
      const prev = conv.messages[i - 1];
      const newDay = !prev || !sameDay(prev.created_at, msg.created_at);
      const fresh =
        !freshShown && readMark > 0 && msg.id > readMark && msg.author !== porch.me && !msg.state;
      if (fresh) freshShown = true;
      const continued =
        !!prev && !newDay && !fresh && prev.author === msg.author && msg.created_at - prev.created_at < GROUP_MS;
      out.push({ msg, continued, day: newDay ? dayLabel(msg.created_at) : null, fresh });
    });
    return out;
  });

  const isDm = $derived(conv.key.startsWith("@"));
  const hasNew = $derived(conv.lastId > conv.lastRead);

  function nearBottom(el: HTMLElement) {
    return el.scrollHeight - el.scrollTop - el.clientHeight < NEAR_BOTTOM;
  }

  function toBottom(smooth = false) {
    scroller?.scrollTo({ top: scroller.scrollHeight, behavior: smooth ? "smooth" : "instant" });
  }

  function onScroll() {
    if (!scroller) return;
    stuck = nearBottom(scroller);
    if (scroller.scrollTop < NEAR_TOP) porch.loadOlder(conv.key);
  }

  // Keep the reader's place when an older page lands above them.
  let snapHeight = 0;
  let snapTop = 0;
  let renderedFirst: number | undefined;

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
      <header class="start">
        {#if isDm}
          <h2>{label(conv.key)}</h2>
          <p>This is the start of your conversation with {label(conv.key)}.</p>
        {:else}
          <h2>#{label(conv.key)}</h2>
          <p>
            {conv.messages.length === 0
              ? "Nobody has said anything here yet. Go on."
              : `This is the very beginning of #${label(conv.key)}.`}
          </p>
        {/if}
      </header>
    {:else if conv.loading || conv.hasMore}
      <p class="loading" aria-live="polite">{conv.loaded ? "Loading older messages" : "Loading messages"}</p>
    {/if}

    {#each rows as row (row.msg.nonce ?? row.msg.id)}
      {#if row.day}
        <div class="divider day" role="separator"><span>{row.day}</span></div>
      {/if}
      {#if row.fresh}
        <div class="divider fresh" role="separator"><span>New</span></div>
      {/if}
      <Message message={row.msg} key={conv.key} continued={row.continued} />
    {/each}
    <div class="tail"></div>
  </div>
</div>

{#if !stuck}
  <button class="jump" class:new={hasNew} type="button" onclick={() => toBottom(true)}>
    {hasNew ? "New messages" : "Jump to present"}
    <Icon name="down" size={15} />
  </button>
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
    padding: 1rem 0 0.25rem;
  }

  .start {
    padding: 2.5rem 1.25rem 0.5rem 1.6rem;
  }

  .start h2 {
    margin: 0 0 0.25rem;
    font-family: var(--serif);
    font-weight: 400;
    font-size: 1.75rem;
    line-height: 1.15;
  }

  .start p {
    margin: 0;
    color: var(--muted);
  }

  .loading {
    margin: 1rem auto;
    color: var(--faint);
    font-size: var(--t-small);
  }

  .divider {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin: 1.25rem 1.25rem 0.25rem;
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--faint);
  }

  .divider::before,
  .divider::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--line-strong);
  }

  .divider.fresh {
    color: var(--porchlight);
  }

  .divider.fresh::before,
  .divider.fresh::after {
    background: rgba(242, 196, 109, 0.4);
  }

  .divider.day + .divider.fresh {
    margin-top: 0.25rem;
  }

  .tail {
    height: 0.5rem;
  }

  .jump {
    position: absolute;
    left: 50%;
    bottom: 0.75rem;
    z-index: 2;
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.4rem 0.85rem;
    transform: translateX(-50%);
    border: 1px solid var(--line-strong);
    border-radius: 999px;
    background: var(--raise);
    color: var(--text);
    font-size: var(--t-small);
    font-weight: 600;
    box-shadow: 0 10px 24px -10px rgba(0, 0, 0, 0.6);
  }

  .jump.new {
    border-color: transparent;
    background: var(--porchlight);
    color: #2a2112;
  }
</style>
