Quire is now Typo. Typo 2.0.0 is Quire 1.1.0 under a new name, from a renamed repository, [kurtlieber/typo](https://github.com/kurtlieber/typo). It began as a fork of [Quire](https://github.com/bjarneo/quire), by bjarneo, which remains its foundation.

## Download

| Platform | File | How to start it |
| --- | --- | --- |
| Linux, x86-64 | `typo-2.0.0-linux-x86_64.tar.gz` | Unpack it and run `./install.sh`. Then open Typo from the app launcher, or run `typo`. |
| macOS, Apple silicon | `typo-2.0.0-macos-arm64.zip` | Unzip it and move `Typo.app` to Applications. The app is not notarized, so right-click it and choose **Open** the first time. |
| Windows, x86-64 | `typo-2.0.0-windows-x86_64.zip` | Unzip it and run `typo.exe`. The app is not signed, so Windows SmartScreen can warn you. Choose **More info**, then **Run anyway**. |

There is no Android APK.

## Moving from Quire

Install Typo by hand this once. In Quire 1.0.0 and 1.1.0, **Update** opens this page instead of installing, because Quire installs only from the old repository's address. From 2.0.0 on, Typo's **Update** button installs new versions again.

1. Quit Quire.
2. Install Typo as above.
3. Open Typo. On its first start it moves Quire's settings, sync settings and sample notes to Typo's folders, so your folders, tabs, themes and sync carry over.
4. Remove Quire: on Linux, `~/.local/bin/quire`, `~/.local/share/applications/quire.desktop` and the `quire.svg` icons in `~/.local/share/icons/hicolor`; on macOS, `Quire.app`; on Windows, the folder with `quire.exe`.
5. If an AI app runs Quire's MCP server, change its command from `quire` to `typo`.

Notes, synced remotes and locked notes from Quire open in Typo unchanged. The default remote folder keeps Quire's name, `quire/<folder name>/`.

## What changed

- The app, its binary, its launcher entry and its release files are named Typo.
- Settings live in Typo's folders, `~/.config/typo` on Linux.
- Environment variables start with `TYPO_`, such as `TYPO_STATE`.
- The README credits Quire, and the license keeps the Quire authors' copyright notice.
