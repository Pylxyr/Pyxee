<script lang="ts">
  const paths: Record<string, string> = {
    home: 'M3 11l9-8 9 8v10h-6v-6H9v6H3z',
    explore: 'M12 3a9 9 0 100 18 9 9 0 000-18zm3.5 5.5l-2 5-5 2 2-5z',
    settings: 'M18.7 10.0 L21.7 10.3 L21.7 13.7 L18.7 14.0 L18.2 15.3 L20.0 17.6 L17.6 20.0 L15.3 18.2 L14.0 18.7 L13.7 21.7 L10.3 21.7 L10.0 18.7 L8.7 18.2 L6.4 20.0 L4.0 17.6 L5.8 15.3 L5.3 14.0 L2.3 13.7 L2.3 10.3 L5.3 10.0 L5.8 8.7 L4.0 6.4 L6.4 4.0 L8.7 5.8 L10.0 5.3 L10.3 2.3 L13.7 2.3 L14.0 5.3 L15.3 5.8 L17.6 4.0 L20.0 6.4 L18.2 8.7z M15 12a3 3 0 11-6 0 3 3 0 016 0z',
    library: 'M4 4v16M9 4v16M14 6l6 14',
    search: 'M11 4a7 7 0 100 14 7 7 0 000-14zm9 16l-4-4',
    play: 'M7 4l13 8-13 8z',
    pause: 'M8 5v14M16 5v14',
    prev: 'M6 5v14M19 5L9 12l10 7z',
    next: 'M18 5v14M5 5l10 7-10 7z',
    shuffle: 'M3 7h4l10 10h4M3 17h4l3-3M14 10l3-3h4M18 4l3 3-3 3M18 14l3 3-3 3',
    repeat: 'M17 2l3 3-3 3M4 11V9a4 4 0 014-4h12M7 22l-3-3 3-3M20 13v2a4 4 0 01-4 4H4',
    heart: 'M12 20s-8-5-8-11a4.5 4.5 0 018-2.8A4.5 4.5 0 0120 9c0 6-8 11-8 11z',
    volume: 'M4 9v6h4l5 4V5L8 9zM17 9a4 4 0 010 6',
    up: 'M6 15l6-6 6 6',
    down: 'M6 9l6 6 6-6',
    more: 'M12 5h.01M12 12h.01M12 19h.01',
    drag: 'M9 6h.01M9 12h.01M9 18h.01M15 6h.01M15 12h.01M15 18h.01',
    close: 'M6 6l12 12M18 6L6 18',
    top: 'M5 4h14M12 20V9M6 14l6-6 6 6',
    plus: 'M12 5v14M5 12h14',
    queue: 'M4 6h12M4 12h12M4 18h7M17 14v6l5-3z',
    music: 'M9 18V5l11-2v13M9 18a3 3 0 11-6 0 3 3 0 016 0zm11-2a3 3 0 11-6 0 3 3 0 016 0z',
  };
  let { name, size = 22, fill = false }: { name: keyof typeof paths | string; size?: number; fill?: boolean } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={fill ? 'currentColor' : 'none'}
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={paths[name] ?? ''} />
</svg>
