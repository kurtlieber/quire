# Typo for the desktop

Typo is a plain editor for folders of Markdown files. Each note is a `.md` file on your disk, so any other app can open it. This is the desktop app, built with [Rune](https://github.com/anomalyco/rune), a GPU-rendered Rust UI framework.

## Build and run

You need the Rust toolchain. `rust-toolchain.toml` pins the version that Rune uses, and `rustup` installs it on the first build. On Linux you also need the `libxkbcommon` and `wayland` development packages.

```sh
cargo run --release              # opens the last folder, or the welcome screen
cargo run --release -- ~/notes   # opens a folder
cargo run --release -- note.md   # opens a note in its folder
```

To install Typo for your user, with a launcher entry and an icon, run:

```sh
./install.sh
```

The script puts the binary in `~/.local/bin/typo`, the icon in `~/.local/share/icons` and `typo.desktop` in `~/.local/share/applications`.

## What it does

### Writing

- Edits the `.md` files of a folder. Saves each note half a second after you stop typing, and on quit.
- Shows source with live Markdown highlighting, or a rendered preview (`Ctrl+E`). The preview keeps each line break as you typed it, in paragraphs, quotes and list items.
- Has a floating formatting toolbar at the bottom of the note in source mode. It stays in view, and the note scrolls under it. The buttons are bold, italic, strikethrough, heading, link, note link, lists, task, quote, code, image, pin and lock. `Ctrl+B`, `Ctrl+I` and `Ctrl+K` do the same as the first buttons. Turn off Settings, Editor, **Formatting toolbar** to hide it.
- Hides everything but the text in focus mode. Use `Ctrl+.` or the focus button in the title bar.
- Locks a note with its own password: use the lock button at the end of the toolbar, or "Lock note with a password…" in the command palette. Typo encrypts the note with AES-256-GCM and a key from the password. The file on disk, search, the link index and sync see only the encrypted text. To read the note, enter the password. Each save encrypts the note again. The note locks again when you close its tab or select the lock button. "Remove password" in the command palette saves it as plain text again. If you forget the password, nobody can open the note. `../SYNC.md` describes the format.

- Turns formatted text that you paste into Markdown. A copy from a web page, a document or a mail app keeps its headings, bold, italic, links, lists, quotes, code blocks and tables. A copy with no formatting, such as one from a code editor, pastes as plain text, and so does every paste inside a code block.
- Makes a link when you paste a web address over selected text: `[text](https://…)`.

### Images

- Shows images in the preview: `![alt](path)` relative to the note, `![[name.png]]` anywhere in the folder, and `https://` images. PNG, JPEG, GIF, WebP, BMP, SVG and AVIF work.
- Adds an image when you drop it on a note, paste it, or pick it with the image button in the toolbar (`Ctrl+Shift+I`). Images from outside the folder are copied into `attachments/`. A pasted screenshot is saved as `attachments/Pasted image <date> <time>.png`.
- Takes file drops on Wayland too. Rune's winit gets no file drops from Wayland, so `vendor/winit` adds them. See `vendor/winit/src/platform_impl/linux/wayland/dnd.rs`.

### Links and search

- Links notes with `[[Note name]]`. `Ctrl`-click a link to follow it. A link to a missing note creates the note.
- Rewrites links in other notes when you rename a note.
- Finds tags (`#tag`), searches every note (`Ctrl+Shift+F`), and lists headings and backlinks.
- Opens any note with the quick switcher (`Ctrl+O`) and runs any command from the palette (`Ctrl+P`).

### Folders

- Makes folders: use the folder button next to the panel tabs, "New folder" in the command palette, or right-click a folder.
- Creates a note inside a folder: use the `+` on the folder's row, right-click the folder, or type `Folder/Name` in the quick switcher. A path creates the folders it names.
- Moves a note into a folder when you drag it onto the folder. Drop it on the folder name or on empty space to move it to the root. Links that spell out the old folder follow the note.
- Moves deleted notes to `.trash` inside the folder.
- Pins a note to the Pinned group at the top of the tree. Right-click the note and select **Pin**, drag it onto the group, select the pin button in the toolbar, or use "Pin note" in the command palette. The file stays in its folder. The tree shows the folder name next to the pinned note. Pins sync with the notes folder when sync is on. Without sync, they are saved in `state.json`.
- Deletes a folder from its right-click menu. A folder with notes asks first. The folder moves to `.trash` with every file in it, so you can get it back.
- Picks up changes that other apps make to the folder while Typo is open.

### Look

- Has 14 themes in Settings, Appearance: 6 for light mode and 8 for dark mode. You pick one for each mode, and "Match system" switches between them.
- Lets you pick the accent colour from 9 swatches, or type a `#rrggbb` value. An accent that is too dark to read on a dark theme gets lighter for text.
- On Omarchy, offers the current Omarchy theme in both lists and follows theme switches while Typo runs. Without a pick, "Match system" uses the Omarchy theme.

### Window

- On Linux, the title bar has no window buttons. The window manager closes and places the window. To quit, press `Ctrl+Q` or use **Quit** in the command palette.
- On Wayland, a touchpad swipe scrolls a note as far as Chromium scrolls a web page. Rune's winit applies the touchpad value 1:1, so `vendor/winit` scales it by 5.3.
- On macOS the shortcuts use `⌘` in place of `Ctrl`.

On first launch Typo writes a sample folder, Field Notes, to `~/Documents/Field Notes`. If that folder already exists, the sample goes to Typo's data folder instead. Settings and open tabs live in `state.json` in Typo's config folder, `~/.config/typo/` on Linux, never in a notes folder.

### Updates

- Typo asks GitHub for the latest release when it starts and every 6 hours. When a newer version is out, an **Update** button shows at the top right of the title bar.
- Select **Update**, and Typo downloads the release file for your system and runs the new binary once to check it. Then it puts the new version in place of the old one and restarts.
  - On Linux, Typo replaces the `typo` binary.
  - On macOS, it replaces `Typo.app`.
  - On Windows, it replaces `typo.exe`.
- If Typo cannot write to its own folder, for example `/usr/bin`, **Update** opens the release page.
- To turn off the check, go to Settings, Files & links, and turn off **Check for updates**. **Check now** asks at once. The check sends no personal data, only the user agent `typo-updater`.
- To print the version, run `typo --version`.

## MCP server

Typo can serve a folder to AI apps that speak the Model Context Protocol, such as Claude Desktop or Cursor. Turn it on in Settings, MCP. Settings, MCP also shows the client configuration to copy.

With the stdio transport, the AI app starts Typo's helper itself:

```json
{ "mcpServers": { "typo": { "command": "typo", "args": ["mcp", "--folder", "/path/to/notes"] } } }
```

With the HTTP transport, Typo serves `http://127.0.0.1:3917/mcp` while the app is open. Each request needs the access token from Settings as a `Bearer` header.

Access is read-only by default. Each tool has its own switch, and hidden folders stay out of listings and search. When "Ask before changes" is on, tools that write return an error, because this version cannot show the approval prompt yet. Turn the option off to allow edits.

## Sync

Settings, Sync & backup connects a notes folder to a bucket. **Test connection** reads the remote folder and reports what it holds. After it passes, Typo syncs when the folder opens, after you save, every 5 minutes, or only when you select **Sync now**, as you choose. The status bar shows the last sync. Select it to open the sync settings.

- Each notes folder has its own **Remote folder**. Without a value, it is `quire/<folder name>/`.
- Notes, images up to 50 MB and folders sync. A file that you delete on one device moves to `.trash` on the others.
- A request that the service does not answer in time is tried again up to 4 times. Transfers run 8 at a time.

Typo keeps a record of the last sync for each folder and bucket in `sync/` in its config folder. The record lets Typo tell an edit from a delete. See [`../SYNC.md`](../SYNC.md) for the format.

## Not in this version

- Very long notes type slower. A keystroke takes about 2.5 ms at 400 lines and 6 ms at 1,000 lines, but about 30 ms at 3,000 lines, because the text field lays out the whole note on each edit.
- The passphrase and keys you enter are saved in `state.json`, which only your user can read. They are not in the system keychain.
- The macOS and Windows builds come from CI and are not tested on those systems yet.

## Develop

```sh
cargo test                                               # unit tests
cargo run -- --snap shot.png --size 1280x800 --scale 2   # render a frame headless
cargo run -- --snap shot.png --scene dark                # scenes: welcome, preview, dark, focus, settings
cargo run -- --script events.json --snap shot.png --at mark:end   # play input events, then render
```

- A headless run opens the folder in `TYPO_FOLDER`. Set `HOME`, `XDG_CONFIG_HOME` and `XDG_DATA_HOME` to an empty folder, so the run does not touch your own settings.
- The sync tests run two devices against an S3 test server in the test process. Three more tests check a real store, and they only run when you ask for them. Each one reads the settings file in `TYPO_LIVE_STATE`, and none of them prints a key:

  ```sh
  TYPO_LIVE_STATE=~/.config/typo/state.json cargo test live_ -- --ignored --nocapture
  ```

  `live_store_check` also writes and deletes one small object, `.quire/probe.txt`.
- `TYPO_SMOOTHING` picks another text rasterizer, for comparing renders. `ref/` holds renders of the design in Chromium, used as the reference for pixel comparisons.
- `vendor/winit` is Rune's winit with Wayland file drops and the touchpad scroll scale added. `Cargo.toml` points the `winit` patch at it.
