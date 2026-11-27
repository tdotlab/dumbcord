<script>
  import { onMount } from "svelte";
  import { porch, label } from "../porch.svelte.js";

  let { key } = $props();

  let field = $state();
  const touch = window.matchMedia("(pointer: coarse)").matches;

  const text = $derived(porch.drafts[key] ?? "");
  const placeholder = $derived(key.startsWith("#") ? `Message #${label(key)}` : `Message ${label(key)}`);
  const typers = $derived(porch.typingIn(key));
  const typingLine = $derived(
    typers.length === 0
      ? ""
      : typers.length === 1
        ? `${typers[0]} is typing...`
        : typers.length === 2
          ? `${typers[0]} and ${typers[1]} are typing...`
          : "Several people are typing...",
  );

  function fit() {
    if (!field) return;
    field.style.height = "auto";
    field.style.height = `${Math.min(field.scrollHeight + 2, 240)}px`;
  }

  function onInput(e) {
    porch.drafts[key] = e.target.value;
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

  function onKey(e) {
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
  function onWindowKey(e) {
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
  <div class="typing" aria-live="polite">{typingLine}</div>
  <form
    class="box"
    onsubmit={(e) => {
      e.preventDefault();
      send();
    }}
  >
    <textarea
      class="field"
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
    <button type="submit" class="btn" disabled={!text.trim()}>Send</button>
  </form>
</div>

<style>
  .composer {
    flex: none;
    padding: 0 var(--pad) var(--pad) 16px;
    padding-bottom: max(var(--pad), env(safe-area-inset-bottom));
  }

  .typing {
    height: 20px;
    padding-left: 2px;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 12px;
    color: var(--weak);
  }

  .box {
    display: flex;
    align-items: flex-end;
    gap: 6px;
  }

  textarea {
    flex: 1;
    min-height: 34px;
    padding: 6px 10px;
    resize: none;
    line-height: 1.45;
  }

  .btn {
    min-height: 34px;
    padding: 0 14px;
  }
</style>
