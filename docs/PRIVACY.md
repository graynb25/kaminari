# Privacy Policy

Last updated: 2026-11-11

Kaminari is local-first. The app has no account system, no analytics, no advertising, no telemetry, and no crash upload by default. This policy explains what stays on your machine and what leaves it when you use the app.

## 1. Who we are

Kaminari is an open-source desktop project. Contact: [[CONTACT_EMAIL]].

## 2. Data we collect

We do not collect personal data on a server we operate. The app does not include an analytics SDK, an advertising SDK, a crash reporter, or a sign-in system. We do not sell, rent, or share personal data with advertisers or data brokers.

If you contact us by email or through the project repository, we receive what you send, such as your address, message and any attachments. We use it to answer you and keep a record of the request.

## 3. Data stored on your device

Kaminari stores the following locally:

- **SQLite database:** library entries, watched episodes, playback progress, history, source library entries, installed extension metadata and extension storage. Extension storage values are encrypted before they are written; the rest of the database is plaintext.
- **WebView local storage:** your theme choice, the Incognito flag and recent searches. While Incognito is on, new searches stay in memory for the open window and are not written to local storage.
- **WebView cache:** images and media fragments cached by the embedded browser.
- **WebView cookies:** cookies set by sites you access through the app or its extensions, if any.

On Windows, the main database and app data live under `%APPDATA%\app.kaminari.desktop`. The embedded WebView data lives under `%LOCALAPPDATA%\app.kaminari.desktop`. On macOS and Linux, the equivalent app data and WebView folders are used.

### 3.1 Data at rest

Kaminari does not encrypt the whole SQLite database. Library entries, watched marks, playback progress, history, source library rows, source progress, source history and installed extension code remain plaintext at rest. They are protected by your Windows account and by Windows disk encryption such as BitLocker or Device Encryption. Use full-disk encryption if you need the database protected against someone who can read the disk or copy the files.

Extension storage values (the `js_storage.value` column used by `storage.get` and `storage.set`) are encrypted with Windows Data Protection API (DPAPI) in the current-user scope before they are written. The extension API still sees plaintext: reads decrypt and writes encrypt. Existing plaintext rows are migrated when the database opens or when an extension first reads the value. Values that cannot be decrypted are treated as missing instead of being returned as ciphertext or crashing the app. Storage key names, timestamps and the rest of the row remain plaintext.

DPAPI binds encrypted extension values to the Windows user and device. A backup keeps those values encrypted, so a backup restored under a different Windows account or on a different device may have extension values that cannot be read. Everything else in a backup is plaintext. A Kaminari build without DPAPI support reads the stored prefix as an ordinary value, so downgrading can make extension storage appear missing. Keep a backup before downgrading.

Full-disk encryption remains the protection for the database, the WebView profile and anything else Kaminari stores. Extension storage encryption is an additional layer for the values most likely to contain tokens or source credentials, not a replacement for disk encryption.

## 4. Network requests

Kaminari makes network requests directly from your device. We do not proxy or receive them.

- **AniList:** `https://graphql.anilist.co` for search, metadata, cover images and banners. AniList receives your IP address, user agent, query and request time under its own privacy policy.
- **Artwork hosts:** cover and banner images are fetched from the hosts AniList or your extensions return.
- **App updates:** when you open Settings, the app may check the signed release feed at `https://github.com/graynb25/kaminari/releases/latest/download/latest.json`. GitHub receives your IP address, user agent and request time under its own privacy policy. An update is downloaded only after you confirm it.
- **Registry URLs:** Kaminari does not expose a registry browser. The legacy registry command remains in the backend for a possible future APK runtime; if it is called, the app fetches the URL you choose and that host receives your request.
- **Extension hosts:** extensions you install can call `net.fetch` against hosts listed in their manifest. Those hosts receive your request and any data the extension sends.
- **Stream hosts:** when you play a video, the player fetches the stream and segments from the host the extension resolved. That host receives your request and may set cookies.
- **External mpv:** if you choose Open in mpv, mpv is a separate program with its own behavior and data handling.

These third parties may log IP addresses, timestamps, headers, cookies and requested URLs under their own policies. We do not control them.

## 5. Cookies and tracking

Kaminari does not use advertising or analytics cookies and does not track you across apps or websites. The embedded WebView may store cookies set by sites you access, including source sites and stream hosts. You can clear that data from Settings > Privacy > Clear WebView data, or by deleting the app's WebView data folder.

### 5.1 Incognito mode and its limits

Incognito is a playback-privacy mode, not a network anonymity tool. While it is on:

- Playback is not written to history, progress, Continue Watching or AniList, and the app pauses its automatic background AniList sync. An explicit "Sync now" in Settings still runs if you press it.
- New recent searches stay in memory for the open window and are not written to WebView local storage.
- Downloads you start are still written to disk, and AniList, extension, artwork, stream and update requests still leave your device with your IP address.
- The WebView profile is not changed. Cover images, media fragments and cookies can still be cached until you clear WebView data or delete the WebView data folder.
- The queue created before Incognito was turned on is kept, not deleted. It resumes syncing when Incognito is turned off.

WebView data is cleared asynchronously. Restart Kaminari after clearing it to start from an empty cache and the cleared local settings.

## 6. Sharing and sale

We do not sell or rent personal data. We do not share data with advertisers. The third-party requests described above are made by your app directly, not by us.

## 7. Retention and deletion

Local data stays on your device until you delete it. On Windows, Settings > Privacy > Data at rest has a **Clear all local data** action. It deletes the SQLite database and its WAL/SHM files, Kaminari's safety and quarantine copies, a local settings file if one exists, the WebView cache, cookies and local storage, and then asks you to restart. Downloaded videos, exported backups, and the AniList token in Windows Credential Manager are not deleted. If a file is still open, Kaminari finishes removing it on the next start.

You can also remove the data manually:

1. Close Kaminari.
2. Delete `%APPDATA%\app.kaminari.desktop` to remove the database and app data.
3. Delete `%LOCALAPPDATA%\app.kaminari.desktop` to remove the WebView cache, local storage and cookies.

On macOS and Linux, delete the equivalent app data and WebView folders. Uninstalling the app may not remove these folders. Back up anything you want to keep first. Deletion cannot be undone.

We do not hold a copy of your local data.

## 8. Children

Kaminari is not directed to children under 13. Do not use it below the age of digital consent in your jurisdiction without a parent or guardian. We do not knowingly collect data from children.

## 9. Security

Kaminari limits extension capabilities and checks extension hosts against a manifest allowlist, but no system is perfectly secure. Third-party extensions can be malicious or broken. Install only extensions you trust. Keep your operating system and the app updated.

## 10. International users

Kaminari runs locally. If you access AniList or another third-party service, that service may process your data in other countries under its own policy. We do not transfer data ourselves.

## 11. Changes

We may update this policy. The updated version will be posted with the app or in the project repository. Continued use after a change means acceptance. If you do not agree, stop using the app and delete your local data.

## 12. Contact

Privacy questions: [[CONTACT_EMAIL]].

Project repository: https://github.com/graynb25/kaminari.

## Note

This is a project policy, not legal advice. Have a qualified lawyer review it before public distribution, especially if you add accounts, telemetry, a public registry, or paid features.
