# This fork

Kurt Lieber's fork of [bjarneo/quire](https://github.com/bjarneo/quire). Upstream takes no pull requests, so changes made here stay here, and this fork follows upstream by merging it. Work on the fork is tracked in [kurtlieber/dotfiles#540](https://github.com/kurtlieber/dotfiles/issues/540); the automatic sync in [#570](https://github.com/kurtlieber/dotfiles/issues/570); its own releases in [#597](https://github.com/kurtlieber/dotfiles/issues/597).

## Branches and merges

- `main` is upstream's latest release plus this fork's patch and its own fixes.
- The repository allows only merge commits. A squash of an upstream merge would drop upstream's history, and the next sync would bring the same commits again.
- Commits use a GitHub noreply address.

## The fork's patch

Quire's updater (`desktop/src/update.rs`) asks this fork for releases, not upstream: `API`, `DOWNLOADS` and `PAGE` name `kurtlieber/quire`. The Update button then installs this fork's builds, which keep the patch, so a device stays on this fork once it runs one of them.

The sync re-applies the patch after every merge, with `sed` on every file under `desktop/src` that names `bjarneo/quire`. It stops with an error when no file there names this fork's download address, so a release that moves the updater ships nothing until the patch is updated.

The updater also reads this fork's own versions, such as `0.5.6-kl.1` (Releases, below). That code is in `desktop/src/update/fork.rs`, and two lines of `update.rs` call it. Those two lines are this fork's own change, not the patch, so an upstream release that changes them conflicts and stops the sync: merge by hand, as below.

The Android app (`android/`) still asks upstream: this fork builds no APK.

## The fork's own fixes

Upstream takes no pull requests, so a fix made here stays here until upstream fixes the same thing. Each fix has an issue upstream that links its commit.

- **The mouse wheel stops at the caret** in source mode, on Wayland with an input method such as fcitx5. The fix is in the vendored winit, `desktop/vendor/winit/src/platform_impl/linux/wayland/seat/text_input/mod.rs`. Upstream issue: [bjarneo/quire#2](https://github.com/bjarneo/quire/issues/2). Its tests are outside Quire's `cargo test`; run `cargo test --lib text_input` in `desktop/vendor/winit`.

A fix ships once Kurt merges it into `main`: the sync builds it as an own release, upstream's version plus `-kl.N` (Upstream sync and Releases, below). Upstream's later releases merge on top of it.

An upstream release that changes the same lines conflicts with the fix and stops the sync. Merge by hand, as below. Once upstream has fixed the same thing, take upstream's file and remove the fix's entry here.

## Upstream sync

`.github/workflows/upstream-sync.yml` runs every day at 06:17 Manila time. It also runs when a change to it or to `desktop/` reaches `main`, and by hand. By Kurt's choice (2026-11-13, #570), upstream's releases merge, build and ship with no review. When upstream's latest release is not this fork's latest, the workflow:

1. Merges upstream's release tag into `main` and re-applies the patch, in one merge commit.
2. Checks that `desktop/Cargo.toml` has the tag's version. Quire compares the two, and a mismatch would offer the same update forever.
3. Pushes `main` and the tag, which repeats upstream's (`v0.5.6`).
4. Creates the release, not yet the latest, and runs `release.yml` for the tag.
5. Makes the release the latest once the Linux, macOS and Windows files are attached and the Linux build's `quire --version` names the tag's version. Quire's Update button offers it from then on.

When this fork already has upstream's latest release, and `main` has changed `desktop/` since this fork's latest release, the workflow makes an own release instead:

1. Tags `main` as upstream's version plus `-kl.N`, one more than the last one: `v0.5.6-kl.1`, then `v0.5.6-kl.2`.
2. Steps 4 and 5 as above. The release notes link the changes since the previous release.

So Kurt's merge of a fix ships it. A change that waits is a merge that brings commits of upstream's that upstream hasn't released, such as a pull request from `bjarneo:main` (below): those ship with upstream's next release.

A failed build leaves the release not the latest, and the next run builds it again. A run by hand with **dry_run** on, from any branch, pushes nothing: it does steps 1 and 2 of an upstream release, or prints the own tag it would push.

GitHub doesn't let a workflow push changes to workflow files. When upstream's release changes its own workflows, the sync opens a pull request from `bjarneo:main` and requests Kurt's review instead. Merge it with **Create a merge commit**, and the next run patches, tags and builds the release. Ignore it, and Quire stays on the version it has.

A conflict in a file whose only change here is the patch takes upstream's file and re-applies the patch. Any other conflict stops the run. Merge by hand then:

```sh
git fetch https://github.com/bjarneo/quire.git refs/tags/vX.Y.Z
git switch -c upstream-merge origin/main
git merge FETCH_HEAD      # resolve the conflicts, then commit
git push origin upstream-merge
```

Open a pull request from `upstream-merge` into `main`.

A failed run of any workflow here posts to the "Failure alerts" issue in kurtlieber/dotfiles, from the `quire-fork-watch` timer on archie (#597).

GitHub turns off a scheduled workflow after 60 days without activity in a public repository. Each run calls GitHub's API to turn the workflow on, the way keepalive actions restart that clock; this fork has not yet gone 60 days without a release to prove it. If it is ever off, turn it on in the Actions tab.

## Releases

A tag that starts with `v` runs `.github/workflows/release.yml`, which builds Linux, macOS and Windows archives and attaches them to the tag's release. A tag the sync pushes doesn't start a workflow, so the sync runs `release.yml` itself.

Tags are upstream's version (`v0.5.6`), or upstream's version plus `-kl.N` for an own release (`v0.5.6-kl.1`). A build takes its version from its tag (`desktop/src/update/fork.rs`), and `desktop/Cargo.toml` stays upstream's, so an upstream release never conflicts there. Quire's updater compares four numbers, with N the fourth: `0.5.6` < `0.5.6-kl.1` < `0.5.6-kl.2` < `0.5.7`.

Builds before `v0.5.6-kl.1` compare only three numbers and never offer an own release. A device on one of them takes `v0.5.6-kl.1` by hand once, or upstream's next release through the Update button; from then on it gets both kinds. v0.5.4 is the last release whose updater asks upstream.

The Pages workflow is turned off in this fork: the website stays upstream's.

## Where it is installed

- monarchy and anarchy: the chezmoi script `run_onchange_after_quire.sh` in [kurtlieber/dotfiles](https://github.com/kurtlieber/dotfiles) installs a pinned release on a new host (app note `homelab/.agents/rules/apps/quire.md` there). From then on, Quire's Update button keeps it current.
- mbp: Kurt installs the macOS archive from this fork's releases by hand, once. From then on, the Update button keeps it current.
- Android: later, signed with Kurt's own key.
