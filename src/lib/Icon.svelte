<script lang="ts">
  // One coherent chrome set: 24px viewBox, 1.5 stroke, currentColor.
  let {
    name,
    size = 20,
    strokeWidth = 1.5,
    class: className = "",
  }: {
    name: string;
    size?: number;
    strokeWidth?: number;
    class?: string;
  } = $props();

  const icons: Record<string, string> = {
    search: '<circle cx="11" cy="11" r="6.5" /><path d="m16 16 4.5 4.5" />',
    play: '<path d="M8 5.5v13l11-6.5z" fill="currentColor" stroke="none" />',
    pause:
      '<rect x="7" y="5" width="3.6" height="14" rx="1" fill="currentColor" stroke="none" /><rect x="13.4" y="5" width="3.6" height="14" rx="1" fill="currentColor" stroke="none" />',
    plus: '<path d="M12 5v14M5 12h14" />',
    check: '<path d="m5 12.5 4.5 4.5L19 7" />',
    library:
      '<path d="M6.5 3.5h11A1.5 1.5 0 0 1 19 5v16l-7-4.5L5 21V5a1.5 1.5 0 0 1 1.5-1.5z" />',
    clock:
      '<circle cx="12" cy="12" r="8.5" /><path d="M12 7.5V12l3 2" />',
    grid:
      '<rect x="3.5" y="3.5" width="7" height="7" rx="1.5" /><rect x="13.5" y="3.5" width="7" height="7" rx="1.5" /><rect x="3.5" y="13.5" width="7" height="7" rx="1.5" /><rect x="13.5" y="13.5" width="7" height="7" rx="1.5" />',
    sliders:
      '<path d="M4 7h9M17 7h3M4 17h3M11 17h9" /><circle cx="15" cy="7" r="2.2" /><circle cx="9" cy="17" r="2.2" />',
    bolt: '<path d="M13 2.5 5 13.5h5.5L10 21.5l8-11h-5.5z" />',
    chevronLeft: '<path d="m14.5 6-6 6 6 6" />',
    chevronRight: '<path d="m9.5 6 6 6-6 6" />',
    arrowLeft: '<path d="M19 12H5m6-6-6 6 6 6" />',
    arrowRight: '<path d="M5 12h14m-6-6 6 6-6 6" />',
    close: '<path d="m6 6 12 12M18 6 6 18" />',
    external:
      '<path d="M14 4h6v6M20 4l-9 9" /><path d="M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5" />',
    volume:
      '<path d="M4 9.5v5h3.5l5 4v-13l-5 4H4z" /><path d="M16 8.5a5 5 0 0 1 0 7" />',
    mute:
      '<path d="M4 9.5v5h3.5l5 4v-13l-5 4H4z" /><path d="m16 9 5 6m0-6-5 6" />',
    fullscreen:
      '<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />',
    minimize:
      '<path d="M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5" />',
    info:
      '<circle cx="12" cy="12" r="8.5" /><path d="M12 11v5M12 8h.01" />',
    alert:
      '<path d="M12 3.5 21 20H3z" /><path d="M12 9v5M12 17h.01" />',
    checkCircle:
      '<circle cx="12" cy="12" r="8.5" /><path d="m8.5 12 2.5 2.5 4.5-5" />',
    xCircle:
      '<circle cx="12" cy="12" r="8.5" /><path d="m9 9 6 6m0-6-6 6" />',
    refresh:
      '<path d="M20 12a8 8 0 1 1-2.4-5.7" /><path d="M20 3v5h-5" />',
    filter: '<path d="M4 6h16M7 12h10M10 18h4" />',
    monitor:
      '<rect x="3" y="4" width="18" height="12" rx="2" /><path d="M8 20h8M12 16v4" />',
    code: '<path d="m8 8-4 4 4 4M16 8l4 4-4 4" />',
    package:
      '<path d="M12 3 20 7.5v9L12 21l-8-4.5v-9z" /><path d="m4 7.5 8 4.5 8-4.5M12 12v9" />',
    shield:
      '<path d="M12 3 19 6v5c0 4.5-3 7.5-7 10-4-2.5-7-5.5-7-10V6z" /><path d="m9 12 2 2 4-4" />',
    star:
      '<path d="m12 3.8 2.5 5.1 5.6.8-4 3.9.9 5.5-5-2.6-5 2.6.9-5.5-4-3.9 5.6-.8z" fill="currentColor" stroke="none" />',
    dots:
      '<circle cx="5" cy="12" r="1.3" fill="currentColor" stroke="none" /><circle cx="12" cy="12" r="1.3" fill="currentColor" stroke="none" /><circle cx="19" cy="12" r="1.3" fill="currentColor" stroke="none" />',
    sparkles:
      '<path d="m12 3 1.6 4.4L18 9l-4.4 1.6L12 15l-1.6-4.4L6 9l4.4-1.6z" /><path d="m19 14 .8 2.2L22 17l-2.2.8L19 20l-.8-2.2L16 17l2.2-.8z" />',
    eye:
      '<path d="M2.5 12S6 6.5 12 6.5 21.5 12 21.5 12 18 17.5 12 17.5 2.5 12 2.5 12z" /><circle cx="12" cy="12" r="2.5" />',
    eyeOff:
      '<path d="M3 3l18 18" /><path d="M10.6 6.7A9.8 9.8 0 0 1 12 6.5c6 0 9.5 5.5 9.5 5.5a16 16 0 0 1-2.6 3.1M6.3 6.9A16 16 0 0 0 2.5 12S6 17.5 12 17.5c1.5 0 2.8-.3 4-.8" /><path d="M9.9 9.9a3 3 0 0 0 4.2 4.2" />',
    download: '<path d="M12 3v12m-5-5 5 5 5-5" /><path d="M4 19h16" />',
    trash:
      '<path d="M4 7h16M9 7V4h6v3M6.5 7l.8 12.5h9.4L17.5 7" /><path d="M10 10.5v6M14 10.5v6" />',
    folder:
      '<path d="M3 6.5h6l2 2h10v10a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5z" />',
    link:
      '<path d="M10 13a4 4 0 0 0 5.7 0l2.3-2.3a4 4 0 0 0-5.7-5.7L11 6.3" /><path d="M14 11a4 4 0 0 0-5.7 0L6 13.3a4 4 0 0 0 5.7 5.7l1.3-1.3" />',
    database:
      '<ellipse cx="12" cy="5.5" rx="7.5" ry="2.8" /><path d="M4.5 5.5v13c0 1.6 3.4 2.8 7.5 2.8s7.5-1.2 7.5-2.8v-13" /><path d="M4.5 12c0 1.6 3.4 2.8 7.5 2.8s7.5-1.2 7.5-2.8" />',
    film:
      '<rect x="3" y="4" width="18" height="16" rx="2" /><path d="M7 4v16M17 4v16M3 9h4M3 15h4M17 9h4M17 15h4" />',
    tv:
      '<rect x="3" y="6" width="18" height="12" rx="2" /><path d="M8 21h8M12 18v3" />',
    layers:
      '<path d="m12 3 9 5-9 5-9-5z" /><path d="m3 13 9 5 9-5M3 17l9 5 9-5" />',
    help:
      '<circle cx="12" cy="12" r="8.5" /><path d="M9.5 9.2a2.6 2.6 0 0 1 5 .9c0 1.8-2.5 2.1-2.5 4M12 17h.01" />',
    keyboard:
      '<rect x="2.5" y="6" width="19" height="12" rx="2" /><path d="M6 10h.01M9.5 10h.01M13 10h.01M16.5 10h.01M6 14h.01M9.5 14h.01M13 14h.01M16.5 14h.01M8 17h8" />',
    settings:
      '<path d="M4 7h10M18 7h2M4 17h4M12 17h8" /><circle cx="16" cy="7" r="2.2" /><circle cx="10" cy="17" r="2.2" />',
    globe:
      '<circle cx="12" cy="12" r="8.5" /><path d="M3.5 12h17M12 3.5c2.5 2.4 3.8 5.3 3.8 8.5S14.5 18.1 12 20.5c-2.5-2.4-3.8-5.3-3.8-8.5S9.5 5.9 12 3.5z" />',
    server:
      '<rect x="3" y="4" width="18" height="6" rx="1.5" /><rect x="3" y="14" width="18" height="6" rx="1.5" /><path d="M7 7h.01M7 17h.01" />',
    cpu:
      '<rect x="6" y="6" width="12" height="12" rx="2" /><path d="M9 2.5V6M15 2.5V6M9 18v3.5M15 18v3.5M2.5 9H6M2.5 15H6M18 9h3.5M18 15h3.5" />',
    hardDrive:
      '<rect x="3" y="5" width="18" height="14" rx="2" /><path d="M7 15h.01M11 15h6" />',
    wifiOff:
      '<path d="M3 3l18 18" /><path d="M8.5 8.6A11 11 0 0 1 21 9.5M5 12.5a11 11 0 0 1 2-1.3M8.5 16a5 5 0 0 1 7 0M12 19h.01" />',
    calendar:
      '<rect x="3.5" y="5" width="17" height="15.5" rx="2" /><path d="M3.5 9.5h17M8 3v4M16 3v4" />',
    arrowUpRight: '<path d="M7 17 17 7M8 7h9v9" />',
    list: '<path d="M9 6h11M9 12h11M9 18h11" /><path d="M4 6h.01M4 12h.01M4 18h.01" />',
    playCircle:
      '<circle cx="12" cy="12" r="8.5" /><path d="m10 8.5 6 3.5-6 3.5z" fill="currentColor" stroke="none" />',
    skipForward:
      '<path d="M5 6.5 14 12l-9 5.5z" fill="currentColor" stroke="none" /><path d="M17 6v12" />',
    skipBack:
      '<path d="M19 6.5 10 12l9 5.5z" fill="currentColor" stroke="none" /><path d="M7 6v12" />',
    speed:
      '<path d="M12 20a8 8 0 1 0-8-8" /><path d="M12 12l4-4" /><circle cx="12" cy="12" r="1.5" fill="currentColor" stroke="none" />',
    captions:
      '<rect x="3" y="5" width="18" height="14" rx="2" /><path d="M10 10.5a2 2 0 1 0 0 3M17 10.5a2 2 0 1 0 0 3" />',
    bug:
      '<path d="M8 8a4 4 0 0 1 8 0" /><rect x="7" y="8" width="10" height="10" rx="5" /><path d="M12 8v10M7 12H4M20 12h-3M8 17l-2 2M16 17l2 2M8 8 6 6M16 8l2-2" />',
    copy:
      '<rect x="8" y="8" width="12" height="12" rx="2" /><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" />',
    sun:
      '<circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />',
    chevronDown: '<path d="m6 9.5 6 6 6-6" />',
    moon:
      '<path d="M20 13.5A8 8 0 1 1 10.5 4 6.5 6.5 0 0 0 20 13.5z" />',
  };
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
  class={className}
  aria-hidden="true"
  focusable="false"
>
  {@html icons[name] ?? icons.sparkles}
</svg>
