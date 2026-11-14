Quire 1.1.0 reads Markdown to a defined standard: [GitHub Flavored Markdown](https://github.github.com/gfm/), plus the Obsidian syntax that notes use. Tables now show as tables.

## Download

| Platform | File | How to start it |
| --- | --- | --- |
| Linux, x86-64 | `quire-1.1.0-linux-x86_64.tar.gz` | Unpack it and run `./install.sh`. Then open Quire from the app launcher, or run `quire`. |
| macOS, Apple silicon | `quire-1.1.0-macos-arm64.zip` | Unzip it and move `Quire.app` to Applications. The app is not notarized, so right-click it and choose **Open** the first time. |
| Windows, x86-64 | `quire-1.1.0-windows-x86_64.zip` | Unzip it and run `quire.exe`. The app is not signed, so Windows SmartScreen can warn you. Choose **More info**, then **Run anyway**. |

There is no Android APK.

On a build of this fork from 0.5.6 on, select **Update** at the top right to install this version.

## New since 1.0.0

- **Tables** show as tables, with each column's alignment. Columns size to their text, as on a web page. A `|` inside a cell is written `\|`, as in Obsidian.
- **Front matter** shows as a small table of properties at the top of the preview, as in Obsidian. Before, it showed as text between two rules.
- **Nested blocks** show nested: a list inside a quote, code inside a list item, lists inside lists.
- **Web addresses** in the text are links.
- A line of `---` under a line of text makes that line a heading, as in Obsidian and on GitHub.
- The outline, tags, backlinks and the link rewrite on rename read the same parse as the preview. A `#tag` or `[[link]]` inside code no longer counts.
- Unchanged, as in Obsidian: line breaks show as typed, and a single `~` stays as typed (only `~~text~~` strikes through). Raw HTML still shows as text, and source mode highlights as before.

Quire uses [comrak](https://github.com/kivikakk/comrak), a Rust port of GitHub's own Markdown parser.
