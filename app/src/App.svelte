<script lang="ts">
  import { porch } from "./lib/porch.svelte";
  import Chat from "./lib/components/Chat.svelte";
  import Login from "./lib/components/Login.svelte";
  import Members from "./lib/components/Members.svelte";
  import PorchLight from "./lib/components/PorchLight.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";

  type Drawer = "none" | "nav" | "members";

  let drawer = $state<Drawer>("none");
  let membersOpen = $state(localStorage.getItem("porch.members") !== "hidden");
  let narrow = $state(false);

  $effect(() => {
    const mq = window.matchMedia("(max-width: 760px)");
    const update = () => {
      narrow = mq.matches;
      drawer = "none";
    };
    update();
    mq.addEventListener("change", update);
    return () => mq.removeEventListener("change", update);
  });

  $effect(() => {
    const waiting = Object.values(porch.convs).reduce((n, c) => n + c.attention, 0);
    document.title = waiting ? `(${waiting}) Porch` : "Porch";
  });

  function togglePeople() {
    if (narrow) {
      drawer = drawer === "members" ? "none" : "members";
    } else {
      membersOpen = !membersOpen;
      localStorage.setItem("porch.members", membersOpen ? "shown" : "hidden");
    }
  }

  const close = () => (drawer = "none");
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && drawer !== "none" && close()} />

{#if porch.phase === "signed-out"}
  <Login />
{:else if porch.phase === "resuming"}
  <main class="splash" aria-busy="true">
    <PorchLight height={110} company={2} lightUp />
    <p>Porch</p>
  </main>
{:else}
  <div
    class="shell"
    class:members-open={membersOpen && !narrow}
    class:nav-drawer={drawer === "nav"}
    class:members-drawer={drawer === "members"}
  >
    <div class="nav-col">
      <Sidebar onnavigate={close} />
    </div>
    <main class="chat-col">
      <Chat
        onmenu={() => (drawer = drawer === "nav" ? "none" : "nav")}
        onpeople={togglePeople}
        peopleOpen={narrow ? drawer === "members" : membersOpen}
      />
    </main>
    <div class="members-col">
      <Members onnavigate={close} />
    </div>
    {#if drawer !== "none"}
      <button class="scrim" type="button" aria-label="Close" onclick={close}></button>
    {/if}
  </div>
{/if}

{#if porch.notice}
  <div class="toast" role="status">{porch.notice}</div>
{/if}

<style>
  .splash {
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 1.25rem;
    height: 100%;
  }

  .splash p {
    margin: 0;
    font-family: var(--serif);
    font-size: var(--t-title);
    color: var(--muted);
  }

  .shell {
    display: grid;
    grid-template-columns: var(--sidebar-w) minmax(0, 1fr) 0;
    height: 100%;
    overflow: hidden;
    position: relative;
  }

  .shell.members-open {
    grid-template-columns: var(--sidebar-w) minmax(0, 1fr) var(--members-w);
  }

  .nav-col,
  .members-col {
    min-width: 0;
    overflow: hidden;
  }

  .chat-col {
    min-width: 0;
    position: relative;
  }

  .scrim {
    display: none;
  }

  @media (max-width: 1000px) {
    .shell.members-open {
      grid-template-columns: 220px minmax(0, 1fr) 200px;
    }

    .shell {
      grid-template-columns: 220px minmax(0, 1fr) 0;
    }
  }

  @media (max-width: 760px) {
    .shell,
    .shell.members-open {
      grid-template-columns: minmax(0, 1fr);
    }

    .nav-col,
    .members-col {
      position: absolute;
      top: 0;
      bottom: 0;
      z-index: 10;
      width: min(84vw, 300px);
      transition: transform 220ms cubic-bezier(0.3, 0, 0.2, 1);
      box-shadow: 0 0 40px rgba(0, 0, 0, 0.45);
    }

    .nav-col {
      left: 0;
      transform: translateX(-105%);
    }

    .members-col {
      right: 0;
      transform: translateX(105%);
    }

    .nav-drawer .nav-col,
    .members-drawer .members-col {
      transform: none;
    }

    .scrim {
      display: block;
      position: absolute;
      inset: 0;
      z-index: 5;
      border: none;
      background: rgba(10, 13, 20, 0.55);
      cursor: default;
    }
  }

  .toast {
    position: fixed;
    left: 50%;
    bottom: 6rem;
    z-index: 20;
    max-width: min(90vw, 28rem);
    padding: 0.6rem 1rem;
    transform: translateX(-50%);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-m);
    background: var(--raise);
    box-shadow: 0 16px 32px -12px rgba(0, 0, 0, 0.6);
    font-size: var(--t-small);
    animation: rise 180ms ease-out;
  }

  @keyframes rise {
    from { opacity: 0; transform: translate(-50%, 6px); }
  }
</style>
