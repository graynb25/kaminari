# Install Kaminari

Kaminari is a Windows desktop app. It ships no anime catalog, no source list, and no APKs. AniList provides discovery and metadata. Playback comes from a source you install and trust.

## Requirements

- Windows 10 or Windows 11, 64 bit.
- Microsoft Edge WebView2 Runtime. Windows 10 and 11 normally include it.
- About 120 MB of disk space for the app and the optional libmpv files.
- Internet access for AniList, updates, and your sources.

## Installer

1. Download `Kaminari_<version>_x64-setup.exe` from the Releases page.
2. Run it. The installer is per-user, so it does not need administrator rights.
3. Windows SmartScreen may warn because the installer is not code-signed. Check that you downloaded it from the official release, then choose More info and Run anyway if you trust it.
4. Launch Kaminari from the Start Menu.

The app installs under `%LOCALAPPDATA%\Kaminari`. Your library is separate, under `%APPDATA%\app.kaminari.desktop`.

## Portable zip

1. Download `Kaminari_<version>_x64-portable.zip`.
2. Extract the whole folder. Keep `Kaminari.exe` and the `lib` folder together.
3. Run `Kaminari.exe`.

The portable build is installer-free, not data-free. Settings and the SQLite library still live in `%APPDATA%\app.kaminari.desktop`, so replacing the folder with a newer portable build keeps your data.

## First run

Browse works without an account. AniList account connection is optional and lives in Settings. The app starts with the public-domain sample extension available but not installed, and no anime source.

A Getting started checklist appears on first launch: add a source, find a title, then watch. Choose Skip for now to dismiss it. Reopen it any time from Settings, About, Getting started.

There is no Aniyomi registry browser. Those packages are Android APKs and cannot run here, so the page shows only native extensions and Mangayomi sources.

The sample is a test source, not a catalog. Install it from Sources if you want to verify the QuickJS host, then run Test search with `Popeye`.

## Add a source

Kaminari does not include or name a source. Add a raw Mangayomi JavaScript file from a provider you trust.

1. Open Sources.
2. Scroll to Mangayomi sources.
3. Paste the raw `.js` URL.
4. Select Inspect source.
5. Read the name, version, language, base URL and SHA-256. The trust panel explains that the code can contact any host and send it what it reads.
6. Select Install only if you trust the source.
7. Open Browse on the installed source card, search a title, open a show, and choose Watch or Play.

A Mangayomi source is a JavaScript file, not an APK. Aniyomi APK extensions are Android bytecode and cannot run in Kaminari. Do not install `.apk` files.

The source file itself decides which sites it talks to. Kaminari does not review, endorse, host, or update it. If the source site changes or blocks traffic, the source may stop working until its author updates it.

## Source settings

Some sources expose settings. Open the source card menu and choose Settings. Common options include stream server, preferred audio, and episode thumbnails. Changes apply the next time the source loads a show.

## Watching

- Watch on an AniList title searches your installed sources and opens the closest match.
- Preferred quality is in Settings and on the show page.
- Autoplay next episode is in Settings. It shows a five second countdown and a Play next button.
- Remote subtitles are proxied with the stream headers. Inline subtitles become WebView tracks.
- Downloads keep a file from a stream when you have the right to keep it. Choose a downloads folder in Settings.

## Updates

The app checks the signed release feed from Settings. A build with no public feed shows a local-build warning instead of failing.

## Uninstall

Use Windows Settings, Apps, Installed apps, Kaminari, Uninstall. The uninstaller removes the app. It does not delete your library. Remove `%APPDATA%\app.kaminari.desktop` yourself if you want a clean start.

## Troubleshooting

- **No anime sources:** install a Mangayomi JavaScript source. The sample is only a test.
- **QuickJS seems broken:** install the sample and run Test search with `Popeye`. If that works, the host is fine and the problem is the third-party source or its site.
- **A source returns nothing:** the site may have changed, be geo-blocked, or use a challenge Kaminari does not solve. Try another source.
- **Playback stops:** use 0.26.2 or later, try another quality or server in Streams, or open the stream in mpv.
- **Portable build will not start:** keep the `lib` folder next to `Kaminari.exe`, and install the WebView2 Runtime.
- **Update check warns:** the release feed is not publicly reachable. This is expected for a private or local build.

## Rights and privacy

Kaminari ships no content and no source list. You are responsible for the sources you install and the content you access. Download only what you have the right to keep. The app stores your library locally in SQLite and sends no telemetry.
