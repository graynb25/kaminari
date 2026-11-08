# UI polish plan

Status: phase 1 shipped in 0.11.4. The phase 2 PageHeader atom shipped in 0.12.0. The remaining phase 2 atoms are open. Derived from a full read of Papyrus (one 1,136-line Tailwind 4 stylesheet, 86 Blade views) and Kaminari (24 Svelte components, 2,219 lines of `app.css`) on 2026-11-08. The conclusion is refinement, not a port. Kaminari already carries a close descendant of the same component grammar: poster card, hero, shelf, glass, toast, stat card, empty state, legal markdown, ranges and player surfaces.

The rule for this pass: finish the atoms and states, do not redesign the identity. Keep Sora, Inter, JetBrains Mono, the void/volt/paper palette, the 1440px container, the 12/16/20/24 radius scale and the 72px rail.

## What not to port

- Vermilion, warm paper and cool ink zinc. Identity is already coherent.
- Bricolage Grotesque, Instrument Serif and the serif voice.
- The rotated hanko seal. Kaminari uses the bolt, unrotated.
- The Reading Room cover wall, canvas ink garden, parallax and palette sampling. Too heavy for a Tauri cold start, and there is no login screen.
- Cover tilt, sheen and a second blurred artwork layer on every card. Noise at 500 posters.
- Glass on page content. The design spec keeps glass for overlays.
- Papyrus's 110rem width, 14px card radius and 3px rules.
- Flag icons and a second icon set. Use text language codes.

## Current gaps, measured

1. `app.css` has no cascade layers. Component classes beat Tailwind utilities, which pushes code toward inline `style`.
2. More than 125 literal accent and status RGB values across `app.css` and Svelte files. Light theme can drift because literals stay dark-tuned.
3. `.glass` sets only background and blur. Every dialog adds its own border and shadow inline.
4. `PosterCard` has no loading underlay, blur-up or broken URL fallback.
5. Poster focus ring is a 1px box shadow, below the 2px in `DESIGN-SPEC.md`.
6. Poster quick actions are hover and focus only. No touch fallback.
7. Library, History, Sources and Settings hand-roll the same page header.
8. Legal, Licenses, Shortcuts, SearchPalette and SourceBrowser repeat focus trap, Escape and panel material.
9. Callouts and progress bars are hand-rolled per page with different tones and borders.
10. Sources available and installed grids stop at three columns, leaving width unused at 1440 and 2560.
11. Settings is one long page with no section navigation.
12. Spec drift: empty icon 62px vs 48px, dialog radius 22px, toast and status menu 14px.

## Phase 1: tokens and feedback, 1 to 2 days

Shipped in 0.11.4.

- Add semantic aliases for accent and status washes, then replace literals. Existing base variables `--accent-rgb`, `--accent-bright-rgb`, `--accent-shadow` and `--line-volt` already support this.
- Add `text-wrap: pretty`, `font-optical-sizing: auto` and a `.nums` tabular utility.
- Add `.on-art` and apply it to poster badges, continue cards and source labels over art.
- Complete `.glass` and `.glass-strong` with border, inset highlight and shadow. Remove duplicate inline border and shadow from `SearchPalette`, `LegalDialog`, `LicensesDialog` and `ShortcutsDialog`.
- Add `.card-press` at scale 0.98, raise the poster focus ring to 2px, and add a `@media (hover: none)` fallback for quick actions and shelf arrows.
- Move the Sources grids to `1 / 2 / 3 / 4` at `sm / lg / xl`.

Gate: screenshots at 1024, 1280, 1600 and 2560 in Storm and Daylight, a keyboard pass, and a reduced motion pass. No new dependencies.

## Phase 2: atoms, 3 to 5 days

PageHeader shipped in 0.12.0. The rest are open.

- `PageHeader.svelte` for eyebrow, title, subtitle, action content passed as `children`, and the soft glow. Replace the four repeated headers.
- Artwork lifecycle in `PosterCard`: reserved frame, shimmer underlay, blur-up on decode, `complete` check for cache, and error fallback. Then reuse for continue cards, history thumbs, hero banners and detail banner.
- `stagger-grid` entrance on the library and source result grids, capped at 18 items so a 500 poster list does not animate everything.
- `BrandMark.svelte` for title bar, footer, empty state and Settings.
- `SegmentedControl.svelte` for detail tabs and future sort controls.
- Static theme preview under the theme picker: two miniatures with poster swatches, `aria-hidden`.

Gate: the same screenshots plus a 500 poster scroll check. No layout shift when images decode.

## Phase 3: structure, 1 to 2 weeks

- Move base and component CSS into `@layer base` and `@layer components`, keeping reduced motion and deliberate overrides unlayered. Move declarations, do not rewrite values. This is the highest risk item and needs the visual gate.
- `Modal.svelte` owning backdrop, panel material, focus trap, Escape, focus restore and the overlay lock. Migrate Legal, Licenses, Shortcuts and the palette shell. Keep SourceBrowser full screen but reuse the focus logic.
- `Callout.svelte` with info, success, warning and danger tones, plus `ProgressBar.svelte` with `role="progressbar"` and a transform based fill. Migrate the offline banner, library refresh, source registry, source search, resolve and install states.
- Settings anchor rail at `lg`, horizontal chips below, with `scroll-margin-top` and `aria-current="location"`.
- `FatalState.svelte` for boot failure, database corruption and view crashes. Use the brand mark and a mono error code.

Gate: focus trap tests for every overlay, keyboard only pass, light and dark screenshots.

## Phase 4: optional

- Player settings drawer: bottom sheet below `sm`, right panel above, speed presets, volume and subtitle sliders, tabular numerals.
- Scroll progress hairline on the main scroll area with a no-JS fallback.
- Episode ribbon for long series with watched, partial and current states.
- View Transitions API spike for poster to detail, behind a flag and measured in WebView2.
- Font fallback metric audit. Local fonts use `font-display: swap` and the spec forbids layout shift.

## Open decisions

1. Refinement only, or a visible refresh?
2. Cascade layers now, or after the atoms land?
3. Settings navigation now, or when playback, data and update sections exist?
4. Blur-up as the interim for the blurhash promise, or a Rust blurhash pipeline first?
5. Treat 62px empty icon and 22px dialog radius as canonical, or move to 48px and the 20/24 scale?
6. Is touch a real target for the quick actions?
