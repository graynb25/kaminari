# Design spec

Status: active. This is the visual contract for Kaminari. If a screen does not match this file, the screen is wrong.

## Direction

Dark media first. Near black canvas, warm paper text, one volt indigo accent. Dark is home, light lands in P2. Posters feel rich, not flat. Glass only for overlays: palette, player bar, toasts, dialogs. Everything else stays solid so text stays readable.

Type: Sora or Space Grotesk for display and hero numbers, Inter for UI, Noto Sans JP for Japanese titles, JetBrains Mono for timecodes and diagnostics. Display never runs body copy. Body never runs all caps.

Desktop native. Frameless window, real title bar with drag, no browser chrome. Chrome icons are one inline SVG set (24px, 1.5 stroke, currentColor). No emoji in the chrome. Every async view ships four forms: loading, empty, error, offline. No blank panels. No layout shift on image load. Poster and backdrop slots reserve aspect ratio and paint blurhash first.

What we steal: Apple TV hero and focus glow, Plex continue watching rows, Mihon source cards and library filters, Stremio detail hero with one clear play action. What we skip: autoplay with sound, server setup wizards, tab metaphors.

## Tokens

Tokens live in `src/app.css`. Add a token before you add a one off value.

```css
:root {
  --void-950: #08080f; /* app bg */
  --void-900: #0e0e1a; /* panel */
  --void-800: #171726; /* raised */
  --void-700: #23233a; /* hover */
  --volt-300: #b7b5ff;
  --volt-400: #8b8aff; /* primary */
  --volt-500: #6c6bf5; /* primary hover */
  --paper-50: #f4f3ff;
  --paper-300: #b8b7cc;
  --paper-500: #7a7893;
  --line: rgb(255 255 255 / 0.08);
  --radius-sm: 12px;
  --radius-md: 16px;
  --radius-lg: 20px;
  --radius-xl: 24px;
}
```

Contrast: paper on void passes AAA. Volt on void is 4.6 to 1. Volt is for large type, bold labels, focus rings and fills paired with text, never for small body copy. Verified with axe and a token test in CI.

Radius steps are 12, 16, 20, 24. Small controls use 12. Cards use 16. Hero, player and dialogs use 20 or 24. Borders are 1px white at 8 percent on dark. When light lands in P2, borders flip to ink at 8 percent.

## Components

### PosterCard (2:3)

Fixed ratio, radius 16, blurhash behind the image. Title clamps to two lines, 13px semibold. Caption holds year and score. Progress bar is 3px volt when continuing. Badges are small and rare: NEW, DUB, AIRING.

Hover lifts 4px and scales to 1.03 over 160ms. Inner image scales to 1.06 over 320ms. Glow is a 1px volt ring plus a soft volt shadow. Focus visible uses 2px accent ring with 2px offset. Keyboard: Enter opens detail, P plays, L toggles library. Loading shows shimmer at the same ratio. Missing art shows a solid void tile with title initial, never a broken icon.

### HeroCarousel (400px)

Five trending titles from AniList, using only entries with a real `bannerImage` so every slide is a clear full-bleed image. Height clamps from 320 to 440 by viewport width. Title clamps to two lines and scales down on narrow windows, so copy never disappears. Dots plus arrow buttons. Keyboard left and right move. Backdrop has a left scrim from void at 85 percent to clear at 40 percent so text holds.

Slide order: eyebrow (`#1 TRENDING, FALL 2026`), 40px display title, meta row (score, year, episodes, format), two line synopsis, three buttons (Play, Add, Details). Text staggers 60ms through transitions, not animation fill, so a dropped frame cannot hide the copy. Every banner slide uses the slow zoom. Titles without a banner stay in the shelves. If fewer than three banner titles exist, the carousel falls back to the wider list and a clear poster card over a blurred cover. Reduced motion kills drift and autoplay, dots stay manual.

### Scores

AniList returns a score from 0 to 100. Show it on a 10 point scale with one decimal. Cards read `★ 8.7`, hero and detail read `8.7/10`. The card badge carries a tooltip naming the raw AniList value, for example `AniList score 87 out of 100`. Never put a star next to a 0 to 100 number. That reads like 87 stars.

### Shelf

Header row: H2, count, See all link, scroll buttons that appear on hover and stay on focus. Body scrolls horizontal with snap, 16px gaps. Scrollbars stay hidden, arrow buttons carry scroll, keyboard arrows work on the focused shelf. First run shows one callout, not stacked empty boxes. Library Preview renders only when the library has items. Backdrop variant is 16:9 at 320px wide with remaining time and progress. Poster variant is 2:3.

### SearchPalette (Ctrl K)

Centered modal, 640px wide, glass void at 90 percent plus 20px blur, radius 20. Input on top, grouped results below: Library, AniList, Actions, Sources. Debounce 200ms. Two chars to query. Abort the last request. Up and down move, Enter opens, Shift plus Enter plays, Esc closes. Empty state shows 8 recent searches stored locally. AniList slow state shows a banner: cached results plus seconds left.

### DetailPage

Banner 320px plus scrim. Poster 224px overlaps by 96px. Title block, meta, action row. Tabs: Episodes, Details, Related. Characters stay read only in v1. Play Next opens the first unwatched episode. Library button has a status menu: watching, planning, completed, paused, dropped. Sync chip shows AniList OAuth state. Episodes virtualize. Each row: 128 by 72 thumb, number, title, duration, watched check. Toggle is optimistic with rollback and a toast on DB error. AniList 429 shows stale data plus a countdown. 404 shows a removed panel.

Current build: Play opens a note until sources land. Episodes are a numbered grid, capped at 200. Each button toggles watched state, with Mark all and Clear beside the progress bar. Marking watched adds the series to the library if it is not there yet.

### Progress

Watched episodes live in SQLite. Continue Watching joins series in the library with status watching, ordered by the newest progress. It shows the last watched episode and watched count. Library cards show watched over total plus a thin volt bar. A series with zero watched episodes is not in Continue Watching.

Native source runs join the same shelf from their own table. One card per show, always the episode with the newest progress. The card shows a 16:9 cover, the extension name, position over duration and a volt progress strip. The shelf only shows the start callout when the library and both continue lists are empty. Clicking a source card resolves that episode and opens the player at the saved position. The source library Resume button does the same. History source rows only open the show, because a log row is not a resume action.

### Sources

The Sources page leads with a registry URL field. Loading the yuzono Aniyomi index returned 250 packages in a live check on 2026-11-08. Installed packages render first, then available packages with name, package id, version, language, source count, and NSFW or torrent badges. A filter narrows the list. Cards render 60 at a time with a Show more button, so 250 packages do not land in the DOM at once. Add stores metadata only. No APK downloads, no package code runs until the QuickJS host lands. The page says that in plain copy.

### VideoPlayer (libmpv surface)

Full window route `/watch/:id/:ep`. Black field. mpv canvas fills. Custom bar in glass, hides after 2.5 seconds idle, returns on move or focus. Time shows current and total in mono. Sliders scrub with 100ms linear response.

Keys: Space or K play and pause, J and L jump 10 seconds, arrows jump 10, Shift plus arrows jump 60, Up and Down volume, M mute, F fullscreen, T fill, V cycle subs, comma and period step speed, 0 resets speed, N next episode, D diagnostics, Esc back. Single click pauses, double click fullscreen, 300ms to tell them apart. Buffering shows spinner plus quality label, never silence. Error shows code plus Retry, Try next source, Copy diagnostics. Ended shows Up Next with a 10 second countdown you can cancel. No native video controls, ever. Subs render through libass, paper white at 22px with outline.

### Library

Status filters across the top: All, Watching, Planning, Completed, Paused, Dropped, each with a count. A title filter sits on the right. Cards show watched over total plus a thin volt bar. Each card carries a status select that writes straight to SQLite, so the filter counts move with it.

### History

Newest first, one row per series and episode. Cover, title, episode number, relative time, and a Watched label. Clicking a row opens the series. The empty state points back to Browse.

Native source progress merges into the same day groups, sorted by the time it was saved. Source rows show the show title, episode, extension name, and either In progress or Watched. Clicking one opens the source detail with that show selected. Events, titles and the latest timestamp in the header count both libraries.

### TitleBar, nav, SourceCard, toast, EmptyState, skeleton

TitleBar is 40px, void 900 at 80 percent plus 16px blur. Drag on empty space. Center holds the palette trigger that looks like an input. Right holds min, max, close with 46 by 40px targets. Close hovers red. Nav rail is 72px, icons plus labels, keys 1 to 5, active pill in volt.

SourceCard: 40px icon, name, version chip, lang, status dot, toggle, Configure and Update buttons, Log link. Error cards keep the last error and a Retry button. Disabled cards dim to 60 percent.

Nav keys are 1 to 5 for the five rail views (Browse, Library, History, Sources, Settings). The spec once said 1 to 7, written for future Downloads and Player views. Those views do not exist yet, so unbound digits stay silent instead of pretending.

Toast: bottom right, max 3, 4200ms, icon plus text plus one action (Undo, Retry). Never covers player controls, offsets when the player is open. Role status for info, alert for error.

EmptyState: centered icon at 48px, H3, one body line in paper 300, one primary action plus one link. Copy names the cause and the next step. Example: `No results for "frieren". Try romaji or check spelling.`

Skeleton: same shape as the content, shimmer 1200ms loop, static fill when reduced motion asks for stillness.

## Motion

Card hover: 160ms ease out, up 4px, scale 1.03. Button hover: 150ms, up 2px. Fade for menus and tips: 100ms. Modal and toast entry: 250ms, up 8px plus fade. Hero crossfade: 600ms. Scrub: 100ms linear. Toggle pop: 300ms spring, sparing use. Animate transform and opacity only. Never width, height, top, left. Short sentences in code comments explain any exception.

Reduced motion: media query plus a Svelte guard. All durations drop near zero. Shimmer, drift, and autoplay stop.

## Window and shortcuts

Min size 1024 by 640. Content max 1440px, gutters 24px (32px at md and up). Tray menu holds Show and Quit. Shortcuts dialog opens with `?` and lists every key.

Global when not typing: Ctrl K or slash opens the palette, 1 to 5 move between rail views, question mark opens help, Esc closes top layer. Focus traps in modal and player. Focus returns on close. Skip link jumps to content. Context menu on cards mirrors hover actions so touch and keyboard reach the same verbs.

## Performance

Cold start under 500ms in release on a mid range laptop, warm cache, internal storage. Measure with the scaffold flag and again in release. Debug numbers do not count.

Grid of 500 posters scrolls at 60fps. That means virtualization, 240px thumbs, lazy load with observer, overscan of 4 rows. Search palette answers under 50ms locally, under 600ms p95 against AniList with debounce and cache. Play to first frame under 1500ms p95 on a cached source. Installer under 15MB before libmpv resources. CI fails past that. Image disk LRU holds 512MB with 30 day TTL. Blurhash ships in the media row.

## Accessibility

Two pixel accent ring with offset on focus visible. Body contrast 7 to 1 or better, captions 4.5 to 1 or better. Landmarks for nav, main, dialog. Icon buttons carry labels. Toasts use live regions. Player time is polite on pause and seek, silent during play. Episodes read as options with selected state. Touch targets in player hit 44px. Strings go through i18n keys from day one even while only English ships.
