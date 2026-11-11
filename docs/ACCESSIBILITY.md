# Accessibility pass

Baseline: `main` at `f642049` (published 0.28.1). Work happened on local branch
`phase3/accessibility`. This pass is a focused audit and fix, not a redesign:
layout, branding, VERSION and CHANGELOG stay untouched.

## What was checked

- Modal and dialog semantics: dialog role, modal state, accessible name, initial
  focus, focus containment, Escape, focus return and background blocking.
- Keyboard paths in the rail, footer, library, history, source browser, player,
  search palette, getting started, quality selectors and the license and legal
  dialogs.
- Focus visibility, tab order, native activation, focusable non-interactive
  elements and closed keyboard traps.
- Async status regions and error announcements.
- State that could otherwise be conveyed by color alone.
- Text and focus contrast on both token sets.
- Reduced motion behavior in CSS and in the carousel and shelf helpers.

The audit covered `Modal.svelte`, `LegalDialog`, `LicensesDialog`,
`SourceMatchDialog`, `SourcePreferencesDialog`, `ShortcutsDialog`, the history
clear confirmation, the extension update confirmation, the source browser, both
player views, `SearchPalette`, `GettingStarted`, `QualitySelect`,
`PlayerQualityMenu`, `AppFooter`, `NavRail`, `LibraryView`, `HistoryView`,
`EmptyState`, `Callout`, `ProgressBar`, `Toasts` and the shared app shell.

## What was fixed

### Modal and dialog layer

- Added `src/lib/a11y.ts` with one shared focus and layer implementation:
  counted overlay lock, a top-layer stack, a focusable-element query, Tab
  containment, and background isolation with `inert` plus `aria-hidden`.
- `Modal.svelte` now accepts `labelledBy`, so every dialog is named by its
  visible heading instead of a duplicate string. `label` remains a fallback.
- The dialog focuses itself first, then moves to `[data-autofocus]` or the first
  control on the next frame. The opener is captured before focus moves and is
  restored when the top layer closes.
- Escape is handled only by the top layer and stops immediately, so closing a
  dialog no longer also closes the detail page underneath. Tab is contained in
  the panel, and a focus-in guard pulls stray focus back inside.
- Siblings outside the dialog's ancestor chain become inert and hidden from
  assistive technology. Toasts stay outside that isolation so notifications
  still announce. Stacked layers unwind in order, and the overlay name is
  restored to the layer below.
- `SourceBrowser`, `PlayerView` and `MpvView` use the same layer stack and focus
  utilities. The source browser now uses the shared overlay lock instead of
  writing `body.dataset.overlay` directly.
- Every dialog with a visible heading now passes a heading id: legal, licenses,
  source match, source preferences, shortcuts, getting started, history clear,
  extension update and the search palette. The player dialogs are named by the
  title they display, with the episode as the description.

### Keyboard and focus

- `SearchPalette` follows the combobox pattern: the input owns focus and
  `aria-activedescendant`, results are listbox options, and Tab no longer walks
  every result. The palette input gets a visible 2px inset focus ring.
- `HistoryView` time range menu now opens onto the checked item, supports Arrow
  Up and Down, Home and End, closes on Escape and returns focus to the trigger.
  Its clear confirmation is named by its heading.
- `QualitySelect` moves focus into the list, supports arrow navigation, closes
  on Escape with focus restored, labels the trigger with the current value, and
  closes when focus leaves.
- `PlayerQualityMenu` moves focus to the active stream, supports arrow keys,
  closes on Escape without closing the player, and returns focus to the quality
  button. Arrow keys inside the menu no longer reach the player volume shortcut.
- `PlayerView` and `MpvView` keep controls visible while a control contains
  focus, restore focus to the dialog shell after Retry, and move focus to the
  primary action when playback ends or fails.
- The source library Resume and Open buttons now include the title in their
  accessible names.
- History row labels include watched or in-progress state, so the state does not
  disappear when the status chip is hidden at small widths.
- `#content:focus-visible` now has a visible inset ring when the skip link moves
  focus there.

### Status and errors

- Loading states have polite live text in the library, history, licenses, source
  match, source preferences, source browser, search palette and players.
  Skeletons and spinners remain decorative.
- Errors use `role="alert"` in the history load block, license load block,
  source search empty state, source browser search, both player error panels and
  the existing callouts. Download completion and failure still use toasts,
  which are already live regions with an alert role for errors.
- `SourceMatchDialog`, `SourcePreferencesDialog`, `SourceBrowser` and the source
  forms expose `aria-busy` where a request is in flight.
- Player status live regions now announce only playing, paused, buffering,
  finished or failed. The previous region included `formatTime(currentTime)`, so
  a screen reader could announce a new timestamp on every `timeupdate`.
- Player seek and volume sliders expose readable `aria-valuetext` values.
- Fill, diagnostics and fullscreen controls expose `aria-pressed`.

### Contrast and motion

- No new colors were introduced. Two tokens moved to meet contrast requirements
  on their own surfaces:
  - Dark `--paper-500` (text-4) changed from `#7a7893` to `#82809a`.
  - Light `--danger` changed from `#e11d48` to `#d91443`.
- Measured contrast after the change:
  - Dark text-4 on `--surface-2` `#141425`: 4.76:1 (was 4.27:1).
  - Dark text-4 on `--surface-1` `#0f0f1c`: 4.98:1 (was 4.46:1).
  - Light danger on `--void-950` `#f6f7fc`: 4.75:1 (was 4.39:1).
  - Light danger on white: 5.08:1 (was 4.70:1).
  - Focus ring `--volt-400`: 6.49:1 on dark `--surface-1` and 4.82:1 on white.
  - Body text tokens remain above 7.9:1 on the tested surfaces.
- The global reduced-motion block now also zeroes animation and transition
  delays, so the hero copy stagger does not slowly reveal after motion is
  reduced. Existing reduced-motion handling for shimmer, toasts, modal entrances,
  poster and source card transforms, artwork blur-up, shelf scrolling and the
  hero carousel autoplay was verified in source.

## Settings status blocks

`SettingsView.svelte` was not edited because another session owns it. Read-only
findings:

- The WebView data, clear local data, AniList and data status blocks use
  `role="status"` even when the message is a failure. They announce politely, but
  a failed clear, backup or AniList action should use `role="alert"` or an
  assertive live region so it interrupts.
- The update status block uses `role="status"` with `aria-live="polite"`, which
  is fine for progress, but its error tone is still polite.
- The clear all local data modal relies on `Modal`'s `label` fallback because its
  visible heading has no id. It still has a valid accessible name, initial focus
  on Cancel, a trap, Escape and focus return through `Modal`.
- The Incognito On and Off button has `aria-pressed` and a title, but no explicit
  `aria-label` tying it to the Incognito heading. Its visible state is text, not
  color alone, so this is a naming improvement rather than a blocker.

## Remaining issues and caveats

- No live Tauri WebView or screen reader session was run for this pass. The
  headless browser smoke used a mocked Tauri bridge and the browser's own
  accessibility tree; it did not exercise WebView2, Narrator, NVDA or native
  file dialogs. The next useful step is a keyboard and Narrator or NVDA pass
  through every dialog, the source browser and both players.
- `DetailPage` status menu and the Sources registry rows were outside the
  requested file list. They inherit the shared Modal behavior but were not
  individually keyboard-audited.
- A native select dropdown inside a modal may consume Escape itself in some
  WebView builds. The modal layer closes only after the page receives the key.
  This needs a live check with a source that exposes list preferences.
- `PlayerQualityMenu` is a non-modal popover with a dialog role and its own
  focus handling. It is intentionally not part of the modal layer stack and does
  not make the player background inert while open.
- The `hls.js` chunk size warning from Vite is unrelated to this pass.

## Verification

- `pnpm install --frozen-lockfile`: lockfile up to date, 75 packages installed.
- `pnpm check`: TypeScript passes.
- `pnpm test`: 117 tests pass in 19 files, including the new focus-order tests.
- `pnpm build`: production build passes. The only warning is the existing
  `hls.js` chunk above 500 kB.
- Style check: no em dash characters in tracked text files.
- `git diff --check`: no whitespace errors.
- Headless Playwright smoke against the Vite dev server with a mocked Tauri
  bridge, no page errors:
  - Getting started is named by `getting-started-title`, focuses Add a source,
    marks the app shell inert and restores it on close.
  - The skip link focuses `#content` and shows a visible outline.
  - Legal is named by `legal-dialog-title`, focuses Close, keeps focus inside
    through ten Tabs, and returns focus to Terms after Escape.
  - Licenses is named by `licenses-dialog-title`, focuses the search field and
    filters to the hls.js row.
  - Shortcuts is named by `shortcuts-dialog-title`, focuses Close and returns
    focus to the rail button.
  - The palette input reports `aria-expanded` and
    `aria-activedescendant="palette-row-0"`, then moves to row 1 on Arrow Down
    while focus stays in the input.
  - The history range menu opens on the checked item, responds to Arrow Up and
    Home, closes on Escape with focus on the trigger, reopens on Arrow Down, and
    the clear confirmation is named by `history-clear-title`, focuses Cancel and
    returns focus to Clear history on Escape.
- Manual verification is still required in the running Tauri WebView, especially
  for focus return when a dialog is opened from the footer, the source browser to
  player to source browser chain, and the palette's active descendant announces
  the same row that is visually highlighted.
