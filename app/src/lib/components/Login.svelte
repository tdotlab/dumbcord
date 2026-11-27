<script lang="ts">
  import { porch } from "../porch.svelte";
  import { isTauri, primeNotifications } from "../platform";
  import PorchLight from "./PorchLight.svelte";

  let mode = $state<"login" | "register">("login");
  let server = $state(porch.server);
  let username = $state("");
  let password = $state("");
  let invite = $state("");

  // A browser that loaded Porch from the server itself already knows where it is.
  const needsServer = isTauri;

  function submit(e: SubmitEvent) {
    e.preventDefault();
    primeNotifications();
    porch.signIn(mode, server, username, password, invite);
  }
</script>

<main class="night">
  <div class="scene">
    <div class="lamp">
      <PorchLight height={150} company={3} lightUp />
    </div>

    <form class="card" onsubmit={submit} aria-labelledby="porch-title">
      <h1 id="porch-title">Porch</h1>
      <p class="lede">
        {mode === "login" ? "Welcome back. Pull up a chair." : "Make yourself an account."}
      </p>

      {#if needsServer}
        <label>
          <span>Server</span>
          <input
            bind:value={server}
            placeholder="porch.example.com"
            autocomplete="url"
            autocapitalize="off"
            spellcheck="false"
            required
          />
        </label>
      {/if}

      <label>
        <span>Username</span>
        <input
          bind:value={username}
          autocomplete="username"
          autocapitalize="off"
          spellcheck="false"
          minlength="2"
          maxlength="24"
          required
        />
      </label>

      <label>
        <span>Password</span>
        <input
          type="password"
          bind:value={password}
          autocomplete={mode === "login" ? "current-password" : "new-password"}
          minlength={mode === "register" ? 8 : undefined}
          required
        />
      </label>

      {#if mode === "register"}
        <label>
          <span>Invite code</span>
          <input bind:value={invite} autocomplete="off" spellcheck="false" />
          <small>Whoever runs your Porch gives you this.</small>
        </label>
      {/if}

      {#if porch.authError}
        <p class="error" role="alert">{porch.authError}</p>
      {/if}

      <button class="primary" type="submit" disabled={porch.busy}>
        {#if porch.busy}
          {mode === "login" ? "Signing in" : "Creating account"}
        {:else}
          {mode === "login" ? "Sign in" : "Create account"}
        {/if}
      </button>

      <p class="switch">
        {#if mode === "login"}
          New here? <button type="button" onclick={() => (mode = "register")}>Create an account</button>
        {:else}
          Have an account? <button type="button" onclick={() => (mode = "login")}>Sign in</button>
        {/if}
      </p>
    </form>
  </div>
</main>

<style>
  .night {
    height: 100%;
    overflow-y: auto;
    display: grid;
    place-items: center;
    padding: 2rem 1.25rem;
    /* Dusk: the last of the light low on the horizon. */
    background:
      radial-gradient(120% 60% at 50% 115%, #3a3550 0%, transparent 60%),
      linear-gradient(180deg, #151a26 0%, var(--night) 55%, #262b40 100%);
  }

  .scene {
    position: relative;
    width: min(100%, 23rem);
  }

  /* Hung on the card's corner, in front of it, so its light falls on the form. */
  .lamp {
    position: absolute;
    z-index: 1;
    left: -1.6rem;
    top: -4.4rem;
    pointer-events: none;
  }

  .card {
    position: relative;
    display: grid;
    gap: 1rem;
    padding: 2rem 1.75rem 1.5rem;
    background: color-mix(in srgb, var(--boards) 88%, transparent);
    border: 1px solid var(--line);
    border-radius: 14px;
    /* Light from the lantern falling on the top-left of the card. */
    background-image: radial-gradient(60% 45% at 0% 0%, rgba(242, 196, 109, 0.1), transparent 70%);
    box-shadow:
      inset 0 1px 0 rgba(242, 196, 109, 0.1),
      0 30px 60px -20px rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(6px);
  }

  h1 {
    margin: 0 0 0 4.5rem;
    font-family: var(--serif);
    font-weight: 400;
    font-size: var(--t-display);
    line-height: 1;
    letter-spacing: -0.01em;
  }

  .lede {
    margin: -0.25rem 0 0.5rem 4.5rem;
    color: var(--muted);
  }

  label {
    display: grid;
    gap: 0.35rem;
  }

  label span {
    font-size: var(--t-small);
    font-weight: 600;
    color: var(--muted);
  }

  small {
    color: var(--faint);
    font-size: var(--t-small);
  }

  input {
    width: 100%;
    padding: 0.65rem 0.8rem;
    background: var(--sunk);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-s);
    outline: none;
    transition: border-color 120ms;
  }

  input:focus {
    border-color: var(--haint);
    box-shadow: 0 0 0 3px var(--haint-wash);
  }

  .error {
    margin: 0;
    padding: 0.6rem 0.8rem;
    border-radius: var(--radius-s);
    background: var(--error-wash);
    color: var(--error);
    font-size: var(--t-small);
  }

  .primary {
    margin-top: 0.25rem;
    padding: 0.7rem 1rem;
    border: none;
    border-radius: var(--radius-s);
    background: var(--haint);
    color: #14201d;
    font-weight: 700;
    transition: background 120ms;
  }

  .primary:hover:not(:disabled) {
    background: #b5e3d8;
  }

  .primary:disabled {
    opacity: 0.6;
    cursor: progress;
  }

  .switch {
    margin: 0;
    text-align: center;
    color: var(--muted);
    font-size: var(--t-small);
  }

  .switch button {
    padding: 0;
    border: none;
    background: none;
    color: var(--haint);
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  @media (max-width: 420px) {
    .lamp {
      left: -0.5rem;
    }
  }
</style>
