# Airing schedule plan

Status: implemented locally on branch `feat/airing-schedule` at commit `4288877`, plus documentation and the 0.29.0 local candidate. Not published. This plan adds a read-only schedule view over the existing library. It does not add content, sources or playback paths.

## Why this is possible

AniList exposes `Media.nextAiringEpisode { airingAt episode timeUntilAiring }` for releasing anime. A one-node `airingSchedule(notYetAired: true)` connection can supply the first upcoming episode when `nextAiringEpisode` is empty. `Page.media(id_in: [...])` accepts up to 50 AniList ids per request, so a normal library refreshes in one or two requests.

Installed Mangayomi sources do not expose reliable episode air times. The schedule must come from AniList. A source-only show without an AniList link cannot be scheduled. The view should say so instead of guessing.

AniList times are Unix timestamps. The app groups them by local calendar day and shows them in the local timezone. No local date is stored.

## What the reference gets right

The reference is calm and scannable: a day strip, a time column, one row per airing, and a small episode badge. We keep that structure. We add the parts the request asks for:

- a poster thumbnail on every row;
- a next-up card with the nearest countdown;
- a live countdown in each future row;
- an `Aired` state for earlier entries on the same day;
- a `Later` section for entries beyond the seven-day window;
- loading, empty, offline and error states that match the rest of the app.

The reference is a narrow mobile capture with a three-day carousel. Kaminari is desktop-first at 1024px minimum, so seven days fit across the page. The selected day stays visually dominant.

## Measured library

A read-only probe of the local database on 2026-11-12 found:

- 45 AniList library rows and 25 linked source rows, with 45 distinct AniList ids.
- 10 library shows releasing with a next episode.
- 9 of those air inside the next seven days. One airs much later.
- The busiest days hold two or three shows. Several days are empty.

This is a sparse schedule. The view must not open on an empty day. It defaults to today when today has entries, otherwise to the next day with entries. A whole-week agenda is not enough on its own because the countdown and the day context are the point.

The probe also confirmed that linked source ids are currently a subset of the AniList library ids. The union costs nothing today and keeps linked shows from disappearing later.

## Scope

In:

- AniList library entries whose AniList media is releasing and has a usable next airing entry.
- Source-library entries with an `anilist_id`, deduped against the AniList library by id.
- Local statuses `watching`, `planning`, `completed` and `paused`. `dropped` is excluded.
- A next-up card, a rolling seven-day strip, a timeline for the selected day, and a Later section.
- A cached snapshot that survives restart and offline use.

Out:

- Scraping source pages for air times. Sources are not a schedule source.
- Notifications, background downloads and auto-play of a newly aired episode.
- A month calendar, genre or status filters, and per-user timezone overrides.
- Progress, library status or AniList sync writes from the schedule. Rows open the show; they do not change state.
- More than one future episode per show. A weekly show is one row per week. A daily show shows only its next episode in v1. The query already asks for the next node, so expanding this later is a contained change.

## Placement

Recommended: a sixth rail item, `Schedule`, after Browse, with key `2`. Library, History, Sources and Settings move to keys 3, 4, 5 and 6. The `View` type, view names, shortcut handler, Shortcuts dialog and DESIGN-SPEC nav paragraph change together. Six items fit the 72px rail.

Alternatives:

- A `Grid | Schedule` segmented control inside Library. No rail change, but the schedule is hidden behind a library management screen.
- A Home shelf only. The reference is a page, and a shelf cannot carry the day strip. Not recommended.

## Layout

```
Estimated
Airing Schedule                                    Updated 4:12 PM  [refresh]
10 shows airing. Next episode in 1d 15h.

[ next-up card: poster 96x144 | title, EP 8, Sat Nov 14, 8:00 AM | 1d 15h 22m ]

[ Thu 12 ] [ Fri 13 ] [ Sat 14 ] [ Sun 15 ] [ Mon 16 ] [ Tue 17 ] [ Wed 18 ]
                         selected

08:00  [poster] Example Show A                    EP 8    in 1d 15h
08:30  [poster] Example Show B                    EP 8    in 1d 16h
09:00  [poster] Example Show C                    EP 7    in 1d 16h

Later
Jan 3  [poster] Example Show D                    EP 12
```

- Header uses `PageHeader`, eyebrow `Estimated`, title `Airing Schedule`.
- Subtitle: `N shows airing. Next episode in ...`, `Airing now.`, or `Nothing airing this week.`
- Right side: last updated time and a refresh button. The button is disabled during a request and shows a spinner.
- Next-up card: 96x144 poster, title, episode, exact local date and time, and a large countdown. The whole card opens the show.
- Day strip: seven buttons with weekday and date. A count dot marks days with entries. The selected day is larger and accent colored. Empty days stay selectable but dimmed. Keyboard Left and Right move days; Home and End jump.
- Timeline: rows sorted by `airing_at`. A 2px left rail with one dot per row. Then local time in mono, a 48x64 poster, the title, a meta line (`EP 8 of 12`), and a countdown pill. Past rows are dimmed and read `Aired`. The whole row opens the show.
- Later: future entries beyond seven days, exact date and time, no countdown. Show the group collapsed when it holds more than five rows.
- Footer note: `Times come from AniList and are estimates. Shown in your local timezone (EST).` Read the abbreviation from `Intl.DateTimeFormat`, do not hardcode it.

## Interaction

- A row opens the AniList detail page when the show is in the AniList library. Otherwise it opens the linked source show.
- `Enter` opens the focused row. `R` refreshes when the view has focus and focus is not in an input. Left and Right move the day strip. No other shortcuts.
- The selected day survives a refresh. If it loses all entries, fall back to today or the next day with entries.
- The view never autoplays.

## Time and countdown rules

- Store and transport `airingAt` as Unix seconds. Never store a local date.
- Group by local calendar day with `Intl.DateTimeFormat` parts. Do not add 24-hour offsets. That keeps DST correct.
- Display times with the system locale: `15:26` on a 24-hour locale, `3:26 PM` on a 12-hour locale.
- Countdown formats:
  - 24 hours or more: `in 2d 4h`
  - 1 hour to 24 hours: `in 5h 12m`
  - 1 minute to 1 hour: `in 12m`
  - under 1 minute: `in 40s`
  - 0 to 30 minutes after `airingAt`: `Airing now`
  - later: `Aired`
- Tick once per second while the view is mounted and `document.visibilityState === "visible"`. Recompute from `airingAt` every tick. Pause when hidden. No background timer.
- Do not put countdowns in a live region. The row's accessible name carries the exact local date and time, for example `Saturday, November 14 at 8:00 AM, in 1 day 15 hours`. The visible countdown updates silently.
- The next-up countdown uses the same helper at display size.

## Data flow

1. On entering the view, read the cached schedule from SQLite and render it if present.
2. If there is no cache, the cache is older than 15 minutes, or the library id set changed, fetch in the background.
3. The backend collects distinct AniList ids from `library` (excluding `dropped`) and `source_library` where `anilist_id` is not null.
4. One `Page.media(id_in: [...])` request per 50 ids, with `nextAiringEpisode` and a one-node `airingSchedule(notYetAired: true)` fallback.
5. Keep entries with a usable `airingAt`. Update the cache in one transaction on a fully successful fetch. Replace future rows, keep past rows from the last 48 hours so today still reads as a timeline after an episode airs.
6. Return `items`, `fetchedAt`, `stale` and an optional `warning`. A matching snapshot returns immediately even when stale; the view uses `stale` to trigger `force=true` in the background. On network failure with a cache, return the cache plus a warning instead of an error.
7. The frontend joins each entry by `anilistId` to `library` and `sourceLibraryItems` for title, cover, status and source info. The cache does not duplicate media rows.

Multiple rows per show are allowed. When an episode airs and AniList advances to the next one, the old row stays for up to 48 hours and the new row is added. The selected day then shows the aired episode and the next episode is in its own day.

## AniList query

```graphql
query Airing($ids: [Int]) {
  Page(page: 1, perPage: 50) {
    media(id_in: $ids, type: ANIME) {
      id
      title { romaji english native }
      coverImage { extraLarge large color }
      format
      episodes
      status
      nextAiringEpisode { airingAt episode timeUntilAiring }
      airingSchedule(notYetAired: true, perPage: 1) {
        nodes { airingAt episode }
      }
    }
  }
}
```

The query uses public media ids and works without an AniList token. The existing token bucket and `Retry-After` handling apply. A token may be attached by the shared client, but the schedule does not depend on it.

## Cache

New migration `0008_airing_schedule.sql`, schema version 4:

```sql
CREATE TABLE IF NOT EXISTS airing_schedule(
  anilist_id INTEGER NOT NULL,
  episode INTEGER NOT NULL,
  airing_at INTEGER NOT NULL,
  PRIMARY KEY (anilist_id, airing_at)
);

CREATE INDEX IF NOT EXISTS airing_schedule_airing_at
  ON airing_schedule(airing_at);
```

Snapshot time lives in `app_meta` under `airing_schedule_fetched_at`. That keeps the "fetched successfully, nothing airing" case from refetching on every open, and it avoids a fake row.

- No foreign key to `media`. A linked source show can be scheduled before an AniList media row exists.
- Update in one transaction: delete ids no longer in the library, delete rows older than 48 hours, delete future rows for the ids being refreshed, insert the new snapshot, then update the snapshot time. Past rows stay so the selected day keeps its aired entries.
- Upsert on `(anilist_id, airing_at)` and update `episode` if AniList corrects a number.
- On failure, keep the previous snapshot.
- The cache is local and included in the existing database backup.
- Add `airing_schedule` to `DIAGNOSTICS_TABLES` as a row count. It holds no titles or personal data, only ids and times.
- No image cache is added. AniList covers use the existing `Artwork` path.

## Refresh and offline

- Freshness window: 15 minutes.
- Auto-refresh: every 30 minutes while the view is open and visible. Pause when hidden.
- After the nearest future `airingAt` passes, schedule one refresh 5 minutes later so the next episode replaces it. Debounce to one request.
- Manual refresh always forces a fetch.
- Offline with cache: show the cache, a warning callout, the saved time and running countdowns. Do not clear the cache.
- Offline without cache: an empty state that asks for a connection and offers Retry.
- AniList error with cache: stale data plus a warning banner and Retry. Rate limits use the existing message.
- AniList error without cache: an error state with Retry. No blank panel.

## Empty, loading and error states

- Loading: a skeleton with the next-up card, seven day chips and four rows. Reserve heights. No layout shift.
- No library: `Save airing anime to your library and their next episodes will appear here.` Actions: Browse, Search.
- Library but nothing airing: `Nothing in your library is airing right now.` Action: Browse airing.
- Selected day empty: `No episodes on this day.` Action: jump to the next day with entries, if any.
- Offline, no cache: `The schedule needs AniList once. Reconnect to load it.` Action: Retry.
- Error, no cache: name the failure and offer Retry.
- Every state keeps the page header and the timezone note so the view never looks broken.

## Backend shape

Rust:

- `anilist.rs`: `AiringScheduleNode`, `AiringScheduleEntry`, `airing_schedule(&self, ids: &[i64])` with batching, parsing and tests.
- `db.rs`: `airing_schedule_ids`, `airing_schedule_cached`, `airing_schedule_replace`, `airing_schedule_fetched_at`. Migration 0008. Tests for empty, replace, 48-hour retention, removal and freshness.
- `commands.rs`: `airing_schedule(force)` combines the database and client, returns a matching snapshot immediately with `stale`, fetches on force or when no snapshot exists, returns cached data on failure, and never writes user progress.
- `main.rs`: register the command.

Frontend:

- `types.ts`: `AiringEntry`, `AiringSchedule`, and `View` gains `schedule`.
- `api.ts`: `airingSchedule(force = false)`.
- `App.svelte`: `schedule` state, loading, error, fetched time, refresh function, view route, timer cleanup.
- `ScheduleView.svelte`: header, next-up card, day strip, timeline, later, states. Props: entries joined to library, loading, error, fetchedAt, offline, onRefresh, onOpen, onBrowse.
- `countdown.ts` and `scheduleDays.ts`: pure helpers with Vitest coverage.
- `NavRail.svelte`, `ShortcutsDialog.svelte`, `types.ts`, `App.svelte` key handler and `DESIGN-SPEC.md`: add Schedule and renumber keys.
- `app.css`: `.schedule-*` classes using existing tokens. No new color literals. Reuse `PageHeader`, `Artwork`, `Callout`, `EmptyState` and `Icon`.

Detail page, optional in the same change:

- Add `nextAiringEpisode` to the Rust `Media` type and `MEDIA_FIELDS`.
- In `DetailPage.svelte`, show a single line under the meta row: `Next episode: Sat, Nov 14 at 8:00 AM, in 1d 15h`. Use the same countdown helper.
- Set the field to `None` in cached database media constructors. The detail query fetches it fresh.

## Countdown and day helpers

`countdown.ts`:

- `countdownLabel(airingAt, now)` returns `{ text, tone, aria }`.
- `futureDuration(airingAt, now)` returns the compact text.
- Boundaries at 24 hours, 1 hour, 1 minute, 30 minutes after airing, and the past.

`scheduleDays.ts`:

- `rollingDays(now, timeZone?, count = 7)`.
- `dayKey(airingAt, timeZone?)`.
- `groupByDay(entries, days, timeZone?)`.
- `nextDayWithEntries(days, selectedKey)`.
- `isToday`, `formatDayLabel`, `formatTime`.
- Tests use fixed zones `America/New_York` and `Asia/Tokyo` around midnight and a DST change. The app calls them with the local zone.

## Accessibility

- The day strip uses `role="tablist"`, `role="tab"`, `aria-selected`, and a `tabpanel` for the timeline. Left and Right move, Home and End jump.
- Rows are links or buttons, never nested interactive elements. The whole row is the target.
- Focus visible uses the existing 2px accent ring with 2px offset.
- The poster is decorative in a row (`alt=""`); the title carries the name.
- The next-up card has a heading and a readable countdown.
- No entrance animation beyond the existing page swap. Reduced motion changes nothing because countdowns only change text.

## Performance

- At most a few dozen rows. No virtualization.
- One 1-second interval while visible, one auto-refresh timer, one post-airing timer. All cleared on view exit.
- Poster thumbnails are 48x64 and lazy. The next-up poster is 96x144 and eager.
- One AniList request per 50 ids, at most a handful per refresh.
- No new dependency.

## Test plan

Rust:

- Parse a response with `nextAiringEpisode`, one with only `airingSchedule`, one with both, and one with neither.
- Batch 0, 1, 50 and 51 ids.
- Replace the snapshot, keep a 48-hour aired row, remove a show, keep the old cache on a failed refresh.
- Freshness boundary: 14:59 returns cache, 15:01 fetches, using a pure helper or a fake clock.
- A changed library id set forces a fetch even inside the freshness window.
- No progress, library or AniList sync writes from the schedule command.

Frontend:

- Countdown boundaries and past, airing and future states.
- Day grouping across midnight, DST and two timezones.
- Default day is today when it has entries, otherwise the next day with entries.
- Dedupe by AniList id; prefer the AniList library row; keep the source link for open behavior.
- Empty, loading, error and offline props render the expected copy and actions.
- No live-region spam.

Manual:

- 1024, 1280, 1600 and 2560 widths.
- Storm and Daylight.
- Keyboard only: nav, day strip, rows, refresh.
- Offline with and without cache.
- AniList rate limit and server error.
- Library with no airing shows, one show, and a show more than seven days out.
- Reduced motion.
- A linked source show opens the source, not a dead AniList detail page.

## Phases and estimate

1. Backend and cache, 0.5 to 1 day: migration, AniList query, command, tests.
2. View, 1 to 1.5 days: rail item, day strip, next-up card, timeline, later, states, CSS.
3. Detail line and polish, 0.5 day: optional next-episode line, accessibility and manual passes.
4. Docs and candidate, 0.5 day: DESIGN-SPEC, ARCHITECTURE, ENGINEERING-NOTES, CHANGELOG, VERSION 0.29.0 candidate. No push or release without approval.

Total: 2.5 to 3.5 working days. No new dependencies. One local branch, local commits.

## Acceptance criteria

- The rail has a Schedule view and keys 1 to 6 work.
- The view shows only library shows with a real AniList airing entry, deduped by AniList id.
- Times are local, day grouping is local, and DST does not move an entry to the wrong day.
- The next-up card shows the nearest future episode and a countdown that updates once per second while visible.
- Past episodes on the selected day are dimmed and marked `Aired`, not hidden.
- A show beyond seven days appears in Later with an exact date.
- Offline with cache keeps the schedule and countdowns, with a clear saved-time note.
- No cache plus offline shows an actionable empty state.
- No source requests are made, no progress is written, and no new permissions or dependencies are added.
- Rust tests, frontend tests, fmt, clippy, build, style and audit pass.

## Decisions

Set on 2026-11-12 from the planning review. The recommended option was the default in each case.

1. Placement: dedicated Schedule rail item after Browse, key 2. Library, History, Sources and Settings move to keys 3, 4, 5 and 6.
2. Scope: AniList library plus linked source shows, deduped by AniList id. `dropped` is excluded. A show with no progress still appears if it is in the library and releasing. Library membership is the rule, not progress.
3. Layout: day strip plus timeline with poster thumbnails, a next-up card and a Later group.
4. Window: rolling seven days from today. The Later group holds entries beyond that window.
5. Detail page: add the next-episode line in the same change.
6. Notifications: out of v1. A Windows toast feature needs its own plan.

## Deferred

- More than one future episode per show. A daily show shows only its next episode in v1.
- A month calendar, genre and status filters, and per-user timezone overrides.
- Notifications, background downloads and auto-play of a newly aired episode.

## Implementation

Local branch `feat/airing-schedule`, 2026-11-12. Backend `b9ca160`, helpers `99ded4c`, view `1c51270`, refresh fix `47936ed`, day fix `4ee2b48`, detail line `4288877`. The branch passes 234 Rust tests and 146 frontend tests, plus fmt, clippy, frontend check, build, style, diff and the dependency audit. Visual QA against a copied database covered light and dark themes, the default day, the empty-day jump, the Aired state, the Later group, the Wednesday Sasaki slot and the detail-page next-episode line. The real database stayed on schema 3. No push, tag, release or GitHub Actions run.
