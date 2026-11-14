<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/mark-on-ink.svg">
    <img src="assets/brand/mark.svg" width="88" height="88" alt="">
  </picture>
</p>

<h1 align="center">typo</h1>

<p align="center">A plain editor for folders of Markdown files.<br>
Each note is a <code>.md</code> file on your disk, so any other app can open it.</p>

<p align="center">Typo began as a fork of <a href="https://github.com/bjarneo/quire">Quire</a>, by <a href="https://github.com/bjarneo">bjarneo</a>, and is built on it. See <a href="#credits">Credits</a>.</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#sync-your-devices">Sync</a> ·
  <a href="#use-it-with-your-ai-tools">MCP server</a> ·
  <a href="site/assets/quire.mp4">Video</a>
</p>

<p align="center">
  <a href="site/assets/quire.mp4"><img src="site/assets/quire.gif" width="960" alt="A short recording of the app, made when it was still Quire. It shows a new note with a link and a tag, bold text and a list from the toolbar, and an image in the preview. Then it shows the quick switcher, focus mode, a dark theme, the sync settings and a locked note."></a>
</p>

## What you get

Typo opens a folder and edits the Markdown files in it. There is no database, no import step and no lock-in.

- **Write in Markdown.** Typo reads [GitHub Flavored Markdown](https://github.github.com/gfm/), tables and task lists included, plus Obsidian's front matter, `[[links]]`, `![[embeds]]` and `#tags`. As in Obsidian, line breaks show as typed and only `~~text~~` strikes through. Source mode highlights the syntax as you type, in a monospaced font. `Ctrl+E` switches to a preview that reads like a printed page.
- **Format with a click.** A floating toolbar at the bottom of the note stays in view while you scroll. It makes text bold, italic or struck through, adds headings, links, lists, tasks, quotes and code, and inserts images. `Ctrl+B`, `Ctrl+I` and `Ctrl+K` work too. Settings, Editor can hide it.
- **Lock a note.** The lock button in the toolbar encrypts a note with its own password. A locked note shows nothing until you enter the password, and it syncs encrypted.
- **Paste from anywhere.** Formatted text from a web page or a document pastes as Markdown, with its headings, lists, links and emphasis. A web address pasted over selected text makes a link.
- **Add images.** Drag an image onto a note, paste one, or use the image button in the toolbar. Typo copies it into `attachments/` and links it. The preview shows `![alt](path)` and `![[name.png]]` images, and web images too.
- **Link notes.** Write `[[Note name]]` and `Ctrl`-click to follow it. A link to a missing note creates the note. Renaming a note rewrites the links to it.
- **Find anything.** `Ctrl+O` opens any note by a fuzzy name. `Ctrl+P` runs any command. `Ctrl+Shift+F` searches every note. Tags (`#idea`) get their own list.
- **Keep folders tidy.** Make folders in the file tree, drag notes into them, and use the `+` on a folder to make a note there. To delete a folder, right-click it. The folder moves to `.trash` with its files. To keep a note at the top of the tree, right-click it and select **Pin**. Pins sync with the notes. In the quick switcher, `Drafts/Idea` makes the note in `Drafts`.
- **Sync your devices.** Keep a notes folder the same on your computers and your phone, through object storage that you control, with optional encryption.
- **Focus.** `Ctrl+.` or the focus button hides everything but the text, dims the other paragraphs and keeps the line you type near the middle.
- **Look at home.** Pick a theme for light mode and one for dark mode from 14 themes, such as White, Black, Nord, Gruvbox, Catppuccin, Tokyo Night, Rosé Pine and Solarized. Pick an accent colour, or type any hex value. "Match system" switches between your two themes. On Omarchy, your Omarchy theme is one of the choices.
- **Stay up to date.** When a new release is out, an **Update** button shows at the top right. Select it, and Typo installs the new version and restarts. This works on Linux, macOS and Windows.
- **Work with AI tools.** A built-in MCP server lets Claude Desktop, Cursor and other MCP clients read and search your notes.

<table>
  <tr>
    <td><img src="site/assets/editor-light.png" alt="Source mode in the light theme, with the file tree, the formatting toolbar, the note and the outline."></td>
    <td><img src="site/assets/preview-dark.png" alt="Preview mode in the dark theme."></td>
  </tr>
  <tr>
    <td align="center">Source, light</td>
    <td align="center">Preview, dark</td>
  </tr>
  <tr>
    <td><img src="site/assets/quick-switcher.png" alt="The quick switcher with a fuzzy query."></td>
    <td><img src="site/assets/focus-mode.png" alt="Focus mode with the other paragraphs dimmed."></td>
  </tr>
  <tr>
    <td align="center">Quick switcher</td>
    <td align="center">Focus mode</td>
  </tr>
</table>

### On your phone

The Android app in `android/` is the same editor for a phone: one pane, a drawer for the files, and tabs for open notes. It is still Quire's app, under Quire's name, and this repository does not build it yet.

<p align="center">
  <img src="site/assets/phone-editor.png" width="250" alt="Quire on Android with a note in source mode and the Markdown toolbar above the keyboard.">
  <img src="site/assets/phone-preview.png" width="250" alt="Quire on Android with a note in preview that shows an image.">
  <img src="site/assets/phone-switcher.png" width="250" alt="The quick switcher on Android, with the notes that match a search.">
</p>

## Install

Download the build for your system from the [latest release](https://github.com/kurtlieber/typo/releases/latest):

| System | File |
| --- | --- |
| Linux, x86-64 | `typo-<version>-linux-x86_64.tar.gz`. Unpack it and run `./install.sh`. |
| macOS, Apple silicon | `typo-<version>-macos-arm64.zip`. The app is not notarized, so right-click it and choose **Open** the first time. |
| Windows, x86-64 | `typo-<version>-windows-x86_64.zip`. The app is not signed, so SmartScreen can warn you. |

The releases have no Android APK. To build Typo yourself, follow the steps below.

### Desktop (Linux)

You need Rust and, on Linux, the `libxkbcommon` and `wayland` development packages. `rustup` installs the pinned toolchain on the first build.

```sh
git clone https://github.com/kurtlieber/typo
cd typo/desktop
./install.sh        # builds, then installs typo, its icon and a launcher entry for your user
typo ~/notes        # or open Typo from the app launcher
```

The desktop app is built with [Rune](https://github.com/anomalyco/rune), a GPU-rendered Rust UI framework. Rune also targets macOS and Windows, but Typo is tested on Linux only. See [`desktop/README.md`](desktop/README.md).

### Android

You need JDK 21 and the Android SDK.

```sh
cd typo/android
./gradlew assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

See [`android/README.md`](android/README.md).

## Use it with your AI tools

Turn on the server in Settings, MCP. Then paste the client configuration from that page into your AI app. With the stdio transport, the AI app starts Typo's helper itself:

```json
{
  "mcpServers": {
    "typo": { "command": "typo", "args": ["mcp", "--folder", "/path/to/notes"] }
  }
}
```

Access is read-only by default. Each tool has its own switch, and hidden folders stay out of listings and search. The HTTP transport serves `http://127.0.0.1:3917/mcp` while the app is open and needs the access token from Settings.

## Sync your devices

Typo keeps a copy of your notes in object storage that you control. The desktop and Android apps use the same format, Quire's, so a note you write on one device opens on the other.

1. Make a bucket at your provider and an access key for it.
2. Open Settings, Sync & backup, and select the provider.
3. Fill in the bucket and the key.
4. Select **Test connection**. Sync starts when the test passes.

Typo works with Amazon S3, Hetzner, Azure Blob Storage, Google Cloud Storage, Cloudflare R2, Backblaze B2, DigitalOcean Spaces, Wasabi, Scaleway, OVHcloud and any other S3-compatible server, such as MinIO.

- **Encrypt before upload** encrypts each note with AES-256-GCM before upload. The key comes from your passphrase, so the provider stores neither note names nor note text. If you lose the passphrase, nobody can open the remote copy.
- **When to sync** is **On save**, **Every 5 min** or **Manually**. **Sync now** is also in the command palette.
- When two devices change the same note, Typo keeps both versions. The other device's text goes to `<name> (conflict <device>).md` next to the note.
- Images in the notes folder sync too, up to 50 MB each.
- Folders sync too, empty ones included. A note or a folder that you delete on one device moves to `.trash` on the other devices.

Each notes folder syncs to its own remote folder, `quire/<folder name>/` unless you set another one in **Remote folder**. The default keeps Quire's name, so a folder that Quire synced syncs on in Typo. To sync one notes folder between two devices, give it the same remote folder on both.

[`SYNC.md`](SYNC.md) describes the remote format, so other tools can read and write it.

## Not in this version

- With "Ask before changes" on, MCP tools that write return an error, because there is no approval prompt yet.
- The macOS and Windows builds come from CI and are not tested on those systems yet.
- On Android, a paste from the context menu inserts no image, and SVG images do not show in the preview.

## In this repository

| Folder | What it holds |
| --- | --- |
| [`desktop/`](desktop) | The desktop app, in Rust with Rune. |
| [`android/`](android) | The Android app, in Kotlin with Jetpack Compose. |
| [`site/`](site) | Quire's website, kept from upstream. This repository does not publish it. |
| [`design/`](design) | Quire's design prototype, which both apps follow. |
| [`assets/`](assets) | The brand marks and the fonts, which keep their SIL Open Font License. |

## Credits

Typo began as a fork of [Quire](https://github.com/bjarneo/quire), by [bjarneo](https://github.com/bjarneo). Quire is the foundation of everything here: the editor, its design and brand, the sync format, the MCP server, the Android app and the website all come from Quire, and most of the code is still Quire's. Typo split from it in November 2026 to follow its own path, and took its own name with 2.0.0. Thank you, bjarneo, for building Quire and for sharing it under the MIT license.

Typo keeps Quire's file and sync formats, so a notes folder, a synced remote or a locked note made with Quire opens in Typo. [FORK.md](FORK.md) has the history and what changed.

The desktop app is built on [Rune](https://github.com/anomalyco/rune), and Typo's Markdown parser is [comrak](https://github.com/kivikakk/comrak).

## License

Typo is MIT licensed, like Quire. See [LICENSE](LICENSE), which keeps the Quire authors' copyright notice.
