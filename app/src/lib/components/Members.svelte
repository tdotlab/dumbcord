<script lang="ts">
  import { porch } from "../porch.svelte";
  import Avatar from "./Avatar.svelte";

  let { onnavigate }: { onnavigate?: () => void } = $props();

  const here = $derived(porch.users.filter((u) => porch.online.has(u)));
  const away = $derived(porch.users.filter((u) => !porch.online.has(u)));

  function dm(user: string) {
    if (user === porch.me) return;
    porch.open(`@${user}`);
    onnavigate?.();
  }
</script>

<aside class="members" aria-label="People">
  {#snippet person(user: string, lit: boolean)}
    <li>
      <button
        class="row"
        class:me={user === porch.me}
        class:away={!lit}
        disabled={user === porch.me}
        title={user === porch.me ? undefined : `Message ${user}`}
        onclick={() => dm(user)}
      >
        <Avatar name={user} size={30} {lit} />
        <span class="name">{user}</span>
        {#if user === porch.me}<span class="you">you</span>{/if}
      </button>
    </li>
  {/snippet}

  <section>
    <h2>On the porch <span class="count">{here.length}</span></h2>
    <ul>
      {#each here as user (user)}
        {@render person(user, true)}
      {/each}
    </ul>
  </section>

  {#if away.length}
    <section>
      <h2>Inside <span class="count">{away.length}</span></h2>
      <ul>
        {#each away as user (user)}
          {@render person(user, false)}
        {/each}
      </ul>
    </section>
  {/if}
</aside>

<style>
  .members {
    height: 100%;
    overflow-y: auto;
    padding: 1rem 0.5rem;
    background: var(--boards);
    border-left: 1px solid var(--line);
  }

  section + section {
    margin-top: 1.5rem;
  }

  h2 {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
    margin: 0 0 0.4rem;
    padding: 0 0.6rem;
    font-size: var(--t-small);
    font-weight: 600;
    color: var(--muted);
  }

  .count {
    color: var(--faint);
    font-weight: 400;
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    width: 100%;
    padding: 0.35rem 0.6rem;
    border: none;
    border-radius: var(--radius-s);
    background: none;
    color: var(--text);
    text-align: left;
  }

  .row:not(:disabled):hover {
    background: var(--raise);
  }

  .row:disabled {
    cursor: default;
  }

  .row.away {
    color: var(--faint);
  }

  .row.away :global(.avatar) {
    opacity: 0.55;
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .you {
    font-size: 0.7rem;
    color: var(--faint);
  }
</style>
