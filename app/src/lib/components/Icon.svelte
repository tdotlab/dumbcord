<script lang="ts" module>
  // 24px stroke icons, drawn for Porch.
  const paths = {
    hash: "M5 9h14M5 15h14M10 4 8 20M16 4l-2 16",
    at: "M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0Zm0 0v1.5a2.5 2.5 0 0 0 5 0V12a9 9 0 1 0-3.5 7.1",
    plus: "M12 5v14M5 12h14",
    send: "M5 12h13M12 5l7 7-7 7",
    people: "M9 11a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Zm-6 9c0-3.3 2.7-6 6-6s6 2.7 6 6M16 4.3a3.5 3.5 0 0 1 0 6.4M21 20c0-2.6-1.6-4.8-4-5.6",
    menu: "M4 7h16M4 12h16M4 17h16",
    close: "M6 6l12 12M18 6 6 18",
    edit: "M4 20h4L19 9a2.8 2.8 0 0 0-4-4L4 16v4Z",
    trash: "M5 7h14M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3",
    out: "M14 4h5v16h-5M10 8l-4 4 4 4M6 12h10",
    down: "M12 5v14M6 13l6 6 6-6",
    retry: "M4 12a8 8 0 1 0 2.4-5.7M4 4v4h4",
  } as const;
  export type IconName = keyof typeof paths;
</script>

<script lang="ts">
  let { name, size = 18 }: { name: IconName; size?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={paths[name]} />
</svg>
