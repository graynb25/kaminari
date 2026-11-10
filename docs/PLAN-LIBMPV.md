# Plan: embedded libmpv playback

Status: P0 spike complete on Windows. The DLLs are bundled from the gitignored `src-tauri/lib` payload, the embedded path is behind the default-off `kaminari.libmpv` flag, and PlayerView is still the default. A draft legal review packet exists in `docs/LEGAL-REVIEW-DRAFT.md`; public distribution waits for counsel review. This doc is the build spec. Read with `docs/PLAN-STREAM-PROXY.md`, `docs/DESIGN-SPEC.md` section VideoPlayer, and `docs/ENGINEERING-NOTES.md` player entries.

Goal: play H.264, HEVC, AV1, HLS, ASS, and HDR inside Kaminari with custom controls, keep the loopback proxy for protected origins, keep external mpv as fallback.

Date: 2026-11-08. Sources checked this day unless noted.

## 1. Current flow and why the WebView fails

Current files and functions:

1. `src/lib/SourceBrowser.svelte`, `resolve(episode)` calls `jsExtensionResolve` in `src/lib/api.ts`, which invokes `js_extension_resolve` in `src-tauri/src/commands.rs`. That calls `ext_host::run` with op `resolveEpisode`. Result is `JsStream[]`: `kind` (`hls`, `mp4`, `dash`, `embed`), `url`, `quality`, `headers`, `subtitles`.
2. `playStream` in the same file decides proxy use via `shouldProxy` plus `proxyFlagEnabled` in `src/lib/streamProxy.ts`. Current rule: proxy only when headers exist and kind is MP4 or HLS. It calls `proxy_register` with extension id and stream, gets `proxiedUrl` plus `proxyId`, mounts `PlayerView`.
3. `src/lib/PlayerView.svelte`, `setup()` branches on `stream.type === "hls"` or `.m3u8` regex. MP4 sets `video.src`. HLS lazy imports `hls.js`, calls `loadSource` plus `attachMedia`. No headers are set client side. Subtitles render as `<track>` elements.
4. Headers only reach external mpv: `openExternal` calls `playerPlay`, which calls `player_play` in `commands.rs`, which calls `PlayerState::play` in `src-tauri/src/player.rs`. `build_args` maps headers to `--http-header-fields` after stripping CR and LF. Subtitles pass as `--sub-file`. One child process at a time. Killed on next play and on exit.
5. Loopback proxy in `src-tauri/src/proxy.rs` binds `127.0.0.1` on ports 39321 to 39328, serves `/s/:id/file`, `/s/:id/manifest.m3u8`, `/s/:id/seg/:child`, plus `/healthz`. CSP in `tauri.conf.json` lists those 8 origins in `connect-src` and `media-src`.
6. Diagnostics in `PlayerView` show state, stream type, host, time, speed, headers requested or not, proxied yes or no. Copy redacts tokens.

Why the WebView fails for the target formats:

* Codecs: WebView2 uses Chromium media. H.264 works. HEVC works only on some Windows builds with platform decoders and license, not reliably. AV1 works in recent Chromium but 10 bit HDR AV1 plus high bitrate anime often falls back to software and drops frames. There is no app control over this. Result: stall, error code 4, or silent downscale.
* Containers: `<video>` handles progressive MP4 and MSE based HLS through hls.js. MKV, multi track, ordered chapters, and segmented VTT in HLS variants fail or lose tracks.
* Subtitles: native `<track>` supports WebVTT only. ASS and SSA lose position, color, fonts, karaoke, and overlap handling. Extension `subtitles` that point at ASS files cannot render correctly.
* HDR: WebView does no tone mapping control. HDR10 and Dolby Vision metadata pass through only if the OS chain supports it. The app cannot pick `gpu-next`, target primaries, or SDR fallback. Colors wash out or clip.
* Headers and cookies: `<video>` and hls.js cannot set `Referer`, `User-Agent`, or seed `Cookie` per segment. That is why the proxy exists. The proxy fixes auth, not decoding.
* Track selection and properties: no API for audio track, subtitle delay, hwdec, or deinterlace. `PlayerView` cycles `textTracks` only.

Net: proxy plus WebView covers header protected H.264 MP4 and single variant HLS. It does not cover HEVC, ASS, HDR, or complex containers. That gap is the reason for embedded libmpv.

## 2. Options

Three paths were reviewed against the installed crate source in `~/.cargo/registry/src/.../tauri-plugin-libmpv-0.3.2` and the public READMEs fetched 2026-11-08.

### Option A: tauri-plugin-libmpv 0.3.2 (recommended)

What it is: a Tauri plugin by nini22P. Rust side loads a dynamic `libmpv-wrapper` library with `libloading`, creates one mpv handle per window label, forwards `command`, `set_property`, `get_property`, plus `set_video_margin_ratio`. Events arrive as JSON strings through an FFI callback and are re emitted as `mpv-event-<label>`.

Native surface: `wid` mode. `src/desktop.rs` reads the Tauri window handle via `raw-window-handle`, converts it with `get_wid` in `src/utils.rs` (Win32 HWND, Xlib window, Xcb id, AppKit NSView), and sets mpv option `wid` unless the caller overrode it or the session is audio only. Verified in source: `init_wid_mode` in `desktop.rs`, lines 76 to 164.

Event loop: mpv runs its own thread inside `libmpv-2.dll`. The wrapper posts JSON events to the Rust callback, which uses `tauri::async_runtime::spawn` to emit. Commands from JS go through `spawn_blocking` so the async runtime never blocks. Properties use JSON strings across FFI with explicit free. Observed properties are declared at init, for example `pause`, `time-pos`, `duration`.

Maturity: version 0.3.2 in the local registry. CHANGELOG shows 0.0.1 first release, 0.2.0 moved to `tauri-plugin-libmpv` and LGPL-2.1, 0.3.0 replaced custom bindings with the wrapper library and relicensed to MPL-2.0, 0.3.1 added setup script. GitHub shows 23 stars and 4 forks on 2026-11-08. Small single maintainer. Risk is abandonment, not complexity.

Packaging: two DLLs on Windows, `libmpv-wrapper.dll` plus `libmpv-2.dll`, live in `src-tauri/lib/` and are bundled through `bundle.resources` as `lib/**/*`. The DLLs stay gitignored, and `pnpm check:libmpv` verifies their pinned hashes before a Tauri bundle. The plugin searches exe dir then exe `lib` dir at runtime (`get_wrapper` in `desktop.rs`). The README setup script downloads the wrapper from `nini22P/libmpv-wrapper` releases and libmpv from `zhongfly/mpv-winbuild` releases. Manual setup is documented for all three OSes.

Cross platform: README platform table on 2026-11-08 says Windows fully tested, Linux experimental with window embedding not working, macOS not tested. `utils.rs` returns an explicit error on Wayland because `--wid` embedding is unsupported there. X11 works in theory. macOS passes NSView but has no test report.

### Option B: direct libmpv FFI owned by Kaminari

What it is: link `libmpv-2.dll` directly with `libloading`, write a small `mpv_client.h` binding for `mpv_create`, `mpv_set_option`, `mpv_observe_property`, `mpv_command`, `mpv_wait_event`. Optionally use `mpv_render_context` with OpenGL for full control over compositing.

Pros: no wrapper hop, no MPL plugin dep, full control over render path, HDR flags, and hwdec. Cons: Kaminari owns all unsafe code, thread wakeups, event pump, string lifetimes, and render context setup. That is 4 to 8 weeks of extra work plus ongoing maintenance for each mpv ABI bump. Only justified if Option A is abandoned or if `wid` compositing proves unable to meet the overlay spec.

### Option C: separate mpv window (current fallback)

What it is: `src-tauri/src/player.rs` today. Finds `mpv.exe` via `KAMINARI_MPV`, PATH, common install paths, then `src-tauri/bin`. Spawns one process, passes headers and start time, kills on next play.

Pros: proven on 2026-11-08 with mpv 0.42.0. Zero packaging work. Full codec support today. Cons: breaks `docs/DESIGN-SPEC.md` VideoPlayer route. No glass bar over video, no 10 second countdown for Up Next, no focus trap, progress depends on close polling, window z-order fights the frameless shell. Acceptable as fallback, not as primary.

Recommendation: ship Option A on Windows first. Keep Option C as the automatic fallback. Defer Option B unless the plugin blocks on Wayland, macOS, or HDR control. Do not add VLC, Electron, or a Node sidecar. That matches `AGENTS.md`.

## 3. Licensing risk, marked S

Severity S. A mistake here affects every install, not one session. This must be resolved before shipping any bundled binary that contains libmpv. This is not legal advice. Have a qualified lawyer review before release.

Facts with sources and dates:

* Kaminari is MIT. Root `LICENSE` says MIT, copyright 2026 graynb25, checked 2026-11-08.
* `tauri-plugin-libmpv` is locked at 0.3.2 in `src-tauri/Cargo.lock` (source `registry+https://github.com/rust-lang/crates.io-index`, checksum `447798aec64877a1e476c2ebc2360a8374740ccf32fdc31949e4942e3fef903e`). Both `Cargo.toml` and `Cargo.toml.orig` in the local registry copy declare `license = "MPL-2.0"`, and the crate `LICENSE` file is Mozilla Public License Version 2.0. Checked in the local registry on 2026-11-08.
* `libmpv-wrapper` v0.1.1 is pinned by archive SHA-256 `D2FF8B2EDCD34D2968E544ADAA915E5E5C48EB1A0995945005269C2AF119A492` and its local `LICENSE` is LGPL-2.1. It is loaded at runtime as a separate DLL. Checked from the pinned release zip on 2026-11-08.
* mpv as a whole is GPLv2 or later by default. The mpv `Copyright` file in `mpv-player/mpv` on master, fetched 2026-11-08, says the mpv program is LGPLv2.1 or later only if built without GPL only files, using the `-Dgpl=false` switch. It lists GPL only files that get excluded (for example `vo_x11.c`, `vo_vdpau.c`, `ao_jack.c`, DVB and DVD code, man pages). It warns that linked libraries still affect the final license, for example if FFmpeg was built as GPL.
* `zhongfly/mpv-winbuild` README, fetched 2026-11-08, publishes two Windows artifacts: `mpv-dev-xxxx.7z` (libmpv) and `mpv-dev-lgpl-xxxx.7z` (libmpv under LGPLv2.1, disables LGPL incompatible packages, statically links FFmpeg under LGPLv3). The author adds they are not a lawyer and cannot guarantee all incompatible packages are disabled, use at own risk.

What this means for Kaminari:

* An LGPL build is possible. Use only the `mpv-dev-lgpl` artifact for `libmpv-2.dll`. Never ship the default GPL `mpv-dev` artifact. A GPL libmpv linked into the app distribution would require the whole combined work to be offered under GPL, which conflicts with the MIT distribution posture and the current docs.
* Dynamic linking matters. Ship `libmpv-2.dll` and `libmpv-wrapper.dll` as separate side by side DLLs. Do not static link libmpv into `kaminari.exe`. Dynamic DLLs keep the LGPL relink right practical: a user can drop in a replacement DLL.
* The distributed app must disclose and offer source:
  1. Ship the LGPLv2.1 text, the LGPLv3 text for the bundled FFmpeg, the MPL-2.0 text for the plugin, and the mpv `Copyright` attribution. The Native libraries group in the Licenses dialog (`public/licenses.json` plus `scripts/generate-licenses.mjs`) lists both DLLs, their versions, hashes, source URLs and texts.
  2. State version, origin URL, archive hash, and build date for the exact `libmpv-2.dll` and `libmpv-wrapper.dll` in use. Pin the 7z and zip hashes in the repo. Record them in `docs/ENGINEERING-NOTES.md` at integration time.
  3. Provide or link the corresponding source plus build scripts for that exact libmpv build (mpv commit, winbuild scripts, FFmpeg config). A link to the release plus commit hash satisfies LGPL only if it stays valid for at least three years. Prefer vendoring a source offer page in the repo docs with permanent URLs.
  4. Allow reverse engineering and relinking for debugging the LGPL parts. Do not block DLL replacement with signature checks that refuse to start.
  5. Keep proprietary Kaminari code in separate files. No mpv headers pasted into MIT files without preserving their license headers.
* Open verification before ship: the FFmpeg commit `cc583787b` is verified from the Build MPV (64, true) log, the zhongfly patch removes `--enable-gpl` and the GPL-only packages, and the enabled features contain no GPL-only VO. The native license texts and source offer are in `licenses/native/`, and the draft legal review is in `docs/LEGAL-REVIEW-DRAFT.md`. Remaining: counsel review for public distribution and the VM installer checks. If any check fails, stop. Do not ship a bundled binary on hope.
* Signing and updates do not change the license duty. A signed NSIS installer that bundles LGPL DLLs still owes source offer and license texts. The updater `latest.json` notes should name the native library versions.

Bottom line: MPL-2.0 plugin verified locally, LGPL-2.1 wrapper verified from the pinned release zip, the chosen `libmpv-2.dll` is the `mpv-dev-lgpl` artifact, and the FFmpeg commit is verified from the build log. MIT plus dynamically linked LGPL libmpv is shippable with attribution, license texts, and a source offer. The remaining gate is counsel review for public distribution and the VM installer checks. GPL libmpv is not shippable under the current posture.

## 4. Architecture

### Rust player module

Keep `src-tauri/src/player.rs` for the external fallback. Add a new module, for example `src-tauri/src/mpv_embed.rs`, for the embedded path. It wraps the plugin or owns init state:

* `MpvEmbedState`: current window label, init flag, last config hash, last error, DLL presence check (`libmpv-wrapper.dll` plus `libmpv-2.dll` in exe dir or `src-tauri/lib` during dev).
* New commands: `mpv_embed_init(window_label, config)`, `mpv_embed_load(window_label, url, headers, start_sec, subtitles)`, `mpv_embed_stop(window_label)`, `mpv_embed_status() -> { available, dlls_present, active_label, last_error }`.
* Prefer thin Rust wrappers that call the plugin commands `init`, `command`, `set_property`, `get_property`, `destroy`. Do not reimplement header sanitizing. Reuse the sanitizer from `player.rs` `build_args` so proxy and mpv paths share one rule.
* Header passing: libmpv accepts `http-header-fields` as an option or `loadfile` param. Reuse the same allowlist as the proxy (`User-Agent`, `Referer`, `Origin`, `Cookie` seed, `Authorization` same origin only, `Accept`, `Accept-Language`). Never pass full extension header maps blindly. Strip CR, LF, NUL. Cap count and bytes.
* Proxy integration: when the stream needs headers and the entry exists, pass the loopback URL (`http://127.0.0.1:<port>/s/<id>/...?token=...`) to `loadfile`, not the upstream URL. libmpv then needs no custom headers for MP4 and HLS. For public MP4 with no headers, pass the upstream URL directly. For header protected HLS without proxy, pass `http-header-fields` plus the master URL and let libmpv fetch segments. Prefer proxy plus libmpv for cookie gated sources because the Rust jar already handles `Set-Cookie`.
* Events: subscribe to the plugin `mpv-event-<label>` in Svelte with `listen`, translate to `kaminari/v1/player_event` shaped payloads (`time-pos`, `duration`, `pause`, `eof`, `hwdec-current`, `video-format`, `audio-params`). Rust keeps a ring of last events for diagnostics. Never log URLs with query, tokens, cookies, or `Authorization`.
* Lifecycle: init once per watch route, `loadfile` per episode, `destroy` on route leave and on window close. The plugin already hooks `WindowEvent::CloseRequested` to destroy before close. Kaminari must also call destroy on player unmount to avoid leaked handles across episodes.

Config baseline for init:

```
vo: gpu-next
hwdec: auto-safe
keep-open: yes
idle: yes
force-window: yes
wid: <set by plugin>
input-default-bindings: no
input-vo-keyboard: no
osc: no
osd-level: 0
sub-auto: fuzzy
subs-fallback: yes
```

Adjust after measurement on Intel, AMD, and NVIDIA.

### Svelte controls

New `src/lib/MpvView.svelte` beside `PlayerView.svelte`. Same props (`stream`, `title`, `subtitle`, `startSec`, callbacks). Same glass bar, same shortcuts from the spec: Space or K, J and L, arrows, Shift plus arrows, Up and Down volume, M, F, T, V, comma and period, 0, N, D, Esc, single click pause, double click fullscreen. Same four states: loading, empty, error, offline. No native mpv OSD. No `<video>` element when embedded.

Progress: observe `time-pos` and `duration` at 2 to 4 Hz, throttle writes to `sourceProgressSet` every 5 seconds plus on pause, close, and EOF, same as today. Time display stays mono. Scrub sends `seek <sec> absolute`. Speed sends `set speed`. Volume sends `set volume` plus `mute`. Subtitle cycle sends `cycle sub`. Track pick sends `set sid` and `set aid`.

Up Next: on `end-file` with reason `eof`, show the same Ended panel with a 10 second countdown and cancel. Next episode loads with `loadfile <url> replace`.

Diagnostics panel adds: `Backend: libmpv`, `DLLs: ok or missing`, `VO: gpu-next`, `HWDec: <current>`, `Video: <codec> <size> <fps>`, `Audio: <codec> <channels>`, `Subs: <current>`, `Proxied: yes or no`. Copy still redacts tokens and full query strings.

### Native surface, overlay z-order, input, fullscreen, DPI, multi monitor

* Transparency: plugin README requires `transparent: true` on the Tauri window plus transparent page background. Kaminari `tauri.conf.json` has no `transparent` today and the frameless shell paints opaque void colors. P0 must set `transparent: true` on the watch route window or the main window, scope the transparent CSS to the player route only, and verify title bar, nav, and dialogs still paint solid elsewhere. Risk: full app transparency hurts text contrast and costs GPU. Prefer a dedicated watch window if Tauri supports per window transparency cleanly, else gate the CSS class on `body[data-player-open]`.
* Z-order: `wid` renders mpv video into the native window behind the WebView. HTML controls composite above it only if the page background over the video rect is transparent and the control layers are positioned. The glass bar, top bar, buffering spinner, error panel, and diagnostics must set solid or glass backgrounds explicitly. Any full cover opaque div hides video. Audit every `absolute inset-0` layer in the new view.
* Input: disable mpv key handling (`input-default-bindings: no`, `input-vo-keyboard: no`) so Svelte owns all shortcuts. Mouse clicks go to the Svelte overlay, not mpv. Do not enable mpv `osc`.
* Fullscreen: use the Tauri window fullscreen for F and double click, not mpv exclusive fullscreen, so controls and focus trap stay alive. Set `video-margin-ratio-bottom` when the bar is visible if letterboxing is preferred, else keep overlay over video per spec. Test both. Spec shows overlay bar, so overlay wins unless readability fails on bright HDR frames.
* DPI and multi monitor: `gpu-next` plus WebView both scale, but the `wid` child can lag on monitor moves or scale changes. Test 100, 125, 150, 200 percent, move the window across monitors mid play, disconnect a monitor, sleep and resume. Acceptance: no black video after move, no stuck 1x backing size, controls stay 44px targets. Log `current-monitor` scale in diagnostics during QA.

## 5. Windows packaging

* Files: `src-tauri/lib/libmpv-wrapper.dll` plus `src-tauri/lib/libmpv-2.dll` (x86_64 first, aarch64 later). Dev run needs the same two files next to the debug exe or in `src-tauri/lib`. `pnpm check:libmpv` fails the bundle if either is missing or does not match the pinned hash.
* `tauri.conf.json`: `bundle.resources` is `["lib/**/*"]`, `app.windows[].transparent` is `true` for the player surface (see architecture note), and `targets` is `nsis`. Do not bundle `mpv.exe`. The fallback still uses system mpv, it is not bundled.
* Installer size: budget in `docs/DESIGN-SPEC.md` says installer under 15 MB before libmpv resources, CI fails past it. libmpv plus wrapper plus deps add roughly 25 to 45 MB compressed based on zhongfly artifact sizes (measure at integration, do not copy this as fact). Record exe size, installer size, and cold start in `docs/ENGINEERING-NOTES.md`. Expect SmartScreen warnings on unsigned builds. Document them per `docs/ARCHITECTURE.md`. Sign the installer with the same cert as the exe when signing exists.
* Updates: updater ships full installer, not DLL patches. `latest.json` notes must list libmpv and wrapper versions. Rollback is reinstall prior exe, library SQLite file stays intact. If a new DLL breaks playback, the app must detect init failure and offer WebView plus external mpv without requiring an update.
* Fallback chain at runtime: check DLL presence, call `init`, on failure show reason in diagnostics and mount `PlayerView` (WebView plus proxy) with a toast, keep Open in mpv button. Never leave a black route with no action.
* macOS notes: install system libmpv via `brew install mpv`, bundle `libmpv-wrapper.dylib`, sign and notarize both, test NSView embedding on Apple Silicon and Intel. Plugin status is untested, so macOS is P3 at earliest.
* Linux notes: depend on system `libmpv-dev` or bundle matching `.so`, bundle `libmpv-wrapper.so`. Window embedding is broken per plugin README. Wayland has no `wid` path. Treat Linux embedded as research until the plugin or a direct render context path proves X11 embedding. Ship external mpv there.

## 6. Playback features and scope

P0 (must have for embedded to replace WebView as default on Windows):

* H.264 MP4 progressive, with and without proxy loopback URLs.
* HLS single and master variants through proxy or direct with headers. Variant switch and audio group pick.
* HEVC 8 and 10 bit MP4 and HLS segments with `hwdec auto-safe` plus software fallback.
* ASS and SSA subtitles with libass: position, override, fonts, embedded attachments. External ASS and SRT over proxy.
* Resume at saved position, progress every 5 seconds plus pause and close, 90 percent marks watched, same tables as today.
* Track selection for audio and subtitles, speed 0.5 to 2, volume plus mute, fullscreen, diagnostics with redaction.
* Error panel with Retry, Try next source, Copy diagnostics. Ended panel with Up Next countdown.

Later (explicit non goals for P0):

* P1: AV1 10 bit HDR tuning, HDR tone mapping options (`tone-mapping`, `target-colorspace`), HDR to SDR fallback flag, screenshot to user folder, subtitle delay and sync keys, audio delay.
* P2: DASH, chapter list, ordered chapters, multi file episodes, picture in picture, per extension font config, HDR metadata readout in diagnostics.
* Never: DRM Widevine, torrent transport in player, downloads in player process, background audio service. Those stay out per proxy plan non goals.

## 7. Security

Threat posture matches `docs/PLAN-STREAM-PROXY.md` section 2. Extensions are untrusted. WebView may hold attacker strings. The proxy stays the only fetcher for protected origins.

* Untrusted URLs and headers: validate `loadfile` URL in Rust before passing to libmpv. Allow http and https only. Reuse proxy registration so the extension manifest allowlist, DNS pin, and SSRF blocks apply. Reject `file:`, `data:`, `blob:`, and loopback other than the app proxy. Sanitize header names and values (no CR, LF, NUL), enforce the same allowlist and caps as the proxy.
* Extension allowlist: every child URL libmpv fetches directly (HLS segments, keys, external subs when proxy is off) must pass the same host check as proxy children. Prefer proxy URLs so checks happen once in Rust. Log blocked hosts without full URLs.
* Cookie jar: jars stay per entry in Rust memory, never in JS. When libmpv loads a proxy URL, the jar attaches server side. Never copy `Set-Cookie` values into Svelte state or diagnostics. Revoke on close, episode switch, source disable, and app exit.
* Proxy integration: register before `loadfile`, revoke after progress flush. Copy buttons use upstream URL, never loopback. Diagnostics print host and entry prefix only. Token never crosses into mpv command line logs.
* Log leakage of the proxy token through libmpv: the loopback URL passed to `loadfile` contains `?token=...`, and mpv logs the filename on load while the wrapper echoes command args and property values in trace output and FFI error strings. Terminal output or a log file would persist the token. Mitigation: set `terminal=no`, keep the mpv log level at warn or higher, never set `log-file`, keep verbose off in release, and wrap wrapper errors in Rust to redact the query string before logging or emitting, using the same `redact_url` rule as the proxy. Revoke the entry on close so a leaked token expires with the entry TTL.
* No shell or arbitrary args: Svelte sends only typed intents (`load`, `seek`, `speed`, `volume`, `sub-track`, `audio-track`). Rust maps intents to a fixed allowlist of mpv commands and properties. Reject unknown `command` names and unknown `set_property` keys. Never interpolate extension strings into a shell. `player.rs` external path already avoids shell by using `Command` args; keep that. libmpv path must not accept raw `command` strings from the WebView.
* Window and fullscreen: no extra capabilities. Keep Tauri capability allowlist to the new `mpv_embed_*` commands. Asset scope unchanged.

## 8. Risks S, H, M, L

Scale: S means systemic, affects all installs or the machine. H means serious but bounded to one user session. M means confined to one playback session. L means cosmetic or noisy. Likelihood H, M, L for this deployment.

| # | Risk | S | Likelihood | Impact | Mitigation and fallback |
|---|---|---|---|---|---|
| 1 | License breach by bundling GPL libmpv or missing source offer | S | M | Forced source release of combined work, store or host takedown, rework | Ship only `mpv-dev-lgpl` artifact, pin hashes, list the Native group in the Licenses dialog, publish the source offer, and get counsel review before public distribution. Local P1 packaging is enabled. |
| 2 | Plugin abandonment or break on Tauri and WebView2 updates | S | M | Embedded path rots, every release needs a fork | Pin plugin plus wrapper versions, wrap all calls behind `mpv_embed.rs` intents, keep WebView and external mpv green in CI, budget a direct FFI fork path. |
| 3 | Transparent window plus `wid` compositing fails on some GPUs | H | M | Black video, controls over void, readability loss | Dedicated watch surface, GPU allowlist logging, auto fallback to WebView when first frame times out, diagnostics with `vo` and `hwdec-current`. |
| 4 | Linux Wayland and macOS embedding gaps | H | H on those OSes | No video on Linux Wayland, untested macOS | Windows first, Linux and macOS stay on WebView plus external mpv until proven. Explicit error, not silent black. |
| 5 | Untrusted headers or URLs reach mpv directly | H | L with checks, M without | SSRF from player, credential leak, local file read | Fixed command allowlist, URL scheme check, reuse proxy allowlist, no raw command passthrough. Fuzz intent mapping. |
| 6 | DLL size, signing, and update regressions | M | M | Installer doubles, SmartScreen blocks, update breaks DLL path | Measure and note sizes, sign both DLLs, test clean install plus update plus rollback, `mpv_embed_status` gate on boot. |
| 7 | Progress loss on fast episode switch or crash | M | M | Resume wrong, watched missed | Flush progress before `loadfile replace` and before revoke, order is report, await save, then revoke. Same as proxy plan section 10. |
| 8 | HDR looks worse than WebView SDR on cheap displays | M | M | Washed or clipped color, user complaints | Default `tone-mapping` on, SDR fallback toggle in Settings, log display caps in QA, keep WebView choice per title. |
| 9 | ASS fonts missing or fallback wrong | M | M | Boxes or wrong typeface | Bundle one open font, set `sub-fonts-dir`, test karaoke and overlap, fall back to WebVTT row when attachment fails. |
| 10 | Event flood from `time-pos` at 60 Hz | L | M | IPC churn, UI jank | Observe at low rate, throttle Svelte writes, poll `time-pos` on demand instead of observing every frame. |
| 11 | Proxy token leaks via libmpv logs or wrapper errors | M | M | `loadfile` URL carries `?token=...` and mpv echoes filenames while the wrapper echoes args in trace and error strings, so a terminal dump or log file replays one stream until TTL expiry | Set `terminal=no`, log level warn or higher, no `log-file`, redact query in Rust error mapping, revoke on close. Rated M not H because the token is per entry, loopback only, and TTL bounded. |

Failure modes and fallbacks:

* DLLs missing: `mpv_embed_status.available=false`, mount WebView, toast once, keep Open in mpv.
* `init` fails: show code plus `Copy diagnostics`, mount WebView, log `wid` error without window internals.
* First frame timeout (no `time-pos` or `vo` event in 3 s): auto retry once with `hwdec no`, then fall back to WebView.
* `loadfile` 403 or 410 on proxy URL: re register once from cached `JsStream`, then require manual retry.
* Upstream 502 or timeout: same copy as proxy plan (refused, timed out), offer mpv button.
* App exit with active handle: plugin close hook plus explicit `destroy` on unmount, kill external child in `PlayerState::stop`.

## 9. Test matrix and phased implementation P0 to P3

No network in Rust unit tests. Fixtures serve on `127.0.0.1` with ephemeral ports. Manual QA uses the public domain sample plus header protected fixtures only.

Test matrix (manual QA per phase):

| Width | Theme | Source | Checks |
|---|---|---|---|
| 1024 | Dark, Light | Sample MP4 direct in libmpv | First frame under 1500 ms p95 on cached source, seek, pause resume, close saves Resume |
| 1280 | Dark, Light | Header MP4 via proxy into libmpv | Plays, seek works, no token in logs, mpv fallback intact |
| 1600 | Dark, Light | HLS media plus keys via proxy | Segments load, key rotation passes, subtitle track shows |
| 1920 | Dark, Light | HEVC 10 bit plus ASS | HWDec status sane, ASS styled, no boxes, controls legible over bright frames |
| 2560 | Dark, Light | HDR sample plus master HLS | Tone mapping sane, variant switch works, Up Next countdown cancels |
| Multi | Dark | Monitor move, scale 125 and 150 | No black video after move, targets stay 44px, fullscreen returns focus |

P0: spike and decision (effort 3 to 5 days). Acceptance: transparent route proves video shows through under glass on the dev machine, `loadfile` plays sample MP4 and a local HEVC file, Svelte shortcuts work with mpv input disabled, `destroy` cleans handles, fallback to WebView works when DLLs are removed. Z-order gate: in windowed and fullscreen, the glass controls, buffering spinner, error panel, and diagnostics must be visible and readable over the video, and the video must not be hidden by an opaque WebView layer. If that gate fails, stop and choose a dedicated watch window or WebView fallback before P1. Records: first frame time, CPU and GPU during HEVC, installer size delta for the two DLLs. No license change yet, no bundle change.

### P0 result, 2026-11-08

Machine: Windows 11 Pro build 26300, i7-12700K, Radeon RX 6600 driver 32.0.21045.16010, WebView2 157.0.4341.42. Native artifact is the pinned `mpv-dev-lgpl` build from section 10.

Dev entry: set `localStorage["kaminari.libmpv"]="true"` or add `?libmpv=1` to the app URL. The harness is `?mpvHarness=1&sample=popeye` or `&sample=short`, and `&file=<absolute path>` for a local sample. Copy both DLLs next to the debug exe, or into its `lib` folder, before the run.

* Z-order gate: pass. In a 1280x800 window and at 2560x1440 fullscreen, the video stayed visible under the glass top bar, bottom bar, diagnostics, and the error panel. `html` must be transparent as well as `body` and `#app`; setting only the app root leaves a black page over the video. Fullscreen uses the Tauri window, not mpv exclusive fullscreen.
* First frame: 1.82 s and 1.94 s for the archive.org Popeye MP4, 0.59 s for the 10 s Big Buck Bunny MP4, and 0.23 s for the local 1080p 10 bit HEVC file. Measured from `loadfile` to the first `time-pos` above zero.
* HEVC: a local libx265 1920x1080 30 fps 10 bit sample played with `video-codec=hevc` and `hwdec-current=d3d11va`. Over 9.5 s: CPU 0.5 percent of the machine (9.1 percent of one core), GPU video decode 3.5 percent average and 10.6 percent peak, GPU 3D 1.4 percent average. HEVC was tested; no external HEVC file entered the repo.
* Fallback: with both DLLs renamed, the SourceBrowser mounted PlayerView and the harness showed the toast `libmpv is unavailable: DLLs are missing. Using the WebView player.` The WebView video played, with no black route.
* Lifecycle: two open and close cycles left `activeLabel` null and the mpv child window count at 0. A real SourceBrowser close did the same. A hard reload destroys the old handle before init. The wrapper also reports an `end-file` reason `quit` during teardown, so the new view ignores it.
* Native payload: `libmpv-2.dll` is 101,853,696 bytes and `libmpv-wrapper.dll` is 391,680 bytes, uncompressed. The bundled installer measurement is recorded in `docs/ENGINEERING-NOTES.md`.
* Blockers for P1: the wrapper rejects JSON arrays, so `http-header-fields` goes as one comma separated string and header values with commas are unsafe. Header protected MP4 and HLS must use the proxy until the wrapper supports lists. The mpv `start` property is rejected at runtime, so resume seeks after `file-loaded`. The native license texts and source offer are in `licenses/native/`, the FFmpeg commit is verified from the build log, and counsel review is the remaining gate for public distribution.
* P1 needs: a dedicated watch window or proven route scoped transparency on the main window, installer size measurement, the header path through the proxy, counsel review, and clean install plus update tests.

P1: Windows packaging and hardening (effort 1 to 2 weeks). Local packaging is enabled. Public distribution waits on section 3 counsel review. Pre-P1 evidence lives in `licenses/native/README.md` (texts, hashes, source offer), `docs/LEGAL-REVIEW-DRAFT.md` (review packet), and `docs/LIBMPV-PRE-P1.md` (header rule, transparency decision, installer checklist). Acceptance: pinned `mpv-dev-lgpl` plus wrapper hashes, `bundle.resources` set, clean install on a Windows 11 VM with WebView2 present plays HEVC plus ASS, signed installer notes SmartScreen behavior, Licenses dialog lists native DLLs with versions and texts, `mpv_embed_status` gates playback, all proxy tests still green. Records: exe size, installer size, cold start, time to first frame.

### P1 packaging result, 2026-11-09

The 0.19.0 NSIS installer is 37,646,594 bytes (35.90 MiB). The release exe is 20,895,232 bytes. The native payload is 102,245,376 bytes uncompressed. It installed per-user to `%LOCALAPPDATA%\Kaminari`, the two DLLs matched the pinned hashes in `lib`, `mpv_embed_status` reported available true with the install lib path, and Popeye episode 1 played through the installed libmpv at 0:14 of 7:36 with activeLabel main and no fallback. Silent uninstall removed the install directory and DLLs and left the SQLite data. The 0.20.0 installer is 37,659,877 bytes, the 0.21.0 installer is 37,745,278 bytes, and the 0.22.0 installer is 37,878,262 bytes, all with the same native payload. The VM clean install, update, rollback, signing and SmartScreen checks remain.

P2: feature parity with `PlayerView` (effort 2 to 3 weeks). Acceptance: HLS master plus audio groups, track pickers, speed, volume, resume, progress, Up Next, diagnostics redaction, error and ended panels match spec, keyboard map complete, focus trap and focus return work, reduced motion stops zoom and shimmer over video. Tests: intent allowlist unit tests, URL scheme tests, event mapping tests, Vitest for control logic, no token in copied diagnostics.

P3: polish and ports (effort 3 to 4 weeks, macOS and Linux may split). Acceptance: HDR tone mapping defaults plus SDR toggle, screenshot command, subtitle and audio delay, HDR readout in diagnostics, macOS smoke on Apple Silicon, Linux X11 smoke, Wayland shows a clear unsupported message with mpv button, docs updated (`ENGINEERING-NOTES.md` numbers, `CHANGELOG.md` entry, `ARCHITECTURE.md` player section if the design changes).

Each phase updates this plan with measured numbers before moving on. No phase widens CSP beyond the 8 loopback ports. No phase ships a bundled binary before the license gate passes.

## 10. Decisions

Approved on 2026-11-08. These replace the open questions.

1. Platform order: Windows first. Linux and macOS stay on WebView plus external mpv until embedding proves out there.
2. Native artifact: use the `zhongfly/mpv-winbuild` `mpv-dev-lgpl` x86_64 build, pinned by archive hash. A qualified lawyer reviews the package before public distribution. Local packaging and playback checks are enabled. Never ship the GPL `mpv-dev` artifact.
3. Installer budget: keep the 15 MB core budget and add a separate native payload budget for the two DLLs.
4. Transparency scope: only the watch route goes transparent. The rest of the window stays opaque.
5. HDR default: tone-mapped SDR by default.
6. ASS fonts: bundle one open fallback font at P1.
7. Native updates: `libmpv-2.dll` and `libmpv-wrapper.dll` update with the app, not on a separate channel.
8. Fallback telemetry: local counters only for embedded success, WebView fallback, and external opens. No upload.

### P1 native artifacts, bundled from a gitignored payload

The DLLs live in `src-tauri/lib/`, which is gitignored. The plugin searches only the exe directory and exe/lib, so before running the harness copy them into the debug target with:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/prepare-libmpv-dev.ps1
```

Re-run that after `cargo clean` or a fresh target directory. `bundle.resources` includes `lib/**/*` and `pnpm check:libmpv` verifies the hashes before packaging.

| Component | Version | URL | Archive SHA-256 | DLL SHA-256 | Size |
|---|---|---|---|---|---|
| libmpv-wrapper | v0.1.1, `libmpv-wrapper-windows-x86_64.zip` | https://github.com/nini22P/libmpv-wrapper/releases/download/v0.1.1/libmpv-wrapper-windows-x86_64.zip | `D2FF8B2EDCD34D2968E544ADAA915E5E5C48EB1A0995945005269C2AF119A492` | `0D5ADEAD5F175C55E0790A80924EC0A2636F72E3675C79A6D9D9568B2ED2384A` | 391,680 bytes |
| libmpv | 2026-11-08, mpv git `8bc468896e`, `mpv-dev-lgpl-x86_64-20261108-git-8bc468896e.7z` | https://github.com/zhongfly/mpv-winbuild/releases/download/2026-11-08-8bc468896e/mpv-dev-lgpl-x86_64-20261108-git-8bc468896e.7z | `84BD684C848591356B55555001C114FC72AF9EDCE2D4EBB8B38CC7356A3C0BF5` | `2A287ECB1FE2BC5B6AB636CFEC0CACBB12FFEC05E4EB4E5C2CADE1752E2D15E6` | 101,853,696 bytes |

The wrapper zip publishes a matching `sha256.txt`, and its local `LICENSE` is LGPL-2.1. The mpv-dev-lgpl archive contains `libmpv-2.dll`, the import library, and headers, but no license or copyright file. P1 added the license texts and a source offer from the mpv and FFmpeg projects. `mpv-dev-lgpl` is still marked use at own risk by its author, so counsel review remains the gate for public distribution.
