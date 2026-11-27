<script lang="ts" module>
  /** How brightly the porch is lit for this many other people. */
  export function glowOf(company: number): number {
    return company === 0 ? 0.22 : Math.min(0.5 + company * 0.12, 1);
  }
</script>

<script lang="ts">
  // A wall lantern. Its glow grows with the number of people on the porch.
  let {
    company = 0,
    height = 64,
    lightUp = false,
  }: {
    /** Other people online right now. */
    company?: number;
    height?: number;
    /** Play the lighting-up moment once on mount. */
    lightUp?: boolean;
  } = $props();

  const glow = $derived(glowOf(company));
  const uid = $props.id();
</script>

<svg
  class="lantern"
  class:light-up={lightUp}
  viewBox="0 0 64 96"
  height={height}
  width={(height * 64) / 96}
  style:--glow={glow}
  aria-hidden="true"
>
  <defs>
    <radialGradient id="halo-{uid}">
      <stop offset="0" stop-color="#f2c46d" stop-opacity="0.75" />
      <stop offset="0.35" stop-color="#f2c46d" stop-opacity="0.22" />
      <stop offset="1" stop-color="#f2c46d" stop-opacity="0" />
    </radialGradient>
    <linearGradient id="glass-{uid}" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#fbe3a8" />
      <stop offset="1" stop-color="#e9a654" />
    </linearGradient>
  </defs>

  <circle class="halo" cx="32" cy="50" r="46" fill="url(#halo-{uid})" />

  <!-- wall plate and arm -->
  <rect x="27" y="2" width="10" height="16" rx="2.5" fill="#4b5570" />
  <path d="M32 18v5" stroke="#4b5570" stroke-width="3" />
  <!-- roof -->
  <path d="M20 31h24l-5-8H25z" fill="#59647f" />
  <!-- glass -->
  <rect class="glass" x="21.5" y="31" width="21" height="33" fill="url(#glass-{uid})" />
  <!-- frame -->
  <path
    d="M21.5 31v33M42.5 31v33M32 31v33M21.5 47.5h21"
    stroke="#59647f"
    stroke-width="2"
  />
  <!-- base and finial -->
  <path d="M19 64h26l-4 6H23z" fill="#59647f" />
  <circle cx="32" cy="73.5" r="2.5" fill="#59647f" />
</svg>

<style>
  .lantern {
    display: block;
    overflow: visible;
  }

  .halo {
    opacity: var(--glow);
    transition: opacity 900ms ease;
  }

  .glass {
    opacity: calc(0.45 + var(--glow) * 0.55);
    transition: opacity 900ms ease;
  }

  /* The bulb catches, stutters once, then holds. */
  .light-up .halo {
    animation: halo-up 1400ms cubic-bezier(0.3, 0, 0.2, 1) both;
  }

  .light-up .glass {
    animation: glass-up 1400ms cubic-bezier(0.3, 0, 0.2, 1) both;
  }

  @keyframes halo-up {
    0% { opacity: 0; }
    18% { opacity: calc(var(--glow) * 0.8); }
    26% { opacity: 0.1; }
    100% { opacity: var(--glow); }
  }

  @keyframes glass-up {
    0% { opacity: 0.3; }
    18% { opacity: 0.9; }
    26% { opacity: 0.4; }
    100% { opacity: calc(0.45 + var(--glow) * 0.55); }
  }
</style>
