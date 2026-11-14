# This fork

Typo is Kurt Lieber's fork of [bjarneo/quire](https://github.com/bjarneo/quire), which is MIT licensed. Quire is its foundation (README, "Credits"). It split from upstream at v0.5.6 on 2026-11-14 and has its own line from 1.0.0 ([kurtlieber/dotfiles#622](https://github.com/kurtlieber/dotfiles/issues/622)). Upstream's maintainer would rather people fork than file issues. So nothing follows upstream, and nothing goes back to it. Earlier work on the fork: [#540](https://github.com/kurtlieber/dotfiles/issues/540), its upstream sync [#570](https://github.com/kurtlieber/dotfiles/issues/570), its first own release [#597](https://github.com/kurtlieber/dotfiles/issues/597).

## The name

The fork shipped as Quire up to 1.1.0, from `kurtlieber/quire`. With 2.0.0 the app and the repository are Typo ([kurtlieber/dotfiles#642](https://github.com/kurtlieber/dotfiles/issues/642)). GitHub redirects `kurtlieber/quire` here. Kurt's earlier, unrelated editor that had this name is archived as [kurtlieber/typo-archived](https://github.com/kurtlieber/typo-archived).

What changed name: the binary (`typo`, `Typo.app`, `typo.exe`), the release files (`typo-<version>-…`), the window and launcher names, the app's folders (`~/.config/typo` and `~/.local/share/typo` on Linux), the `TYPO_*` environment variables, the MCP server name and the updater's user agent.

What keeps Quire's name, so that notes, remotes and locked notes from Quire work in Typo:

- The sync format in [SYNC.md](SYNC.md): `.quire/format.json`, `.quire/manifest.*`, the check text `quire`, the `QUIRE1` blob magic and the default remote folder `quire/<folder name>/`.
- The locked note header `%QUIRE-LOCKED-NOTE`. Typo's notice line says "Open it in Typo", and Typo still opens notes with Quire's notice.
- The Android app in `android/`, which this fork does not build yet (#547). It still asks upstream for updates, and it opens a locked note only when its notice line is Quire's. When the fork builds an APK, rename the app and change its `Lock.kt` to skip any notice line, as `desktop/src/lock.rs` does.
- `site/` (upstream's website, which this fork does not publish), `design/` and `desktop/ref/`, which are Quire's design.

On its first start, Typo 2.0.0 moves Quire's config and data folders to Typo's (`store::move_from_quire`). It skips a folder that Typo already has, and it skips the move on a run with `TYPO_STATE` set.

## Branches and merges

- `main` is what ships (Releases, below).
- The repository allows only merge commits.
- Commits use a GitHub noreply address.

## Taking an upstream change

Nothing merges from upstream on its own. To take one of upstream's changes, bring it in a pull request that Kurt reviews:

```sh
git fetch https://github.com/bjarneo/quire.git main
git switch -c upstream-<topic> origin/main
git cherry-pick <commit>      # or: git merge FETCH_HEAD
git push origin upstream-<topic>
```

Upstream's code says Quire where this fork says Typo, its `desktop/src/update.rs` asks upstream for releases, and its version in `desktop/Cargo.toml` is lower than this fork's. Keep this fork's in all three.

## The updater

Typo's updater (`desktop/src/update.rs`) asks this repository for releases: `API`, `DOWNLOADS` and `PAGE` name `kurtlieber/typo`. It compares the latest release's version with its own, `desktop/Cargo.toml`'s, as three numbers, and offers only a newer one. It installs only a file whose address starts with `DOWNLOADS`, and only when the new binary's `--help` starts with "Typo".

Quire 1.0.0 and 1.1.0 still find 2.0.0, through GitHub's redirect, but they install only files from `kurtlieber/quire`. So their **Update** button opens the release page, and Typo is installed by hand once. Builds from 0.5.6 to 0.5.6-kl.1 offer 1.0.0 and stop there. The fork's v0.5.4 still asks upstream.

## The fork's own fixes

- **The mouse wheel stops at the caret** in source mode, on Wayland with an input method such as fcitx5. The fix is in the vendored winit, `desktop/vendor/winit/src/platform_impl/linux/wayland/seat/text_input/mod.rs`. Kurt reported it upstream as [bjarneo/quire#2](https://github.com/bjarneo/quire/issues/2). Its tests are outside the app's `cargo test`; run `cargo test --lib text_input` in `desktop/vendor/winit`.

- **Markdown is GitHub Flavored Markdown** plus Obsidian's syntax, parsed by [comrak](https://github.com/kivikakk/comrak) in `desktop/src/md/gfm.rs` (1.1.0, [kurtlieber/dotfiles#636](https://github.com/kurtlieber/dotfiles/issues/636)). The preview, outline, tags, backlinks and the link rewrite on rename all read its tree. Source mode's highlighting still uses the line helpers in `desktop/src/md.rs`. The Android app keeps its own parser.

## Releases

The version in `desktop/Cargo.toml` is the release. A pull request that changes `desktop/` also:

1. Raises that version. Use three numbers, `X.Y.Z`: the updater reads no more. `cargo build` updates `desktop/Cargo.lock` to match.
2. Rewrites `.github/release-notes.md` for the new version.

Kurt's merge ships it, under his standing yes for this fork's releases (2026-11-13, #597). `.github/workflows/ship.yml` then:

1. Tags `main` as `v<version>`.
2. Creates the release, not yet the latest. Its notes are `.github/release-notes.md` and a link to the changes since the previous release.
3. Runs `release.yml` for the tag, which builds the Linux, macOS and Windows archives and attaches them. A tag that a workflow pushes doesn't start another workflow, so Ship runs it itself.
4. Makes the release the latest once all three files are attached and the Linux build's `typo --version` names the version. Typo's Update button offers it from then on.

A merge that changes `desktop/` but keeps the version fails Ship, so the change can't sit unreleased unseen. Bump the version in another pull request to ship it. A merge that changes nothing under `desktop/` ships nothing.

A failed build leaves the release not the latest. Run **Ship** again from the Actions tab to build it again. A run with **dry_run** on, from any branch, pushes nothing and prints the tag it would push.

A failed run of any workflow here posts to the "Failure alerts" issue in kurtlieber/dotfiles, from the `quire-fork-watch` timer on archie (#597).

Releases before 1.0.0 were upstream's (`v0.5.4`, `v0.5.6`) and one own release, `v0.5.6-kl.1`, from when this fork followed upstream. Releases up to 1.1.0 carry Quire's name.

The Pages workflow is turned off in this fork: the website stays upstream's.

## Where it is installed

- monarchy and anarchy: a chezmoi script in [kurtlieber/dotfiles](https://github.com/kurtlieber/dotfiles) installs a pinned release on a new host (app note under `homelab/.agents/rules/apps/` there). From then on, Typo's Update button keeps it current.
- mbp: Typo's Update button keeps it current.
- Android: later, signed with Kurt's own key.
