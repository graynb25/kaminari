# Privacy Policy

Last updated: 2026-11-08

Kaminari is local-first. The app has no account system, no analytics, no advertising, no telemetry, and no crash upload by default. This policy explains what stays on your machine and what leaves it when you use the app.

## 1. Who we are

Kaminari is an open-source desktop project. Contact: [[CONTACT_EMAIL]].

## 2. Data we collect

We do not collect personal data on a server we operate. The app does not include an analytics SDK, an advertising SDK, a crash reporter, or a sign-in system. We do not sell, rent, or share personal data with advertisers or data brokers.

If you contact us by email or through the project repository, we receive what you send, such as your address, message and any attachments. We use it to answer you and keep a record of the request.

## 3. Data stored on your device

Kaminari stores the following locally:

- **SQLite database:** library entries, watched episodes, playback progress, history, source library entries, installed extension metadata and extension storage.
- **WebView local storage:** your theme choice and recent searches.
- **WebView cache:** images and media fragments cached by the embedded browser.
- **WebView cookies:** cookies set by sites you access through the app or its extensions, if any.

On Windows, the main database and app data live under `%APPDATA%\app.kaminari.desktop`. The embedded WebView data lives under `%LOCALAPPDATA%\app.kaminari.desktop`. On macOS and Linux, the equivalent app data and WebView folders are used.

Local data is not encrypted by Kaminari. It is protected by your operating system account and device security. Use full-disk encryption if you need it.

## 4. Network requests

Kaminari makes network requests directly from your device. We do not proxy or receive them.

- **AniList:** `https://graphql.anilist.co` for search, metadata, cover images and banners. AniList receives your IP address, user agent, query and request time under its own privacy policy.
- **Artwork hosts:** cover and banner images are fetched from the hosts AniList or your extensions return.
- **Registry URLs:** Kaminari does not expose a registry browser. The legacy registry command remains in the backend for a possible future APK runtime; if it is called, the app fetches the URL you choose and that host receives your request.
- **Extension hosts:** extensions you install can call `net.fetch` against hosts listed in their manifest. Those hosts receive your request and any data the extension sends.
- **Stream hosts:** when you play a video, the player fetches the stream and segments from the host the extension resolved. That host receives your request and may set cookies.
- **External mpv:** if you choose Open in mpv, mpv is a separate program with its own behavior and data handling.

These third parties may log IP addresses, timestamps, headers, cookies and requested URLs under their own policies. We do not control them.

## 5. Cookies and tracking

Kaminari does not use advertising or analytics cookies and does not track you across apps or websites. The embedded WebView may store cookies set by sites you access, including source sites and stream hosts. You can clear that data by deleting the app's WebView data folder or by resetting the app's site data.

## 6. Sharing and sale

We do not sell or rent personal data. We do not share data with advertisers. The third-party requests described above are made by your app directly, not by us.

## 7. Retention and deletion

Local data stays on your device until you delete it. To remove it on Windows:

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

Project repository: [[REPOSITORY_URL]].

## Note

This is a project policy, not legal advice. Have a qualified lawyer review it before public distribution, especially if you add accounts, telemetry, a public registry, or paid features.
