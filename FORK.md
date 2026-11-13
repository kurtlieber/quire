# This fork

Kurt Lieber's fork of [bjarneo/quire](https://github.com/bjarneo/quire). Upstream takes no pull requests, so changes made here stay here, and this fork follows upstream by merging it. Work on the fork is tracked in [kurtlieber/dotfiles#540](https://github.com/kurtlieber/dotfiles/issues/540); the automatic sync in [#570](https://github.com/kurtlieber/dotfiles/issues/570).

## Branches and merges

- `main` is upstream's latest release plus this fork's patch.
- The repository allows only merge commits. A squash of an upstream merge would drop upstream's history, and the next sync would bring the same commits again.
- Commits use a GitHub noreply address.

## The fork's patch

Quire's updater (`desktop/src/update.rs`) asks this fork for releases, not upstream: `API`, `DOWNLOADS` and `PAGE` name `kurtlieber/quire`. The Update button then installs this fork's builds, which keep the patch, so a device stays on this fork once it runs one of them.

The sync re-applies the patch after every merge, with `sed` on every file under `desktop/src` that names `bjarneo/quire`. It stops with an error when no file there names this fork's download address, so a release that moves the updater ships nothing until the patch is updated.

The Android app (`android/`) still asks upstream: this fork builds no APK.

## Upstream sync

`.github/workflows/upstream-sync.yml` runs every day at 06:17 Manila time. It also runs when a change to it reaches `main`, and by hand. By Kurt's choice (2026-11-13, #570), upstream's releases merge, build and ship with no review. When upstream's latest release is not this fork's latest, the workflow:

1. Merges upstream's release tag into `main` and re-applies the patch, in one merge commit.
2. Checks that `desktop/Cargo.toml` has the tag's version. Quire compares the two, and a mismatch would offer the same update forever.
3. Pushes `main` and the tag, which repeats upstream's (`v0.5.6`).
4. Creates the release, not yet the latest, and runs `release.yml` for the tag.
5. Makes the release the latest once the Linux, macOS and Windows files are attached. Quire's Update button offers it from then on.

A failed build leaves the release not the latest, and the next run builds it again. A run by hand with **dry_run** on, from any branch, does steps 1 and 2 only, and pushes nothing.

GitHub doesn't let a workflow push changes to workflow files. When upstream's release changes its own workflows, the sync opens a pull request from `bjarneo:main` and requests Kurt's review instead. Merge it with **Create a merge commit**, and the next run patches, tags and builds the release. Ignore it, and Quire stays on the version it has.

A conflict in a file whose only change here is the patch takes upstream's file and re-applies the patch. Any other conflict stops the run. Merge by hand then:

```sh
git fetch https://github.com/bjarneo/quire.git refs/tags/vX.Y.Z
git switch -c upstream-merge origin/main
git merge FETCH_HEAD      # resolve the conflicts, then commit
git push origin upstream-merge
```

Open a pull request from `upstream-merge` into `main`.

GitHub turns off a scheduled workflow after 60 days without activity in a public repository. Each run calls GitHub's API to turn the workflow on, the way keepalive actions restart that clock; this fork has not yet gone 60 days without a release to prove it. If it is ever off, turn it on in the Actions tab.

## Releases

A tag that starts with `v` runs `.github/workflows/release.yml`, which builds Linux, macOS and Windows archives and attaches them to the tag's release. A tag the sync pushes doesn't start a workflow, so the sync runs `release.yml` itself.

Tags repeat upstream's version, with no suffix: Quire's updater reads the tag as its version, and the patch alone doesn't change what Quire does. v0.5.4 is the last release whose updater asks upstream.

The Pages workflow is turned off in this fork: the website stays upstream's.

## Where it is installed

- monarchy and anarchy: the chezmoi script `run_onchange_after_quire.sh` in [kurtlieber/dotfiles](https://github.com/kurtlieber/dotfiles) installs a pinned release on a new host (app note `homelab/.agents/rules/apps/quire.md` there). From then on, Quire's Update button keeps it current.
- mbp: Kurt installs the macOS archive from this fork's releases by hand, once. From then on, the Update button keeps it current.
- Android: later, signed with Kurt's own key.
