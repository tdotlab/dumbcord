<script lang="ts" module>
  // Muted dusk tones; a name always gets the same one.
  const tones = ["#7fa3c4", "#c49a86", "#93b88a", "#b58fb0", "#7fb7ad", "#b8ad7f", "#9a8fc0", "#c48f8f"];

  export function toneOf(name: string): string {
    let h = 0;
    for (const c of name) h = (h * 31 + c.charCodeAt(0)) >>> 0;
    return tones[h % tones.length]!;
  }
</script>

<script lang="ts">
  let {
    name,
    size = 36,
    lit = false,
  }: { name: string; size?: number; lit?: boolean } = $props();
</script>

<span
  class="avatar"
  class:lit
  style:--tone={toneOf(name)}
  style:width="{size}px"
  style:height="{size}px"
  style:font-size="{Math.round(size * 0.42)}px"
  aria-hidden="true"
>
  {name.slice(0, 1).toUpperCase()}
</span>

<style>
  .avatar {
    position: relative;
    display: inline-grid;
    place-items: center;
    flex: none;
    border-radius: 50%;
    background: color-mix(in srgb, var(--tone) 26%, var(--night));
    color: var(--tone);
    font-family: var(--serif);
    line-height: 1;
    user-select: none;
  }

  /* A small lit bulb at the shoulder: this person is on the porch. */
  .lit::after {
    content: "";
    position: absolute;
    right: -1px;
    bottom: -1px;
    width: 30%;
    height: 30%;
    min-width: 8px;
    min-height: 8px;
    border-radius: 50%;
    background: var(--porchlight);
    box-shadow:
      0 0 0 2px var(--boards),
      0 0 8px var(--porchlight-glow);
  }
</style>
