# Quire for Android

Quire is a plain editor for folders of Markdown notes. This is the Android app, in Kotlin with Jetpack Compose. It runs on Android 8.0 and later.

The app is the same editor as the desktop app, made for a phone: one pane, a drawer for the files, and tabs for open notes. It reads and writes the same sync format, so a notes folder can be the same on the phone and on the computer.

## Requirements

- JDK 17 or later. JDK 21 works.
- The Android SDK with platform `android-36` and build tools `36.0.0`.
- A `local.properties` file that points to the SDK, or the `ANDROID_HOME` variable.

```sh
echo "sdk.dir=$HOME/Android/Sdk" > local.properties
```

The Gradle wrapper downloads Gradle 9.9.1 on the first build.

## Build and test

To build the debug APK, run this command:

```sh
./gradlew assembleDebug
```

The APK is `app/build/outputs/apk/debug/app-debug.apk`.

To run the unit tests, run this command:

```sh
./gradlew testDebugUnitTest
```

The tests run on the JVM, with no device. They cover the sync engine, the encryption test values in [`../SYNC.md`](../SYNC.md), an S3 round trip against a local test server, the image rules, the toolbar rules, the themes, the folder delete and the pins.

## Release build

The release build is signed with a key from Gradle properties, so no key or password is in this repository. Put these lines in `~/.gradle/gradle.properties`, with your own values:

```properties
QUIRE_RELEASE_STORE_FILE=/path/to/release.jks
QUIRE_RELEASE_STORE_PASSWORD=...
QUIRE_RELEASE_KEY_ALIAS=...
QUIRE_RELEASE_KEY_PASSWORD=...
```

Then run this command:

```sh
./gradlew assembleRelease
```

The APK is `app/build/outputs/apk/release/app-release.apk`. Without the properties, the build makes an unsigned APK, `app-release-unsigned.apk`, which Android does not install.

A release APK cannot update a debug install, because their signatures differ. To change from one to the other, uninstall the app first. An uninstall deletes the notes in the app storage, so move them to a folder outside the app first.

## Install

To install on a device over adb, run this command:

```sh
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

If more than one device is connected, give the serial number:

```sh
adb devices
adb -s <serial> install -r app/build/outputs/apk/debug/app-debug.apk
```

A release build shows an "Update" button at the top right when a newer release with an APK is on GitHub. Select it to download the APK and open the system installer. To turn the check off, go to Settings > Files & links > Check for updates.

## Where the notes are

- "Create a new folder" makes a folder in the app storage, under `files/vaults/<name>`.
- "Open a folder…" uses the system folder picker. Quire keeps the permission, so the folder opens again after a restart. Use it for a folder that another app syncs.
- On the first start, Quire writes the sample folder "Field Notes". Settings > Files & links > Restore puts it back.

Quire saves an edit 500 ms after you stop typing, and again when the app goes to the background. It reads the folder again when the app comes back, so edits from other apps show up.

## Folders

- To make a folder, use "New folder" in the drawer or in the command palette.
- To add a note or a folder inside a folder, long-press the folder. The "+" at the end of a folder row adds a note there.
- To delete a folder, long-press it and select "Delete folder". A folder with notes asks first. The folder moves to `.trash` with every file in it.
- To open the note menu, long-press a note in the drawer, a tab, or the note text in preview mode. The menu has Pin or Unpin, Rename, Move to…, Share, the lock items, and Delete note. A tab also has Close tab and Close other tabs. Delete note asks first, and moves the note to `.trash`. A locked note has no Share until you unlock it.
- A long-press in source mode selects text. To open the note menu from there, select "Note actions…" in the text selection menu. When the selection menu is full, the item is under the "⋮" button.
- To move a note, long-press it in the drawer and drag it onto a folder. Drop it on the folder name at the top to move it to the root. You can also select "Move to…" in the note menu.
- In the quick switcher, type a path such as `Drafts/Idea` to create a note in that folder.
- To pin a note, select "Pin" in the note menu, or use the pin button in the toolbar. The note shows in the Pinned group at the top of the drawer, with its folder name next to it. The file stays in its folder. Pins are saved for each notes folder and sync with it, in the same way as on the desktop. A pin or an unpin starts a sync a moment later, unless sync is set to "Manually".

## Writing

- While the editor has focus, a toolbar above the keyboard makes text bold, italic or struck through, and adds headings, links, note links, lists, tasks, quotes, code and images. It scrolls sideways when it does not fit. The rules are the same as on the desktop.
- The image button opens the photo picker. Quire copies the image into `attachments/` in the notes folder and links it at the caret. An image that another app drops on the editor goes in the same way.
- The preview shows `![alt](path)` images relative to the note, `![[name.png]]` images from anywhere in the folder, and `https://` images.
- A paste of formatted text from a web page, a mail or a document becomes Markdown, with its headings, lists, links and emphasis. A web address pasted over selected text links that text. Inside a code block, a paste stays plain text. This works for Paste in the text menu and for `Ctrl+V`, but not for the clipboard suggestion of the keyboard.

## Themes

Settings > Appearance has 14 themes: 6 for light mode and 8 for dark mode. Pick one for each mode. "Match system" follows the dark setting of the phone. Pick an accent colour from 9 swatches, or type a `#rrggbb` value.

## Sync

Settings > Sync & backup connects the notes folder to object storage that you control: Amazon S3, Hetzner, Azure, Google Cloud, Cloudflare R2, Backblaze B2, DigitalOcean, Wasabi, Scaleway, OVHcloud or another S3-compatible server.

1. Select the provider and fill in the bucket and the keys.
2. Select **Test connection**. Sync starts when the test passes.

Quire syncs when the folder opens, when the app comes to the front or goes to the background, after a save, every 5 minutes, or only on **Sync now**, as you choose in **When to sync**.

- Each notes folder has its own **Remote folder**, `quire/<folder name>/` by default. To sync a folder with the desktop app, give it the same remote folder on both devices.
- Credentials and the passphrase are encrypted with a key in the Android Keystore.
- The device name in conflict copies is the phone model, for example `SM-S931B`.

See [`../SYNC.md`](../SYNC.md) for the format.

## Not in this version

- A paste from the context menu inserts no image. The Compose text field gives no clipboard images. Only `Ctrl+V` on a hardware keyboard pastes copied image files.
- SVG images do not show in the preview, because Android has no built-in SVG decoder. AVIF images show on Android 12 and later.

## Fonts

The fonts are under the SIL Open Font License. The license files are in `app/src/main/assets/licenses`.
