<script lang="ts">
  import { porch, type Msg } from "../porch.svelte";
  import { mentions, render } from "../markdown";
  import { openExternal } from "../platform";
  import { fullTime, timeOf } from "../time";
  import Avatar from "./Avatar.svelte";
  import Icon from "./Icon.svelte";

  let {
    message,
    key,
    continued,
  }: { message: Msg; key: string; continued: boolean } = $props();

  const own = $derived(message.author === porch.me);
  const html = $derived(render(message.text));
  const forMe = $derived(!own && mentions(message.text, porch.me));
  const editing = $derived(porch.editing === message.id);

  let confirming = $state(false);
  let editText = $state("");

  $effect(() => {
    if (editing) editText = message.text;
  });

  function onBodyClick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (a?.href) {
      e.preventDefault();
      void openExternal(a.href);
      return;
    }
    const m = (e.target as HTMLElement).closest<HTMLElement>(".mention");
    const user = m?.dataset.user;
    if (user && user !== porch.me) porch.open(`@${user}`);
  }

  function onEditKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      porch.editing = null;
    } else if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      save();
    }
  }

  function save() {
    if (editText.trim() === message.text) porch.editing = null;
    else if (!editText.trim()) {
      porch.editing = null;
      confirming = true;
    } else porch.edit(message.id, editText);
  }

  function onTrash(e: MouseEvent) {
    if (e.shiftKey) porch.remove(message.id);
    else confirming = true;
  }

  function grow(node: HTMLTextAreaElement) {
    const fit = () => {
      node.style.height = "auto";
      node.style.height = `${node.scrollHeight}px`;
    };
    fit();
    node.focus();
    node.setSelectionRange(node.value.length, node.value.length);
    node.addEventListener("input", fit);
    return { destroy: () => node.removeEventListener("input", fit) };
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<article
  class="msg"
  class:continued
  class:for-me={forMe}
  class:pending={message.state === "pending"}
  class:failed={message.state === "failed"}
  class:editing
  tabindex={own ? -1 : undefined}
>
  <div class="gutter">
    {#if continued}
      <time class="hover-time" datetime={new Date(message.created_at).toISOString()} title={fullTime(message.created_at)}>
        {timeOf(message.created_at)}
      </time>
    {:else}
      <Avatar name={message.author} size={36} />
    {/if}
  </div>

  <div class="content">
    {#if !continued}
      <header>
        <span class="author">{message.author}</span>
        <time datetime={new Date(message.created_at).toISOString()} title={fullTime(message.created_at)}>
          {timeOf(message.created_at)}
        </time>
      </header>
    {/if}

    {#if editing}
      <div class="edit">
        <textarea use:grow bind:value={editText} onkeydown={onEditKey} rows="1" maxlength="2000" aria-label="Edit message"
        ></textarea>
        <p class="hint">
          Escape to <button type="button" onclick={() => (porch.editing = null)}>cancel</button>, Enter to
          <button type="button" onclick={save}>save</button>
        </p>
      </div>
    {:else}
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="body" onclick={onBodyClick}>
        {@html html}
        {#if message.edited_at}<span class="edited" title={fullTime(message.edited_at)}>(edited)</span>{/if}
      </div>
    {/if}

    {#if message.state === "failed" && message.nonce}
      {@const nonce = message.nonce}
      <p class="problem">
        Didn't send.
        <button type="button" onclick={() => porch.retry(key, nonce)}><Icon name="retry" size={14} /> Try again</button>
        <button type="button" onclick={() => porch.discard(key, nonce)}>Discard</button>
      </p>
    {/if}

    {#if confirming}
      <p class="confirm">
        Delete this message?
        <button type="button" class="danger" onclick={() => { confirming = false; porch.remove(message.id); }}>Delete</button>
        <button type="button" onclick={() => (confirming = false)}>Keep it</button>
      </p>
    {/if}
  </div>

  {#if own && !message.state && !editing && !confirming}
    <div class="actions">
      <button type="button" title="Edit" aria-label="Edit message" onclick={() => (porch.editing = message.id)}>
        <Icon name="edit" size={16} />
      </button>
      <button type="button" title="Delete (Shift-click skips asking)" aria-label="Delete message" onclick={onTrash}>
        <Icon name="trash" size={16} />
      </button>
    </div>
  {/if}
</article>

<style>
  .msg {
    position: relative;
    display: grid;
    grid-template-columns: 3.25rem 1fr;
    padding: 0.35rem 1.25rem 0.15rem 0.75rem;
    margin-top: 0.85rem;
    border-left: 2px solid transparent;
  }

  .msg.continued {
    margin-top: 0;
    padding-top: 0.08rem;
  }

  .msg:focus {
    outline: none;
  }

  .msg:hover {
    background: rgba(255, 255, 255, 0.022);
  }

  /* Someone said your name: the porchlight finds the line. */
  .msg.for-me {
    background: var(--porchlight-wash);
    border-left-color: var(--porchlight);
  }

  .msg.pending .body {
    opacity: 0.55;
  }

  .gutter {
    display: flex;
    justify-content: center;
    padding-top: 0.1rem;
  }

  .hover-time {
    visibility: hidden;
    padding-top: 0.2rem;
    font-size: 0.68rem;
    color: var(--faint);
    white-space: nowrap;
  }

  .msg:hover .hover-time {
    visibility: visible;
  }

  .content {
    min-width: 0;
  }

  header {
    display: flex;
    align-items: baseline;
    gap: 0.55rem;
    line-height: 1.3;
  }

  .author {
    font-weight: 700;
  }

  header time {
    font-size: 0.72rem;
    color: var(--faint);
  }

  .body {
    overflow-wrap: anywhere;
    font-size: var(--t-body);
    line-height: 1.5;
  }

  .body :global(p) {
    margin: 0;
  }

  /* "(edited)" sits at the end of the last line, not under it. */
  .body > :global(p:last-of-type) {
    display: inline;
  }

  .body :global(p + p) {
    margin-top: 0.35rem;
  }

  .body :global(a) {
    color: var(--haint);
    text-decoration: underline;
    text-decoration-color: rgba(159, 216, 203, 0.4);
    text-underline-offset: 2px;
  }

  .body :global(a:hover) {
    text-decoration-color: currentColor;
  }

  .body :global(code) {
    padding: 0.1em 0.35em;
    border-radius: 4px;
    background: var(--sunk);
    font-family: var(--mono);
    font-size: 0.86em;
  }

  .body :global(pre) {
    margin: 0.35rem 0;
    padding: 0.6rem 0.8rem;
    max-width: 100%;
    overflow-x: auto;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--sunk);
  }

  .body :global(pre code) {
    padding: 0;
    background: none;
  }

  .body :global(blockquote) {
    margin: 0.2rem 0;
    padding-left: 0.75rem;
    border-left: 3px solid var(--line-strong);
    color: var(--muted);
  }

  .body :global(ul),
  .body :global(ol) {
    margin: 0.2rem 0;
    padding-left: 1.4rem;
  }

  .body :global(hr) {
    border: none;
    border-top: 1px solid var(--line-strong);
  }

  .body :global(.mention) {
    padding: 0 0.2em;
    border-radius: 4px;
    background: var(--haint-wash);
    color: var(--haint);
    font-weight: 600;
    cursor: pointer;
  }

  .edited {
    margin-left: 0.3rem;
    font-size: 0.7rem;
    color: var(--faint);
  }

  .edit textarea {
    display: block;
    width: 100%;
    resize: none;
    padding: 0.5rem 0.65rem;
    border: 1px solid var(--haint);
    border-radius: var(--radius-s);
    background: var(--sunk);
    outline: none;
    line-height: 1.5;
  }

  .hint,
  .problem,
  .confirm {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
    margin: 0.25rem 0 0;
    font-size: var(--t-small);
    color: var(--muted);
  }

  .hint {
    display: block;
  }

  .hint button {
    padding: 0;
    border: none;
    background: none;
    color: var(--haint);
  }

  .problem {
    color: var(--error);
  }

  .problem button,
  .confirm button {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.15rem 0.55rem;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-s);
    background: var(--raise);
    color: var(--text);
    font-size: var(--t-small);
  }

  .confirm .danger {
    border-color: transparent;
    background: var(--error);
    color: #2a1410;
    font-weight: 700;
  }

  .actions {
    position: absolute;
    top: -0.9rem;
    right: 1rem;
    display: none;
    padding: 2px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-s);
    background: var(--raise);
    box-shadow: 0 6px 16px -6px rgba(0, 0, 0, 0.5);
  }

  .msg:hover .actions,
  .msg:focus-within .actions {
    display: flex;
  }

  .actions button {
    display: grid;
    place-items: center;
    width: 1.85rem;
    height: 1.85rem;
    border: none;
    border-radius: 4px;
    background: none;
    color: var(--muted);
  }

  .actions button:hover {
    background: var(--raise-hover);
    color: var(--text);
  }
</style>
