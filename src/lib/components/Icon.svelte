<script lang="ts">
  // Small inline stroke icon set (no external icon font).
  const paths: Record<string, string> = {
    grid: "M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z",
    list: "M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01",
    clock: "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 7v5l3 2",
    star: "M12 3l2.8 5.7 6.2.9-4.5 4.4 1 6.2L12 17.3 6.5 20.2l1-6.2L3 9.6l6.2-.9z",
    filter: "M3 5h18l-7 8v6l-4 2v-8z",
    folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
    download: "M12 4v11M7 10l5 5 5-5M4 20h16",
    settings:
      "M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z",
    search: "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM20 20l-4-4",
    chevronDown: "M6 9l6 6 6-6",
    chevronLeft: "M15 6l-6 6 6 6",
    arrowRight: "M5 12h14M13 6l6 6-6 6",
    arrowUp: "M12 19V5M6 11l6-6 6 6",
    arrowDown: "M12 5v14M6 13l6 6 6-6",
    close: "M6 6l12 12M18 6L6 18",
    check: "M5 12l5 5 9-10",
    plus: "M12 5v14M5 12h14",
    more: "M5 12h.01M12 12h.01M19 12h.01",
    undo: "M9 14L4 9l5-5M4 9h10a6 6 0 0 1 0 12h-3",
    image: "M4 5h16v14H4zM4 15l4-4 4 4 3-3 5 5M15 9h.01",
    refresh: "M20 11a8 8 0 0 0-14.9-3M4 4v4h4M4 13a8 8 0 0 0 14.9 3M20 20v-4h-4",
    trash: "M4 7h16M10 11v6M14 11v6M5 7l1 13h12l1-13M9 7V4h6v3",
    edit: "M4 20h4L19 9l-4-4L4 16zM14 6l4 4",
    gamepad: "M6 12h4M8 10v4M15.5 11h.01M17.5 13h.01M7 6h10a5 5 0 0 1 4.9 6l-.8 4a3 3 0 0 1-5.2 1.4L14 15h-4l-1.9 2.4A3 3 0 0 1 2.9 16l-.8-4A5 5 0 0 1 7 6z",
    eye: "M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12zM12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z",
    eyeOff: "M3 3l18 18M10.6 5.1A10 10 0 0 1 12 5c6 0 10 7 10 7a17 17 0 0 1-3.3 4M6.6 6.6A17 17 0 0 0 2 12s4 7 10 7a10 10 0 0 0 4.4-1M9.9 9.9a3 3 0 0 0 4.2 4.2",
    tag: "M3 12V4h8l10 10-8 8zM7.5 7.5h.01",
    grip: "M9 6h.01M15 6h.01M9 12h.01M15 12h.01M9 18h.01M15 18h.01",
    board: "M3 4h11v7H3zM16 4h5v16h-5zM3 13h11v7H3z",
    home: "M3 11l9-7 9 7M5 9.5V20h5v-6h4v6h5V9.5",
    layers: "M12 3l9 5-9 5-9-5zM3 13l9 5 9-5",
  };

  let { name, size = 18, fill = false, stroke = 2 }: { name: string; size?: number; fill?: boolean; stroke?: number } =
    $props();
</script>

<svg
  aria-hidden="true"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={fill ? "currentColor" : "none"}
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"><path d={paths[name] ?? ""} /></svg>
