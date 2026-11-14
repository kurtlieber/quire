Quire 1.0.0 starts this fork's own line. The fork no longer follows [bjarneo/quire](https://github.com/bjarneo/quire): it began from upstream's 0.5.6, and its releases come only from here ([FORK.md](https://github.com/kurtlieber/quire/blob/main/FORK.md)).

## Download

| Platform | File | How to start it |
| --- | --- | --- |
| Linux, x86-64 | `quire-1.0.0-linux-x86_64.tar.gz` | Unpack it and run `./install.sh`. Then open Quire from the app launcher, or run `quire`. |
| macOS, Apple silicon | `quire-1.0.0-macos-arm64.zip` | Unzip it and move `Quire.app` to Applications. The app is not notarized, so right-click it and choose **Open** the first time. |
| Windows, x86-64 | `quire-1.0.0-windows-x86_64.zip` | Unzip it and run `quire.exe`. The app is not signed, so Windows SmartScreen can warn you. Choose **More info**, then **Run anyway**. |

There is no Android APK.

On a build of this fork from 0.5.6 on, select **Update** at the top right to install this version.

## New since 0.5.6

- **The mouse wheel scrolls past the caret** in source mode, on Wayland with an input method such as fcitx5. Before, it stopped where the caret would leave the screen. 0.5.6-kl.1 had this fix already.
- Versions are plain numbers again. 0.5.6-kl.1 was the only release with a suffix.
