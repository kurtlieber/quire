# This fork

Kurt Lieber's fork of [bjarneo/quire](https://github.com/bjarneo/quire), which is MIT licensed. It split from upstream at v0.5.6 on 2026-11-14 and has its own line from 1.0.0 ([kurtlieber/dotfiles#622](https://github.com/kurtlieber/dotfiles/issues/622)). Upstream's maintainer would rather people fork than file issues. So nothing follows upstream, and nothing goes back to it. Earlier work on the fork: [#540](https://github.com/kurtlieber/dotfiles/issues/540), its upstream sync [#570](https://github.com/kurtlieber/dotfiles/issues/570), its first own release [#597](https://github.com/kurtlieber/dotfiles/issues/597).

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

Upstream's `desktop/src/update.rs` asks upstream for releases, and its version in `desktop/Cargo.toml` is lower than this fork's. Keep this fork's in both.

## The updater

Quire's updater (`desktop/src/update.rs`) asks this fork for releases: `API`, `DOWNLOADS` and `PAGE` name `kurtlieber/quire`. It compares the latest release's version with its own, `desktop/Cargo.toml`'s, as three numbers, and offers only a newer one. A device stays on this fork once it runs one of this fork's builds.

Every build of this fork from 0.5.6 on offers 1.0.0. The fork's v0.5.4 still asks upstream.

The Android app (`android/`) still asks upstream: this fork builds no APK.

## The fork's own fixes

- **The mouse wheel stops at the caret** in source mode, on Wayland with an input method such as fcitx5. The fix is in the vendored winit, `desktop/vendor/winit/src/platform_impl/linux/wayland/seat/text_input/mod.rs`. Kurt reported it upstream as [bjarneo/quire#2](https://github.com/bjarneo/quire/issues/2). Its tests are outside Quire's `cargo test`; run `cargo test --lib text_input` in `desktop/vendor/winit`.

- **Markdown is GitHub Flavored Markdown** plus Obsidian's syntax, parsed by [comrak](https://github.com/kivikakk/comrak) in `desktop/src/md/gfm.rs` (1.1.0, [kurtlieber/dotfiles#636](https://github.com/kurtlieber/dotfiles/issues/636)). The preview, outline, tags, backlinks and the link rewrite on rename all read its tree. Source mode's highlighting still uses the line helpers in `desktop/src/md.rs`. The Android app keeps its own parser.

## Releases

The version in `desktop/Cargo.toml` is the release. A pull request that changes `desktop/` also:

1. Raises that version. Use three numbers, `X.Y.Z`: Quire's updater reads no more. `cargo build` updates `desktop/Cargo.lock` to match.
2. Rewrites `.github/release-notes.md` for the new version.

Kurt's merge ships it, under his standing yes for this fork's releases (2026-11-13, #597). `.github/workflows/ship.yml` then:

1. Tags `main` as `v<version>`.
2. Creates the release, not yet the latest. Its notes are `.github/release-notes.md` and a link to the changes since the previous release.
3. Runs `release.yml` for the tag, which builds the Linux, macOS and Windows archives and attaches them. A tag that a workflow pushes doesn't start another workflow, so Ship runs it itself.
4. Makes the release the latest once all three files are attached and the Linux build's `quire --version` names the version. Quire's Update button offers it from then on.

A merge that changes `desktop/` but keeps the version fails Ship, so the change can't sit unreleased unseen. Bump the version in another pull request to ship it. A merge that changes nothing under `desktop/` ships nothing.

A failed build leaves the release not the latest. Run **Ship** again from the Actions tab to build it again. A run with **dry_run** on, from any branch, pushes nothing and prints the tag it would push.

A failed run of any workflow here posts to the "Failure alerts" issue in kurtlieber/dotfiles, from the `quire-fork-watch` timer on archie (#597).

Releases before 1.0.0 were upstream's (`v0.5.4`, `v0.5.6`) and one own release, `v0.5.6-kl.1`, from when this fork followed upstream.

The Pages workflow is turned off in this fork: the website stays upstream's.

## Where it is installed

- monarchy and anarchy: the chezmoi script `run_onchange_after_quire.sh` in [kurtlieber/dotfiles](https://github.com/kurtlieber/dotfiles) installs a pinned release on a new host (app note `homelab/.agents/rules/apps/quire.md` there). From then on, Quire's Update button keeps it current.
- mbp: Quire's Update button keeps it current.
- Android: later, signed with Kurt's own key.
