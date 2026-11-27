<script>
  import { porch } from "../porch.svelte.js";
  import { isTauri, primeNotifications } from "../platform.js";

  let register = $state(false);
  let server = $state(porch.server);
  let username = $state("");
  let password = $state("");
  let invite = $state("");

  // A browser that loaded Porch from the server itself already knows where it is.
  const needsServer = isTauri;

  function submit(e) {
    e.preventDefault();
    primeNotifications();
    porch.signIn(register ? "register" : "login", server, username, password, invite);
  }
</script>

<main class="login">
  <form onsubmit={submit} aria-labelledby="login-title">
    <h1 id="login-title">Your friends. Your server.</h1>

    {#if needsServer}
      <label>
        <span>Server</span>
        <input
          class="field"
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
        class="field"
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
        class="field"
        type="password"
        bind:value={password}
        autocomplete={register ? "new-password" : "current-password"}
        minlength={register ? 8 : undefined}
        required
      />
    </label>

    <label class="check">
      <input type="checkbox" bind:checked={register} />
      <span>Create account</span>
    </label>

    {#if register}
      <label>
        <span>Invite code</span>
        <input class="field" bind:value={invite} autocomplete="off" spellcheck="false" />
        <small>Whoever runs your Porch gives you this.</small>
      </label>
    {/if}

    {#if porch.authError}
      <p class="error" role="alert">{porch.authError}</p>
    {/if}

    <button class="btn go" type="submit" disabled={porch.busy}>
      {#if porch.busy}
        {register ? "Creating account..." : "Signing in..."}
      {:else}
        {register ? "Create account and sign in" : "Sign in"}
      {/if}
    </button>
  </form>
</main>

<style>
  .login {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 48px 20px;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 14px;
    width: min(100%, 320px);
    margin: 0 auto;
  }

  h1 {
    margin: 0 0 6px;
    font-size: 24px;
    font-weight: 300;
    color: var(--strong);
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  label > span {
    font-size: 13px;
    color: var(--text);
  }

  small {
    font-size: 12px;
    color: var(--weak);
  }

  .check {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    align-self: flex-start;
    cursor: pointer;
  }

  .check > span {
    font-size: 14px;
  }

  /* egui's checkbox: a grey square with a light tick. */
  .check input {
    appearance: none;
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    margin: 0;
    border: 1px solid transparent;
    border-radius: var(--radius);
    background: var(--button);
    cursor: pointer;
  }

  .check input:hover {
    border-color: #969696;
  }

  .check input:checked::after {
    content: "";
    width: 8px;
    height: 4px;
    margin-top: -2px;
    border: solid var(--strong);
    border-width: 0 0 2px 2px;
    transform: rotate(-45deg);
  }

  .error {
    margin: 0;
    font-size: 13px;
    color: var(--error);
  }

  .go {
    align-self: flex-start;
    min-height: 32px;
    padding: 0 16px;
  }
</style>
