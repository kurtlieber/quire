Quire 0.5.6 turns formatted text that you paste into Markdown, and adds a note menu on Android.

## Download

| Platform | File | How to start it |
| --- | --- | --- |
| Linux, x86-64 | `quire-0.5.6-linux-x86_64.tar.gz` | Unpack it and run `./install.sh`. Then open Quire from the app launcher, or run `quire`. |
| macOS, Apple silicon | `quire-0.5.6-macos-arm64.zip` | Unzip it and move `Quire.app` to Applications. The app is not notarized, so right-click it and choose **Open** the first time. |
| Windows, x86-64 | `quire-0.5.6-windows-x86_64.zip` | Unzip it and run `quire.exe`. The app is not signed, so Windows SmartScreen can warn you. Choose **More info**, then **Run anyway**. |
| Android 8.0 and later | `quire-0.5.6-android.apk` | Open the file on your phone and allow the install. |

The Linux build needs Wayland or X11 with Vulkan. The macOS and Windows builds come from CI and are not tested on those systems yet. Please open an issue if they do not start.

With 0.5.4 or later, select **Update** at the top right to install this version.

## New

- **Paste as Markdown.** A copy from a web page, a document or a mail app pastes as Markdown. It keeps the headings, bold, italic, strikethrough, links, images, lists, tasks, quotes, code blocks and tables. Google Docs copies work too.
- A copy with no formatting, such as one from a code editor, pastes as plain text. Every paste inside a code block is plain text.
- **Paste a link over text.** Select text and paste a web address, and Quire makes the link `[text](https://…)`.
- These work on the desktop and on Android.
- **Note menu on Android.** Long-press a note in preview, a tab or a note in the drawer to open its menu: Pin, Rename, Move to…, Share, the lock items and Delete note. In source mode, a long-press still selects text, and the selection menu has **Note actions…**. A tab's menu also has **Close tab** and **Close other tabs**.
- On Android, a paste from the keyboard's clipboard chip stays plain text. Use **Paste** in the text menu or `Ctrl+V` to convert.
