# Design spec

Status: active. This is the visual contract for Kaminari. If a screen does not match this file, the screen is wrong.

## Direction

Dark media first. Near black canvas, warm paper text, one volt indigo accent. Dark stays the signature, Light is a first class theme, and System follows Windows. Posters feel rich, not flat. Glass only for overlays: palette, player bar, toasts, dialogs. Everything else stays solid so text stays readable.

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

Radius steps are 12, 16, 20, 24. Small controls use 12. Cards use 16. Hero, player and dialogs use 20 or 24. Borders are 1px white at 8 percent on dark. In light, borders are ink at 10 percent.

### Themes

Three modes: System, Dark, Light. The choice lives under `kaminari.theme` in localStorage, and `public/theme-init.js` applies it before the module loads so the first frame is already correct. System listens to `prefers-color-scheme` and repaints live.

Light is named Daylight. Page `#f6f7fc`, cards `#ffffff`, ink `#191826`, muted `#6a6880`, borders ink at 10 percent, shadows tinted indigo instead of black. The primary gradient runs `#605ef0` to `#4d4ad8`, which keeps white button text above 4.5 to 1. Success uses `#0a7c56`, warning `#b45309`, danger `#e11d48`.

Media surfaces keep the dark token set in both themes: poster art, source art and the player. That keeps badge text, progress strips and player chrome legible over unknown artwork. The home hero, detail banner and source hero flip to an airy wash with ink copy in light, so the page never looks like a dark app with a white sheet pasted on.

The light theme is not an inversion. The atmosphere keeps the violet current, the grain stays, and the glass overlays turn to frosted porcelain. No pure black, no pure white text on the page.

## Components

### PosterCard (2:3)

Fixed ratio, radius 16, blurhash behind the image. Title clamps to two lines, 13px semibold. Caption holds year and score. Progress bar is 3px volt when continuing. Badges are small and rare: AIRING is green, COMPLETED is amber with a check, custom badges such as NEW or DUB use the neutral badge.

Hover lifts 4px and scales to 1.03 over 160ms. Inner image scales to 1.06 over 320ms. Glow is a 1px volt ring plus a soft volt shadow. Focus visible uses 2px accent ring with 2px offset. Keyboard: Enter opens detail, P plays, L toggles library. Loading shows shimmer at the same ratio. Missing art shows a solid void tile with title initial, never a broken icon.

### HeroCarousel (400px)

Five trending titles from AniList, using only entries with a real `bannerImage` so every slide is a clear full-bleed image. Height clamps from 320 to 440 by viewport width. Title clamps to two lines and scales down on narrow windows, so copy never disappears. Dots plus arrow buttons. Keyboard left and right move. Backdrop has a left scrim from void at 85 percent to clear at 40 percent so text holds.

Slide order: eyebrow (`#1 TRENDING, FALL 2026`), 40px display title, meta row (score, year, episodes, format), two line synopsis, three buttons (Play, Add, Details). Text staggers 60ms through transitions, not animation fill, so a dropped frame cannot hide the copy. Every banner slide uses the slow zoom. Titles without a banner stay in the shelves. If fewer than three banner titles exist, the carousel falls back to the wider list and a clear poster card over a blurred cover. Reduced motion kills drift and autoplay, dots stay manual.

### Scores

AniList returns a score from 0 to 100. Show it on a 10 point scale with one decimal. Cards read `★ 8.7`, hero and detail read `8.7/10`. The card badge carries a tooltip naming the raw AniList value, for example `AniList score 87 out of 100`. Never put a star next to a 0 to 100 number. That reads like 87 stars. When AniList has no score yet, the card shows `NR` in the same top-right slot with the tooltip `AniList has no score yet`; the detail card shows `n/a`. Never leave an empty badge or hide the slot.

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

The Sources page shows native extensions and Mangayomi sources. It has no Aniyomi registry browser: those packages are Android APKs and cannot run in Kaminari, so the page hides the registry until an APK runtime exists. Four stat cards show native extensions, Mangayomi sources, allowed hosts and runtime memory. The page states that Kaminari ships no source list and that you add a source you trust.

Source detail: cover, banner, synopsis and episode rows. Each row has a 96 by 56 thumbnail, using the episode image when the source provides one and the show banner or cover as a muted fallback, with an `EP n` badge in the bottom left. The episode action is Play, Resume or Rewatch. Play resolves the episode and starts the first stream, one click. Streams opens the quality list with Copy and Open in mpv. Resolve is implementation language, never a button label.

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

### App icon

1024 square, 928 squircle at radius 232 with transparent corners. Deep indigo gradient from `#2C2368` to `#06050D`, a volt halo, a low floor glow and a 4px rim. One beveled bolt from `#FFFFFF` to `#7B79FF` with a soft glow and a 26px shadow. No text and no play glyph. Source lives at `assets/brand/icon.svg` and renders to `assets/brand/icon-1024.png`. Regenerate every platform size with `pnpm tauri icon assets/brand/icon-1024.png`. The taskbar read is the test: 32px must still read as a bolt, not a blob.

### Footer

Sits at the bottom of the main scroll area, pushed down on short pages. 1px top border, content max 1440, 16 to 18px vertical padding. Left: 28px bolt tile, Kaminari wordmark, and `No content or sources are included.` Right: `Metadata by AniList`, Terms, Privacy, Licenses, and the version in mono. Terms and Privacy open a local dialog with tabs, a draft notice, focus trap, Escape close and focus restore. The page behind locks while the dialog is open. The footer adapts to both themes through tokens, no separate light styles.

### Licenses dialog

Opened from the Settings card or the footer. 980px max width, 88vh max height. Header: package icon, title, `Every package in this build and the license it declares.`, close. A search field filters package names, versions, license ids and sources. Summary shows package count, distinct license count and generation date. Groups: Bundled, Rust, JavaScript. Each row: name, versions in mono, license chip, source, and `Read license` when a text exists. One license text opens at a time in a focusable, scrollable pre. Notes list the Rust host triple, generation rules and the AniList service note. The data comes from `public/licenses.json`; never hand-edit it, run `pnpm licenses:build`.

## Motion

Card hover: 160ms ease out, up 4px, scale 1.03. Button hover: 150ms, up 2px. Fade for menus and tips: 100ms. Modal entry: 320ms ease out, up 12px plus fade, with a 220ms overlay fade. Toast entry: 250ms, up 8px plus fade. Hero crossfade: 600ms. Scrub: 100ms linear. Toggle pop: 300ms spring, sparing use. Animate transform and opacity only. Never width, height, top, left. Short sentences in code comments explain any exception.

Reduced motion: media query plus a Svelte guard. All durations drop near zero. Shimmer, drift, and autoplay stop.

## Window and shortcuts

Min size 1024 by 640. Content max 1440px, gutters 24px (32px at md and up). Tray menu holds Show and Quit. Shortcuts dialog opens with `?` and lists every key.

Global when not typing: Ctrl K or slash opens the palette, 1 to 5 move between rail views, question mark opens help, Esc closes top layer. Focus traps in modal and player. Focus returns on close. Skip link jumps to content. Context menu on cards mirrors hover actions so touch and keyboard reach the same verbs.

## Performance

Cold start under 500ms in release on a mid range laptop, warm cache, internal storage. Measure with the scaffold flag and again in release. Debug numbers do not count.

Grid of 500 posters scrolls at 60fps. That means virtualization, 240px thumbs, lazy load with observer, overscan of 4 rows. Search palette answers under 50ms locally, under 600ms p95 against AniList with debounce and cache. Play to first frame under 1500ms p95 on a cached source. Installer under 15MB before libmpv resources. CI fails past that. Image disk LRU holds 512MB with 30 day TTL. Blurhash ships in the media row.

## Accessibility

Two pixel accent ring with offset on focus visible. Body contrast 7 to 1 or better, captions 4.5 to 1 or better. Landmarks for nav, main, dialog. Icon buttons carry labels. Toasts use live regions. Player time is polite on pause and seek, silent during play. Episodes read as options with selected state. Touch targets in player hit 44px. Strings go through i18n keys from day one even while only English ships.
