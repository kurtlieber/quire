# This fork

Kurt Lieber's fork of [bjarneo/quire](https://github.com/bjarneo/quire). Upstream takes no pull requests, so changes made here stay here, and this fork follows upstream by merging it. Work on the fork is tracked in [kurtlieber/dotfiles#540](https://github.com/kurtlieber/dotfiles/issues/540).

## Branches and merges

- `main` is upstream's `main` plus this fork's own changes.
- The repository allows only merge commits. A squash of an upstream merge would drop upstream's history, and the next sync would bring the same commits again.
- Commits use a GitHub noreply address.

## Upstream sync

`.github/workflows/upstream-sync.yml` runs every day at 06:17 Manila time and can be run by hand. When upstream's `main` has commits that this `main` lacks, it opens a pull request from `bjarneo:main` into `main` and requests Kurt's review. That pull request follows upstream as it moves, so at most one is open at a time.

GitHub turns off a scheduled workflow after 60 days without activity in the repository. If upstream goes quiet that long, turn the workflow back on in the Actions tab.

### When the pull request has a conflict

GitHub can't resolve a conflict on a branch of upstream's repository. Merge it here instead:

```sh
git fetch upstream main
git switch -c upstream-merge origin/main
git merge upstream/main     # resolve the conflicts, then commit
git push origin upstream-merge
```

Open a pull request from `upstream-merge` into `main`, and close the one from `bjarneo:main`.

## Releases

A tag that starts with `v` runs `.github/workflows/release.yml`, which builds Linux, macOS and Windows archives and attaches them to the tag's release.

- While the fork has no changes of its own, its tags repeat upstream's version: `v0.5.1`.
- Once it has changes, a release adds a suffix to the upstream version it is built on: `v0.5.2-kl.1`, then `v0.5.2-kl.2`.

The Pages workflow is turned off in this fork: the website stays upstream's.

## Where it is installed

- monarchy and anarchy: the chezmoi script `run_onchange_after_quire.sh` in [kurtlieber/dotfiles](https://github.com/kurtlieber/dotfiles) installs the pinned Linux release (app note `homelab/.agents/rules/apps/quire.md` there).
- mbp: Kurt installs the macOS archive from this fork's releases by hand.
- Android: later, signed with Kurt's own key.
