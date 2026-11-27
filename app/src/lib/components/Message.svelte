<script>
  import { porch } from "../porch.svelte.js";
  import { mentions, render } from "../markdown.js";
  import { openExternal } from "../platform.js";
  import { fullTime, timeOf } from "../time.js";

  let { message, key, continued } = $props();

  const own = $derived(message.author === porch.me);
  const html = $derived(render(message.text));
  const forMe = $derived(!own && mentions(message.text, porch.me));
  const editing = $derived(porch.editing === message.id);
  const iso = $derived(new Date(message.created_at).toISOString());

  let confirming = $state(false);
  let editText = $state("");

  $effect(() => {
    if (editing) editText = message.text;
  });

  function onBodyClick(e) {
    const a = e.target.closest("a");
    if (a?.href) {
      e.preventDefault();
      void openExternal(a.href);
      return;
    }
    const user = e.target.closest(".mention")?.dataset.user;
    if (user && user !== porch.me) porch.open(`@${user}`);
  }

  function onEditKey(e) {
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

  function onDelete(e) {
    if (e.shiftKey) porch.remove(message.id);
    else confirming = true;
  }

  function grow(node) {
    const fit = () => {
      node.style.height = "auto";
      node.style.height = `${node.scrollHeight + 2}px`;
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
  tabindex={own ? -1 : undefined}
>
  {#if !continued}
    <header>
      <span class="author">{message.author}</span>
      <time datetime={iso} title={fullTime(message.created_at)}>{timeOf(message.created_at)}</time>
    </header>
  {/if}

  {#if editing}
    <div class="edit">
      <textarea class="field" use:grow bind:value={editText} onkeydown={onEditKey} rows="1" maxlength="2000" aria-label="Edit message"
      ></textarea>
      <div class="line">
        <button type="button" class="btn small" onclick={save}>Save</button>
        <button type="button" class="btn small" onclick={() => (porch.editing = null)}>Cancel</button>
        <span class="hint">Enter saves, Escape cancels</span>
      </div>
    </div>
  {:else}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="body" onclick={onBodyClick} title={continued ? fullTime(message.created_at) : undefined}>
      {@html html}
      {#if message.edited_at}<span class="edited" title={fullTime(message.edited_at)}>(edited)</span>{/if}
    </div>
  {/if}

  {#if message.state === "failed" && message.nonce}
    {@const nonce = message.nonce}
    <div class="line problem">
      <span>Didn't send.</span>
      <button type="button" class="btn small" onclick={() => porch.retry(key, nonce)}>Try again</button>
      <button type="button" class="btn small" onclick={() => porch.discard(key, nonce)}>Discard</button>
    </div>
  {/if}

  {#if confirming}
    <div class="line">
      <span>Delete this message?</span>
      <button
        type="button"
        class="btn small danger"
        onclick={() => {
          confirming = false;
          porch.remove(message.id);
        }}>Delete</button
      >
      <button type="button" class="btn small" onclick={() => (confirming = false)}>Keep it</button>
    </div>
  {/if}

  {#if own && !message.state && !editing && !confirming}
    <div class="actions">
      <button type="button" class="btn small" onclick={() => (porch.editing = message.id)}>Edit</button>
      <button type="button" class="btn small" title="Shift-click deletes without asking" onclick={onDelete}>Delete</button>
    </div>
  {/if}
</article>

<style>
  .msg {
    position: relative;
    margin-top: 8px;
    padding: 1px var(--pad) 1px 16px;
    border-left: 2px solid transparent;
  }

  .msg.continued {
    margin-top: 0;
  }

  .msg:hover {
    background: rgba(255, 255, 255, 0.025);
  }

  .msg:focus {
    outline: none;
  }

  /* Someone said your name. */
  .msg.for-me {
    border-left-color: var(--attention);
  }

  .msg.pending .body {
    opacity: 0.5;
  }

  header {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .author {
    font-weight: 500;
    color: var(--strong);
  }

  time {
    font-size: 11px;
    color: var(--weak);
  }

  .body {
    overflow-wrap: anywhere;
  }

  .body :global(p) {
    margin: 0;
  }

  /* "(edited)" sits at the end of the last line, not under it. */
  .body > :global(p:last-of-type) {
    display: inline;
  }

  .body :global(p + p) {
    margin-top: 4px;
  }

  .body :global(strong) {
    font-weight: 500;
    color: var(--strong);
  }

  .body :global(a) {
    color: var(--link);
    text-underline-offset: 2px;
  }

  .body :global(code) {
    padding: 0 4px;
    border-radius: var(--radius);
    background: var(--well);
    font-family: var(--mono);
    font-size: 0.88em;
    color: var(--strong);
  }

  .body :global(pre) {
    margin: 4px 0;
    padding: 8px 10px;
    max-width: 100%;
    overflow-x: auto;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--well);
  }

  .body :global(pre code) {
    padding: 0;
    background: none;
  }

  .body :global(blockquote) {
    margin: 2px 0;
    padding-left: 10px;
    border-left: 2px solid var(--line);
    color: var(--weak);
  }

  .body :global(ul),
  .body :global(ol) {
    margin: 2px 0;
    padding-left: 22px;
  }

  .body :global(hr) {
    border: 0;
    border-top: 1px solid var(--line);
  }

  .body :global(.mention) {
    color: var(--link);
    font-weight: 500;
    cursor: pointer;
  }

  .body :global(.mention.me) {
    color: var(--attention);
  }

  .edited {
    margin-left: 4px;
    font-size: 12px;
    color: var(--weak);
  }

  .edit .field {
    display: block;
    resize: none;
    line-height: 1.45;
  }

  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
    font-size: 13px;
  }

  .hint {
    color: var(--weak);
  }

  .problem {
    color: var(--error);
  }

  .btn.small {
    min-height: 22px;
    padding: 0 8px;
    font-size: 12px;
  }

  .btn.danger {
    background: #7a2a24;
    color: #ffd9d4;
  }

  .actions {
    position: absolute;
    top: 2px;
    right: var(--pad);
    display: none;
    gap: 4px;
    padding-left: 6px;
    background: var(--bg);
  }

  .msg:hover .actions,
  .msg:focus-within .actions {
    display: flex;
  }
</style>
