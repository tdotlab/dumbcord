<script lang="ts">
  import { onMount } from "svelte";
  import { porch, label } from "../porch.svelte";
  import Icon from "./Icon.svelte";

  let { key }: { key: string } = $props();

  let field: HTMLTextAreaElement | undefined = $state();
  const touch = window.matchMedia("(pointer: coarse)").matches;

  const text = $derived(porch.drafts[key] ?? "");
  const placeholder = $derived(key.startsWith("#") ? `Message #${label(key)}` : `Message ${label(key)}`);
  const typers = $derived(porch.typingIn(key));
  const typingLine = $derived(
    typers.length === 0
      ? ""
      : typers.length === 1
        ? `${typers[0]} is typing`
        : typers.length === 2
          ? `${typers[0]} and ${typers[1]} are typing`
          : "Several people are typing",
  );

  function fit() {
    if (!field) return;
    field.style.height = "auto";
    field.style.height = `${Math.min(field.scrollHeight, 240)}px`;
  }

  function onInput(e: Event) {
    porch.drafts[key] = (e.target as HTMLTextAreaElement).value;
    fit();
    if (porch.drafts[key]) porch.nudgeTyping(key);
  }

  function send() {
    if (!text.trim()) return;
    porch.say(key, text);
    porch.drafts[key] = "";
    // `value` is one-way: if the draft goes "" -> text -> "" inside one flush,
    // Svelte sees no change and would leave the sent text in the box.
    if (field) field.value = "";
    queueMicrotask(fit);
    field?.focus();
  }

  function onKey(e: KeyboardEvent) {
    if (e.isComposing) return;
    if (e.key === "Enter" && !e.shiftKey && !touch) {
      e.preventDefault();
      send();
    } else if (e.key === "ArrowUp" && !text) {
      const last = porch.lastOwn(key);
      if (last) {
        e.preventDefault();
        porch.editing = last.id;
      }
    }
  }

  // Start typing anywhere and it lands here.
  function onWindowKey(e: KeyboardEvent) {
    if (!field || e.ctrlKey || e.metaKey || e.altKey || e.key.length !== 1) return;
    const t = document.activeElement;
    if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || porch.editing !== null) return;
    field.focus();
  }

  onMount(() => {
    fit();
    if (!touch) field?.focus();
  });
</script>

<svelte:window onkeydown={onWindowKey} />

<div class="composer">
  <div class="typing" aria-live="polite">
    {#if typingLine}
      <span class="dots" aria-hidden="true"><i></i><i></i><i></i></span>
      {typingLine}
    {/if}
  </div>
  <form
    class="box"
    onsubmit={(e) => {
      e.preventDefault();
      send();
    }}
  >
    <textarea
      bind:this={field}
      value={text}
      oninput={onInput}
      onkeydown={onKey}
      {placeholder}
      aria-label={placeholder}
      rows="1"
      maxlength="2000"
      enterkeyhint={touch ? "enter" : "send"}
    ></textarea>
    <button type="submit" class="send" disabled={!text.trim()} title="Send" aria-label="Send">
      <Icon name="send" size={18} />
    </button>
  </form>
</div>

<style>
  .composer {
    padding: 0 1rem 1rem;
  }

  .typing {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    height: 1.4rem;
    padding-left: 0.4rem;
    font-size: 0.75rem;
    color: var(--muted);
  }

  .dots {
    display: inline-flex;
    gap: 3px;
  }

  .dots i {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--muted);
    animation: bob 1.2s infinite ease-in-out;
  }

  .dots i:nth-child(2) {
    animation-delay: 0.15s;
  }

  .dots i:nth-child(3) {
    animation-delay: 0.3s;
  }

  @keyframes bob {
    0%, 60%, 100% { transform: translateY(0); opacity: 0.5; }
    30% { transform: translateY(-3px); opacity: 1; }
  }

  .box {
    display: flex;
    align-items: flex-end;
    gap: 0.4rem;
    padding: 0.35rem 0.35rem 0.35rem 0.9rem;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-m);
    background: var(--raise);
    transition: border-color 120ms;
  }

  .box:focus-within {
    border-color: rgba(159, 216, 203, 0.45);
  }

  textarea {
    flex: 1;
    min-height: 2.2rem;
    padding: 0.45rem 0;
    resize: none;
    border: none;
    background: none;
    outline: none;
    line-height: 1.45;
  }

  textarea::placeholder {
    color: var(--faint);
  }

  .send {
    display: grid;
    place-items: center;
    flex: none;
    width: 2.2rem;
    height: 2.2rem;
    border: none;
    border-radius: var(--radius-s);
    background: var(--haint);
    color: #14201d;
    transition: opacity 120ms, background 120ms;
  }

  .send:disabled {
    background: transparent;
    color: var(--faint);
    cursor: default;
  }
</style>
