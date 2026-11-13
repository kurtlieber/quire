Quire 0.5.4 updates itself and syncs pins between devices.

## Download

| Platform | File | How to start it |
| --- | --- | --- |
| Linux, x86-64 | `quire-0.5.4-linux-x86_64.tar.gz` | Unpack it and run `./install.sh`. Then open Quire from the app launcher, or run `quire`. |
| macOS, Apple silicon | `quire-0.5.4-macos-arm64.zip` | Unzip it and move `Quire.app` to Applications. The app is not notarized, so right-click it and choose **Open** the first time. |
| Windows, x86-64 | `quire-0.5.4-windows-x86_64.zip` | Unzip it and run `quire.exe`. The app is not signed, so Windows SmartScreen can warn you. Choose **More info**, then **Run anyway**. |
| Android 8.0 and later | `quire-0.5.4-android.apk` | Open the file on your phone and allow the install. |

The Linux build needs Wayland or X11 with Vulkan. The macOS and Windows builds come from CI and are not tested on those systems yet. Please open an issue if they do not start.

This is the last release that you install by hand. From 0.5.4 on, Quire tells you when a new version is out.

## New

- **Update button.** When a newer release is out, an **Update** button shows at the top right.
  - On Linux, macOS and Windows, select it, and Quire downloads the new version, checks that it runs, puts it in place and restarts.
  - On Android, Quire downloads the APK and opens the system installer. The first time, Android asks you to allow installs from Quire.
- Quire checks GitHub when it starts and every 6 hours. Settings, Files & links has **Check for updates** and **Check now**. The check sends no personal data.
- **Pins sync.** Pinned notes now sync between your devices with the notes. A pin that you add or remove on one device is added or removed on the others. A device with an older version keeps its pins.
- `quire --version` prints the version.

## Notes

- If Quire cannot write to its own folder, for example `/usr/bin`, **Update** opens the release page.
- An Android app that you installed from a computer with Android Studio or `adb` is a debug build. It does not check for updates, because a release APK cannot install over a debug build.
